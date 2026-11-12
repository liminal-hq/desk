# Storage: block devices, FAT and the path layer on the native track

The design for reading files from disks in the native kernel: roadmap R74 (the block-device trait and the disk drivers), R75 (a filesystem, read-only first), R76 for modules (a `.DMD` loaded from disk) and the code side of R125 (the path and mount model). The build plan is [`STORAGE-PLAN.md`](STORAGE-PLAN.md).

> **Status.** A design, written on 2026-11-11 against `main` at `68b5a26`, with three of the owner's decisions of the same day (below). Nothing in it is built. The INT 13h reads it builds on are built and tested ([`N3-BIOS-PATH.md`](N3-BIOS-PATH.md)); everything else is a target. Every size is an estimate. The path and mount model (R125) is **not decided**: the path layer here is written against the proposal and waits for the decision.

## Contents

- [Decisions](#decisions)
- [What exists today](#what-exists-today)
- [The layers](#the-layers)
- [The block device](#the-block-device)
- [Partitions](#partitions)
- [FAT: the vendored rust-fatfs](#fat-the-vendored-rust-fatfs)
- [The fallback: a read-only FAT crate of our own](#the-fallback-a-read-only-fat-crate-of-our-own)
- [The drivers](#the-drivers)
- [Where the root lives](#where-the-root-lives)
- [The path layer (gated on R125)](#the-path-layer-gated-on-r125)
- [Concurrency, preemption and paging](#concurrency-preemption-and-paging)
- [R80a and R80b](#r80a-and-r80b)
- [The hosted track](#the-hosted-track)
- [Test images](#test-images)
- [Costs on a 486](#costs-on-a-486)
- [Questions for the owner](#questions-for-the-owner)
- [What changes in other documents](#what-changes-in-other-documents)
- [What is and is not verified](#what-is-and-is-not-verified)

## Decisions

Decided by the owner on 2026-11-11:

1. **`/` lives on a FAT16 disk on the emulated IDE controller** for the first storage milestone. The kernel boots from the raw boot floppy as it does today (stage 1, stage 2, the flat kernel from LBA 18) and reads `/` from the first hard disk. Reason: it needs no change to the loader or the boot image, the INT 13h LBA path to that disk already works, and an IDE disk is what a real 486 has.
2. **Vendor `rust-fatfs` (MIT)** rather than write our own FAT crate. Reason: a mature reader and writer of FAT12, FAT16 and FAT32 with long names, which the writable filesystem (R75's second half) would otherwise have to rebuild; the cost is a vendored dependency in the kernel, which the owner accepts under the conditions in [FAT](#fat-the-vendored-rust-fatfs). Our own read-only crate stays a recorded [fallback](#the-fallback-a-read-only-fat-crate-of-our-own).
3. **R80 is two named milestones**: **R80a**, the first OS milestone at stage 1 (preemptive tasks in ring 0, unprotected), and **R80b**, the same with protection (paging, ring 3, system calls). Reason: an app does not change between the stages by design ([`APP-MODEL.md`](APP-MODEL.md)), so the milestone can be reached and named honestly before protection. Storage is planned against R80a; [R80a and R80b](#r80a-and-r80b) says what R80b changes.

Proposed here, for the owner to confirm or change with the questions at the end: the block device is the HAL sketch with three small changes; disks are found by an MBR partition table, with an unpartitioned disk accepted; the vendored crates live in a tracked `third_party/` directory; the read-only guarantee is an adapter that refuses writes; the FAT adapter sits in a new `storage/` workspace; the INT 13h stopgap stays for R80a, and ATA PIO replaces it for IDE afterwards.

## What exists today

Read from the code, not the documents:

- **INT 13h works from the kernel** (`kernel/src/bios.rs`, `kernel/src/biostest.rs`). The trampoline runs one BIOS interrupt with interrupts off and the 8259s on the BIOS vectors; a call is refused from an interrupt handler (`InInterrupt`), while another is in progress (`Busy`) and with paging on (`PagingEnabled`). The `bios` test mode reads the boot floppy by CHS (AH=02h; 16 sectors from LBA 18 equal the running kernel) and, with an IDE disk attached, drive `0x80` by LBA (AH=41h, AH=42h with a disk address packet at `BOUNCE:0`, the data at `BOUNCE:0x200`), and checks that a read past the end fails.
- **The bounce buffer** is 8 KiB at physical `0x11000`, inside the 64 KiB block `0x10000` to `0x1FFFF`, so a floppy DMA into it neither crosses a 64 KiB boundary nor leaves the 8237's reach. With the packet in its first sector, one LBA call moves at most 15 sectors.
- **`desk_hal::Bios`** limits its one buffer to `BIOS_BUFFER_MAX` = 512 bytes, too small for disk reads ([`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#hal-feedback)). The kernel's disk code does not need the trait: it is kernel code and uses `bios::call` and the bounce buffer directly, as `biostest.rs` does. The constant stays as it is.
- **`tools/native-run.sh --hd FILE`** attaches `FILE` as the first IDE disk (`-drive if=ide,index=0,format=raw`) with the boot order kept on the floppy. `tools/native-test.sh` makes a 1 MiB disk with known text in sectors 0 and 1 for the BIOS test.
- **The boot image** (`tools/native-image.sh`) is a raw floppy, not a FAT volume, and the kernel is capped at 1,000 sectors (about 500 KiB) by stage 1's low load buffer.
- **Nothing else**: no block-device trait in `hal/hal/src/` (the sketch is in [`HAL.md`](HAL.md#storage-sketch-the-native-storage-milestone-builds-it)), no filesystem, no path type, no ATA or floppy driver, nothing vendored (`NOTICE` says so).
- **The CI image** (`ghcr.io/liminal-hq/ci-dos`, the digest in `.github/workflows/ci.yml`) has GNU mtools 4.0.43 (`mformat`, `mcopy`, `mmd`, `mdir`, `mattrib`, `mtype`, `mpartition`) and dosfstools 4.2 (`mkfs.fat`, `fsck.fat`), checked on 2026-11-11 by running the image.

## The layers

```
 apps, the shell, the loader             names a file: a Path (S5), or until then a list of names
 ───────────────────────────────────────────────────────────────────────────────────────────────
 namespace (S5, after R125)              Path, per-task Context, mount table: / and /mnt/<name>, virtual /mnt and /dev
 Filesystem trait (S5)                   open, read, list, stat — the core's view of one volume
 ───────────────────────────────────────────────────────────────────────────────────────────────
 desk-fs (storage/fs)                    Volume: mount read-only, list, open, read; MBR partitions; BlockIo adapter
 fatfs (third_party/rust-fatfs)          the FAT12/16/32 code, vendored and pinned
 ───────────────────────────────────────────────────────────────────────────────────────────────
 BlockDevice (desk-hal)                  512-byte blocks by LBA
 BiosDisk (kernel, S2) · desk-ata (S3) · desk-fdc (S4) · MemDisk (desk-hal-mock)
```

Two rules hold the layers apart:

- **Below the path layer nothing parses a path.** `desk-fs` takes names one at a time (or a `/`-joined string it builds itself and hands to `fatfs`); it never sees a drive letter, a `\`, a `[volume]` prefix or a current directory. So S1 to S4 do not depend on R125, and S5 can change the spelling of paths without touching a driver.
- **The block device knows nothing of filesystems.** A driver moves blocks; partitions, caching and the read-only guarantee are `desk-fs`'s.

## The block device

The HAL's sketch, made concrete in `hal/hal/src/storage.rs` (a new file, so it collides with no other branch):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockError {
    NoMedia,          // no disk in the drive, or no drive
    Timeout,          // the device did not answer in its bound
    Io(u8),           // the device or the BIOS reported an error; the byte is its status (INT 13h AH, the ATA error register)
    ReadOnly,         // a write to a device or wrapper that refuses writes
    OutOfRange,       // a block past the end
    BadLength,        // a buffer that is empty or not a multiple of the block size
    Busy,             // the device is in use by another caller (the BIOS trampoline's guard)
}

pub trait BlockDevice {
    /// 512 for every device in this design; a device with another size is refused by desk-fs.
    fn block_size(&self) -> u32;
    /// The number of blocks (u32 LBA: 2 TiB at 512 bytes).
    fn blocks(&self) -> u32;
    /// Whether `write` will refuse.
    fn read_only(&self) -> bool;
    /// Reads `buf.len() / block_size` blocks from `lba` into `buf`. A device splits a long read into the transfers it can make.
    fn read(&mut self, lba: u32, buf: &mut [u8]) -> Result<(), BlockError>;
    fn write(&mut self, lba: u32, buf: &[u8]) -> Result<(), BlockError>;
}
```

The changes from the sketch: `Io` carries the status byte, so a transcript can name the BIOS's or the drive's error; `BadLength` and `Busy` exist because the INT 13h device has both cases; `read_only` lets `desk-fs` refuse a writable mount early. `&mut self` because every real device has state (the drive selected, the motor). The contract: **never from an interrupt handler** (the INT 13h device would get `InInterrupt`, and the ATA and floppy drivers poll or wait); blocking; one caller at a time (see [Concurrency](#concurrency-preemption-and-paging)); every wait bounded ([`HAL.md`](HAL.md#errors)).

The mock is `desk_hal_mock::MemDisk`: a `Vec<u8>` of whole blocks, with a log of every read (`lba`, `count`) so tests can count device calls, an optional injected error at a given block, and a read-only switch.

## Partitions

`desk-fs` finds a volume in two ways:

- **An MBR** (sector 0 ends in `0x55AA` and its four entries are sane: status `0x00` or `0x80`, a type that is not zero, a start and length inside the disk, entries not overlapping): each FAT entry (types `0x01`, `0x04`, `0x06`, `0x0E`; `0x0B` and `0x0C` for FAT32) is a volume, reached through a `Window` that offsets block numbers by the partition's start and bounds them by its length. Extended partitions (`0x05`, `0x0F`) are listed and not followed in this design.
- **No partition table** (a "superfloppy": sector 0 is a FAT boot sector with a jump at byte 0 and a sane BPB): the whole disk is the volume. Floppies are always like this.

The order is: a sane MBR first, then a boot sector at sector 0. The FAT type comes from the cluster count, as Microsoft's specification defines it, never from the partition type or the BPB's "FAT16" text: an experiment for this design formatted a 16 MiB partition of type `0x04` with `mformat` and got **FAT12** (8-sector clusters, about 4,000 of them, under FAT12's limit of 4,085), and only with `-c 4` FAT16. The test disk forces its cluster size so that it is FAT16, and checks the type it got.

## FAT: the vendored rust-fatfs

### What is vendored

| Crate | Version | Source | Licence | Why |
|---|---|---|---|---|
| `fatfs` | 0.4.0 (unreleased), commit `38b3a7950bf0e765e34286d1dfe76c519ee347e2` of `https://github.com/rafalh/rust-fatfs`, 2026-10-16 ("In `FileSystem::new` seek to position 0") | git | MIT (`LICENSE.txt`, "Copyright 2017 Rafał Harabień") | The FAT code |
| `bitflags` | a 2.x release at least two weeks old when S1 is built (2.13.2 was the newest on 2026-11-11) | crates.io | MIT OR Apache-2.0 | A non-optional dependency of `fatfs` |
| `log` | a 0.4.x release at least two weeks old when S1 is built (0.4.35, published 2026-10-30, is too new today) | crates.io | MIT OR Apache-2.0 | A non-optional dependency: `fatfs`'s `log_macros.rs` names `log::LevelFilter` even with every log level off |

**Why the git commit and not crates.io's 0.3.6.** The newest release on crates.io is 0.3.6 (2023-01-17). The `no_std` support with `fatfs`'s own `Read`, `Write` and `Seek` traits is on the default branch (version 0.4.0 in its `Cargo.toml`, `rust-version = "1.71"`, edition 2021), which is what this design needs; as recalled, 0.3.x reached `no_std` through the `core_io` crate, which tracks particular nightlies and is not something to build a kernel on. The chosen commit is 26 days old on the day of writing. The fit task re-reads the upstream history and may move the pin to a later commit, recording why.

What was read of the pinned commit on 2026-11-11 (the source fetched into a scratch directory and searched, not built): 6,725 lines in 11 files under `src/`; **no `unsafe`**; **no atomics**, interior mutability by `core::cell::RefCell` and `Cell` only; **no `f32` or `f64`**; 64-bit arithmetic for byte offsets (`u64` positions in `Seek`), which on an i486 becomes calls into `compiler_builtins`, not floating point; 182 sites that can panic (`unwrap`, `expect`, `assert`, `unreachable` and slice indexing, counted roughly with one pattern), which is why the fit task fuzzes it. Features: `std`, `alloc`, `lfn` (long names, needs `alloc`), `unicode` (case tables), `chrono` (optional dependency), `log_level_*`; the default set enables `chrono`, `std`, `alloc`, `lfn`, `unicode` and `log_level_trace`.

### Where it lives and how it is pinned

- **`third_party/rust-fatfs/`, `third_party/bitflags/`, `third_party/log/`**: tracked copies of the upstream sources, exactly as published (the crate's `src/`, `Cargo.toml`, licence files and README; no tests, examples or CI files). Not `vendor/`: that directory is gitignored and holds the DOS tools that `tools/setup.sh` links or downloads, so nothing in it is reviewed or versioned. Not inside `kernel/`: the adapter and its host tests (`storage/`) build the crate for the host as well as for the kernel, and two trees should not each think they own it. A top-level `third_party/` is the usual name, and it keeps every third-party line in one place for `NOTICE` and for the licence-header and markdown checks to skip.
- **`third_party/README.md`** lists each crate: name, version, the upstream URL, the exact commit or release, the date taken, the licence, the SHA-256 of every file (or of a sorted manifest of them), and local changes (none, by policy; a needed fix goes upstream first, and a local patch, if one is unavoidable, is a file in `third_party/<crate>/PATCHES.md` with the diff and the upstream issue).
- **Cargo.** `storage/fs/Cargo.toml` depends on `fatfs = { path = "../../third_party/rust-fatfs", default-features = false, features = ["alloc", "lfn"] }`. The `fatfs` manifest asks for `bitflags = "2"` and `log = "0.4"` from crates.io; the workspaces that build it (`storage/` and `kernel/`) redirect both with `[patch.crates-io]` to the `third_party/` copies, so the build reads no registry and `Cargo.lock` records path sources. Whether `cargo build --locked --offline` resolves a patched dependency with no registry index at all is the first thing the fit task checks; if it does not, the fallback is to change the two dependency lines in `third_party/rust-fatfs/Cargo.toml` to paths, recorded as a local patch.
- **A check**, `tools/check-third-party.sh` (new): every file under `third_party/` matches its recorded hash, every crate has a licence file and a row in `third_party/README.md` and in `NOTICE`, and nothing else in the repository depends on a crate outside `third_party/` or the workspaces' own (the `Cargo.lock` files list no registry source). It is listed for the CI stack to wire in.
- **Updating** is a pull request that replaces a crate's directory from the new upstream commit or release, updates the hashes, the row and `NOTICE`, and reruns the fit checks below with the numbers in the description (size before and after, the fuzz run, the disassembly checks).
- **`tools/check-headers.sh` and `tools/check-docs.py`** skip `third_party/`, as they skip `vendor/` and `design/`: the files are upstream's and must stay byte-identical. That is a two-line change to each script, in S1, and it says so.

### How it sits beside the project's rules

The rules for `shell/` (no external crates, `#![forbid(unsafe_code)]`, no floats, no compare-and-swap, no clocks) and for `hal/` stay as they are. `fatfs` is **kernel-side**: it is a dependency of `desk-fs` in `storage/`, which the kernel links; nothing in `shell/` or `hal/` depends on it, and no app sees it. The owner's rule is that the no-external-crates rule is scoped to `shell/` and may be adapted for the kernel by its own small documentation change; S0 in the plan proposes the wording for `AGENTS.md` and `CLAUDE.md` (the owner edits those files): vendoring kernel-side code needs a `NOTICE` entry, a pinned version and a fit check.

`desk-fs` itself, the code we write, follows `shell/`'s rules except the external-crate one: `no_std` + `alloc`, `#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]`, the compare-and-swap ban through a `clippy.toml` copied from `hal/`, no clocks. Its only external dependency is `fatfs` from `third_party/`. Clippy's lints do not run on the vendored crates; the fit check stands in for them on the built code (no `cmpxchg`, no x87 instruction, no `unsafe` in the source).

### The fit check

Before the vendored code is committed, and again on every update:

- **Builds** for `kernel/i486-desk-none.json` with `-Zbuild-std=core,alloc` (`RUSTC_BOOTSTRAP=1`, `-Zjson-target-spec`), with the feature set above, from `storage/` and from `kernel/`, `--locked` and offline.
- **No floats and no x87.** A throwaway static library of `desk-fs` for the kernel target, disassembled with `llvm-objdump` (or `objdump` in the CI image): no x87 opcodes (`fld`, `fst`, `fstp`, `fild`, `fist`, `fadd`, `fmul`, `fdiv`, `fcom`, `fucom`, `fnstcw`, `fldcw` and the rest of `D8` to `DF`) and no `cmpxchg` (the HAL's rule, which also catches a `log` logger path that should be dead code; `log` keeps its logger in atomics, and as recalled `set_logger` uses compare-and-swap, which must not be linked).
- **No `unsafe`** in the three crates' sources (a search, recorded with the commit), and none in the code paths `desk-fs` calls if a later version adds some.
- **Binary size**: `kernel.bin` before and after linking the storage code, against the 1,000-sector cap of `tools/native-image.sh`. The cost is unmeasured; a guess of 40 to 90 KiB with `opt-level = "s"` and LTO is an **estimate** to be replaced by the figure. If the kernel would pass about 900 sectors, the task stops and reports: the cap is stage 1's, and loading apps from disk relieves it only later.
- **No panic and no hang on a corrupt image.** A fixed-seed fuzzer (an integer generator, no randomness source) mutates the `mtools` images (FAT12 floppy and FAT16 disk) 10,000 times each and, for each, mounts, walks every directory and reads every file through `desk-fs` on the host. Every outcome must be `Ok` or an error; a panic fails the test, and a walk that exceeds the [read budget](#read-only-caching-and-the-read-budget) fails it as a hang. Targeted damage as well: a cluster chain that loops, a chain into a free or reserved cluster, a cluster number past the end, a directory entry larger than its chain, every BPB field zero and maximal, a truncated image. A panic found here is reported upstream; the pin waits for the fix, or `desk-fs` validates the case before calling `fatfs`, and the task records which.
- **Reads touch only what they must**: reading a 300 KB file and listing a directory of 100 entries, counted through `MemDisk`'s log, stay within a stated number of device reads (the figures pinned by the first run and reviewed).

If the fit check fails in a way that cannot be fixed by an upstream fix, a validation in `desk-fs` or a later pin, the fallback below is built instead, and the owner is asked first.

### Read-only, caching and the read budget

`desk-fs` wraps a `BlockDevice` (or a partition `Window` of one) in `BlockIo<D>`, the type that implements `fatfs`'s `IoBase`, `Read`, `Write` and `Seek`, because `fatfs::FileSystem` requires all three (`IO: ReadWriteSeek`) even for reading:

- **Read** keeps a byte position; whole aligned blocks go straight to the device in one multi-block read, partial blocks go through the cache.
- **Write returns an error** (`BlockIoError::ReadOnly`, which `fatfs` reports as `Error::Io`): this is the read-only guarantee, independent of the device and of `fatfs`'s options. A read-only mount writes nothing: reading the pinned source, `fatfs` writes on its own only to clear the volume's dirty flag at unmount, and only if the flags changed (`set_dirty_flag`), and to update access dates only when `update_accessed_date` is on, which it is not by default. The adapter makes that a guarantee rather than a reading, and a test mounts, reads and drops a volume on a `MemDisk` and checks that no write was attempted.
- **Seek** is bounded by the volume's length.
- **A cache** of 16 blocks (8 KiB; an estimate to be tuned by the read counts), replaced by a counter-based least-recently-used rule, no clocks. It matters: `fatfs` reads FAT entries a few bytes at a time, and without a cache each would be an INT 13h call of about 100 µs or more on a 486 for an IDE disk, and far more for a floppy.
- **A read budget** per top-level operation (`mount`, `list`, `open`, `read`): `desk-fs` resets a counter before calling `fatfs` and the adapter refuses reads beyond it (a budget of a few times the volume's cluster count in blocks, an estimate). It turns a loop in a corrupt cluster chain, which a reader that does not detect loops would follow for ever, into an error.

`desk-fs`'s API (a sketch; the builder may change it and says so):

```rust
pub struct Disk<D: BlockDevice> { /* the device and its partition list */ }
pub struct PartitionInfo { pub index: u8, pub kind: u8, pub start: u32, pub blocks: u32, pub active: bool }
impl<D: BlockDevice> Disk<D> {
    pub fn open(dev: D) -> Result<Disk<D>, FsError>;                  // reads the MBR, or finds a superfloppy
    pub fn partitions(&self) -> &[PartitionInfo];
    pub fn mount(&mut self, index: u8) -> Result<Volume<'_, D>, FsError>;   // read-only; index 0 is a superfloppy's whole disk
}
pub struct VolumeInfo { pub fat: FatKind, pub label: [u8; 11], pub serial: u32, pub cluster_bytes: u32, pub clusters: u32 }
pub struct Entry { pub short: [u8; 11], pub long: Option<String>, pub dir: bool, pub size: u32 }
impl<'d, D: BlockDevice> Volume<'d, D> {
    pub fn info(&self) -> VolumeInfo;
    pub fn list(&self, dir: &[&str]) -> Result<Vec<Entry>, FsError>;   // names one at a time, from the root
    pub fn stat(&self, file: &[&str]) -> Result<Entry, FsError>;
    pub fn read_to_vec(&self, file: &[&str], limit: u32) -> Result<Vec<u8>, FsError>;
    pub fn read_at(&self, file: &[&str], offset: u32, buf: &mut [u8]) -> Result<usize, FsError>;
}
pub enum FsError { Device(BlockError), NoVolume, NotFat, Unsupported(&'static str), Corrupt(&'static str), NotFound, NotADirectory, IsADirectory, TooLarge, BadName, Budget }
```

Each name is matched case-insensitively against the 8.3 name and, with `lfn`, the long name; a name containing `/`, `\`, NUL, or equal to `.` or `..`, is `BadName` (the path layer resolves those before). `FsError`'s `Display` lines are transcript lines and stay stable once S2 pins them.

## The fallback: a read-only FAT crate of our own

Kept as the recorded alternative, built only if the fit check fails and the owner agrees:

- **What**: `storage/fat` (`desk-fat`), FAT12 and FAT16 read-only, 8.3 names with long-name entries recognised and skipped (or read, later), the same `Volume` API as `desk-fs` so the kernel side does not change. Under `shell/`'s rules in full, so no `AGENTS.md` change would have been needed.
- **What it must get right**: the FAT type by cluster count; FAT12 entries of a cluster that straddle two sectors; the fixed root directory region of FAT12 and FAT16; end-of-chain (`>= 0xFF8` and `>= 0xFFF8`), bad-cluster and free-cluster values; loop detection by bounding a chain at the cluster count; `0xE5` and `0x05` as the first byte; the volume-label and long-name attributes; `..` with cluster 0 meaning the root; a size that disagrees with its chain.
- **Tests**: the same `mtools` images and fuzzing, plus a small image writer in its tests for crafted damage (as the loader plan's `elfgen`).
- **Cost**: about 1,100 lines including tests (an estimate); a read-only reader only, so the writable filesystem would need its own work later, which is the reason the owner chose to vendor.
- **What vendoring costs instead**: three crates and about 7,000 lines (`fatfs` alone 6,725) that the project did not write and must review on every update, a documentation change to the project's rules, a `NOTICE` entry, the fit check, kernel size (unmeasured), and upstream's pace (the default branch has been unreleased since 2023's 0.3.6). What it buys: FAT32, long names and writing, maintained by others and used by other projects.

## The drivers

### INT 13h, the stopgap (S2)

`kernel/src/disk.rs`: `BiosDisk { drive, blocks, lba, geometry }` implements `BlockDevice` over `bios::call(0x13, …)` and the bounce buffer.

- **Probe**: AH=08h for the geometry (and whether the drive exists), AH=41h for the LBA extensions, then AH=48h (extended parameters) for the exact sector count when available, else cylinders × heads × sectors.
- **Reads**: AH=42h when the extensions are there (the packet at `BOUNCE:0`, the data from `BOUNCE:0x200`, at most 15 sectors a call), else AH=02h by CHS, at most 16 sectors a call and never across a track (some BIOSes refuse a multi-track read, and a floppy read cannot cross a head without the multi-track bit). The kernel copies out of the bounce buffer after each call.
- **Retries**: a floppy read that fails is retried up to three times with AH=00h (reset) between, as DOS does; a hard-disk read once. The final status is `BlockError::Io(AH)`.
- **Writes**: refused (`ReadOnly`). The writable filesystem enables AH=43h later, behind the same trait.
- **Errors from the trampoline**: `Busy` and `InInterrupt` map to `BlockError::Busy`; `PagingEnabled` to `Io(0xFF)` with a clear transcript line.

It is a stopgap because every call turns interrupts off for the BIOS's own time, which is tens of microseconds for an IDE read under emulation and unmeasured on a 486 (the [cost estimate](N3-BIOS-PATH.md#cost) is "ten to a few hundred milliseconds" for a floppy sector), and because it does not survive paging without the identity-mapped low megabyte ([`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#paging)). It is enough for R80a.

### ATA PIO (S3)

`hal/drivers/ata` (`desk-ata`), over `PortIo` only, no `unsafe`, as [`HAL.md`](HAL.md) plans:

- The primary channel at `0x1F0` to `0x1F7` and its control register at `0x3F6`; the secondary at `0x170` and `0x376` as a constructor argument.
- `IDENTIFY DEVICE` (`0xEC`) for the model, the LBA28 sector count and whether LBA is supported; `READ SECTORS` (`0x20`) with LBA28, at most 256 sectors a command, data by 256 `inw` per sector. Polled: `nIEN` set in the device control register, so IRQ 14 is not used yet; the 400 ns delay after selecting a drive as four reads of the alternate status register; every wait on `BSY` and `DRQ` bounded and returning `Timeout`; `ERR` returning `Io(error register)`; a software reset (`SRST`) and re-identify after a timeout.
- Writes refused until the writable filesystem.
- A mock ATA device in `desk-hal-mock`, a `MemDisk` behind the task-file registers, with scripted delays and errors, so every path is host-tested.
- In the kernel, `kernel/src/ata.rs` wraps it with `hal::PORTS`. The BIOS also drives the controller (INT 13h and the IRQ 14 and 15 lines are left to the BIOS during a call, [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#interrupts-the-pics-the-timer-and-the-keyboard)), so the driver selects the drive and checks the status before every command and holds no state in the device between calls.
- IRQ 14 and `rep insw` come later, when the measured cost asks for them: the interrupt needs the wait and wake of K2 to be useful, and `rep insw` is a platform fast path behind a HAL method, because a driver has no `unsafe`.

### The floppy controller (S4, after R80a)

`hal/drivers/fdc` (`desk-fdc`) and `hal/drivers/dma8237` (`desk-dma8237`, which the sound milestone also plans; whichever is built first builds it for both): the 82077AA at `0x3F0` to `0x3F7` (the digital output register for the motor, reset and DMA enable; the main status register; the FIFO; the configuration control register for the data rate), `SPECIFY`, `RECALIBRATE`, `SEEK`, `SENSE INTERRUPT STATUS`, `READ DATA` with MFM and the multi-track bit, IRQ 6 to signal completion, ISA DMA channel 2 in single mode, the buffer from `PhysMem::alloc_dma` (contiguous, below 16 MiB, not crossing 64 KiB, as [`HAL.md`](HAL.md#memory-mapped-regions-and-physical-memory-region-built-physical-memory-sketch) sketches), the motor's spin-up wait (about 300 to 500 ms) and a motor-off timer. It is the riskiest driver (DMA, timing, real drives untested) and nothing on the way to R80a needs it: the boot floppy is read by the loader, and `/` is on the IDE disk.

## Where the root lives

Decided (decision 1): **for the first storage milestone and R80a, `/` is the first FAT16 partition of the IDE disk at BIOS drive `0x80`**, and the kernel still boots from the raw floppy. The floppy is not a volume and has no path.

What that implies:

- The kernel is not a file on `/`. Updating it means rebuilding the floppy image, as today.
- A machine with only a floppy drive has no `/`. That is acceptable for R80a; the alternative recorded for later is a **FAT12 boot floppy with the kernel in reserved sectors**: a BPB in sector 0 (stage 1 would move its patched kernel length from offset 3, which is inside the BPB, to a reserved field), stage 2 and the kernel in about 1,018 reserved sectors, and about 930 KiB of FAT12 data that DOS and `mtools` can read and write. It touches stage 1 (with its 512-byte budget), `tools/native-image.sh` and every boot of every test, so it belongs with the installer (R109 to R111) or with S4, not with R80a.
- A fresh test disk is made for each run by a script, never committed (`*.img` is gitignored anyway).
- Which partition is `/` when there are several: the first FAT partition marked active, else the first FAT partition, else a superfloppy. A disk with none leaves the kernel without `/`, which the `fs` mode reports and fails on.

## The path layer (gated on R125)

Built only after the owner decides R125. Written here against the proposal of the owner's explorer (<https://claude.ai/artifact/6F48KzWj7g2VUEJb9BFETL>, read on 2026-11-11), so that the plan is ready the day it is decided; every point below is the proposal's, not a decision.

- **One generic path in the core**: a root and a list of names, no separator, no drive letter. `desk-path` in `shell/path`, under `shell/`'s rules in full (no external crates), because the shell's apps, the file picker and the loader all hold paths. `Name` is a validated component (not empty, not `.` or `..`, no `/`, `\` or NUL, at most 255 bytes); `Path { root: Root, names: Vec<Name> }` with `Root::Namespace` (the namespace's `/`) or `Root::Volume(VolumeId)` (a stable volume name).
- **The namespace**: `/` is the system volume; other volumes are under `/mnt/<name>`, named letter-like (`a`, `d`) by default so both tracks agree; `/mnt` and `/dev` are virtual folders in memory that shadow any real `MNT` or `DEV` folder on the system volume. Devices are typed nodes (`File`, `Dir`, `Device`); windows, the palette and the screen are not in the namespace.
- **The canonical stored form**: `[system]/music/cocoa.mid` for the system volume and the FAT volume serial for others (`[4A1F-22C0]/music/boss.mid`); square brackets cannot appear in an 8.3 name, so the prefix is never a folder; 8.3 names that are all upper case are stored in lower case. A stored path names the disk, not the drive, so it fails cleanly while that disk is out.
- **Per-track presentation**: native spells `/music/cocoa.mid` and `/mnt/a/…`; hosted spells `C:\MUSIC\COCOA.MID` and maps letters to mounts. Both `/` and `\` separate names when parsing. Native refuses drive letters (the proposal's sandbox does).
- **Resolution is per task** (decided, [`APP-MODEL.md`](APP-MODEL.md#decisions) decision 20): a `Context { cwd: Path }` is an argument of `resolve`, never a global; each task holds its own, so one app's `cd` cannot change another's names. Normalisation (`.`, `..`, `..` at the top clamped), then the mount table, then the owning backend.
- **Equality and moves**: paths are compared after normalisation and by the backend's name rules (FAT folds case); a copy can loop into itself through a mount, and a move across volumes is a copy then a delete, never a rename.
- **The `Filesystem` trait** (the proposal draws it at the HAL boundary, core above and track-specific below): `open`, `read`, `list`, `stat` first, `rename`, `write` and `same_volume` with the writable filesystem. In `desk-hal` (`hal/hal/src/fs.rs`), so the hosted track's DOS backend in `rt` can implement it later beside the native FAT one in `desk-fs`.

What the decision can change, and what it cannot: the spelling, the stored form, the mount names and the virtual folders are all inside `desk-path` and the mount table, so any answer to R125 changes S5 alone. What it cannot change without touching S1 to S4: the rule that drivers and `desk-fs` take names one at a time.

## Concurrency, preemption and paging

- **Before K1** (today's single-threaded kernel) one caller at a time is automatic.
- **With stage-1 preemption** (K1, `docs/TASKS-PLAN.md` when written): an INT 13h call runs with interrupts off, so a task switch cannot happen inside one, but two tasks could each start a read: the trampoline's guard would turn the second into `Busy`. The disk therefore sits behind one kernel lock (the mutex of K2 and Y1, `desk_wait` underneath), taken by `desk-fs`'s caller around each top-level operation, never an interrupts-off section, which would hold interrupts off for a whole file. Every INT 13h read is a scheduling latency of the BIOS's duration; the transcript of the `fs` mode reports the longest call in ticks so that the cost is visible. The ATA driver (S3) polls with interrupts on and is preemptible between commands.
- **With paging** (P1): the INT 13h path needs the low megabyte identity-mapped or paging off for the call ([`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#paging)); the ATA driver needs nothing (port I/O); the floppy driver's DMA buffer needs its physical address, which `PhysMem` already returns.
- **No interrupt handler ever reads a disk**, on any path.

## R80a and R80b

**R80a** (decision 3): the kernel at stage 1 loads `HELLOWIN.DKX` from the FAT16 disk and runs it as a task. Storage provides it through S2: `BiosDisk` on drive `0x80`, `desk-fs` mounting `/` read-only, `read_to_vec` of the file, then the loader's host (M6) and the app runner (A2). S2 already proves the chain with a `.DMD` module. **R80a does not need S5**: if R125 is decided in time, R80a names the file `/desk/apps/hellowin.dkx` through `desk-path`; if not, the kernel names it by components (`["DESK", "APPS", "HELLOWIN.DKX"]`) through `desk-fs` and nothing about the app changes. The app itself needs no file access in R80a, so no file service for apps is part of it.

**R80b** changes nothing in the filesystem, the drivers or the disk format. It changes where the layers run and who calls them: storage stays in the kernel's address space, reached by apps (when they get file access) through the system-call table of stage 3, which copies a path string out of the task's memory and resolves it against that task's `Context`; per-task path resolution was designed for that from the start (decision 20). With paging on, the INT 13h stopgap either keeps the identity-mapped low megabyte or gives way to ATA PIO for IDE (S3), which is the reason to have S3 before R80b.

## The hosted track

Unchanged. The hosted Desk reads files through DOS (`rt::dos::File`), not through block devices ([`HAL.md`](HAL.md#storage-sketch-the-native-storage-milestone-builds-it)), and does not link `fatfs`. S5's `Filesystem` trait is what the hosted track will implement over DOS, in the hosted lane, so that `desk-path` and the mount table run on both tracks; until then nothing on DOS depends on this design.

## Test images

Made by `mtools` at test time, never committed; the scripts run in the CI image, and on a host without `mtools` in the same image through Docker, as the other Docker steps do.

- **FAT16 IDE disk** (`tools/native-fsimage.sh`, S2): 16 MiB (an estimate; large enough for FAT16 at 2 KiB clusters, small for a RAM-backed `/tmp`), an MBR with one FAT16 partition from sector 63 made by `mpartition`, formatted by `mformat -c 4` with a fixed serial (`-N`) and label, and then files: `HELLO.TXT` with a known line; `BIG.BIN` (300,000 bytes from a fixed-seed generator that the kernel repeats); a fragmented `FRAG.BIN` (written after a file that is then deleted, so its chain has at least two runs; the kernel prints the run count); a directory with 100 entries, which spans several clusters; a path eight directories deep; one long name; `DESK/MODULES/HELLO.DMD` copied from `dist/modules/`. Timestamps are set to a fixed date before copying (`touch -d`, then `mcopy -m`), and the image bytes are never compared, only what the kernel reads: `mformat` writes its own version into the boot sector's OEM name (`MTOO4050` from 4.0.50 on the development machine, 4.0.43 in the CI image), so images from two `mtools` versions differ.
- **A damaged copy** for `fs-bad` (`tools/fat-damage.py`, S2): a loop in one file's chain, a chain past the last cluster, a directory entry larger than its chain, one flipped bit in `HELLO.DMD` (the loader refuses it by checksum), and a second partition whose BPB says 0 bytes per sector.
- **Host images** (S1): a 1.44 MB FAT12 floppy (`mformat -f 1440`) and a FAT16 superfloppy and partitioned disk, built by `storage/images.sh` into `$TMPDIR` and read by the host tests and the fuzzer.

## Costs on a 486

All unmeasured, all estimates: one INT 13h call about 100 µs plus the BIOS's own time; an IDE sector by PIO about 256 `inw` at about 1 µs each on ISA, so a few hundred kilobytes a second at best; the floppy at its 500 kbit/s data rate, under 60 KB/s, with a seek and a motor spin-up of the order of a second; the cache's 8 KiB and a FAT12 floppy's whole FAT (9 sectors) fit in memory easily, a FAT16 FAT of up to 256 sectors does not need to. Emulator timings say nothing about these.

## Questions for the owner

1. **The path and mount model (R125).** Gates S5 only. Default: accept the proposal as written above.
2. **Long names.** Build `fatfs` with `lfn`? Default: yes, read-only; they cost little and the proposal's native presentation wants them. A long name can be shown on native and is invisible on the hosted track's 8.3 view.
3. **Superfloppies.** Accept a hard disk with no partition table? Default: yes (QEMU users make them with `mformat` alone); an MBR is tried first.
4. **The pin.** The default branch's commit `38b3a79` (unreleased 0.4.0) rather than crates.io's 0.3.6. Default: yes, for its `no_std` traits.
5. **A `storage` label** for pull requests. None exists; default: use `enhancement`, `rust` and `testing`, and add `storage` only if the owner wants it.

## What changes in other documents

- [`HAL.md`](HAL.md): the storage surface as built (S1), `MemDisk` in the mock's table, `desk-ata` and `desk-fdc` rows when built, and the `Filesystem` trait (S5).
- [`ROADMAP.md`](ROADMAP.md): R74 and R75 point here and at the plan; R75's "Possibly: a Rust FAT crate" becomes the decision to vendor `rust-fatfs`; R80 becomes R80a and R80b; "Reuse and vendoring" records `fatfs`, `bitflags` and `log` with their pins.
- `NOTICE`: the three crates, their licences and their place in `third_party/` (S1).
- `AGENTS.md` and `CLAUDE.md`: the rule on external crates scoped to `shell/`, and the conditions for kernel-side vendoring (S0, wording proposed, the owner edits).
- [`DIRECTION.md`](DIRECTION.md): the open questions on the filesystem, boot media and early disk access answered for now (FAT16 on IDE, the raw floppy, INT 13h then ATA PIO).
- [`NATIVE-BOOT.md`](NATIVE-BOOT.md): test modes 19 and 20 and the disk the `fs` modes need.
- [`SPEC.md`](SPEC.md) ("Files on disk") and [`FORMATS.md`](FORMATS.md): stored paths, once R125 is decided (S5).

## What is and is not verified

Verified on 2026-11-11 for this design: the CI image's `mtools` and `dosfstools` versions (by running the image); that `mformat` on a 16 MiB partition of type `0x04` gives FAT12 and with `-c 4` FAT16, that a superfloppy of the same size is FAT16, that `mcopy` writes long-name entries for a name that is not 8.3, and that the OEM name carries the `mtools` version (on the development machine's 4.0.50, in a scratch directory since removed); the pinned `fatfs` commit's line count, features, dependencies, and the absence of `unsafe`, atomics and floats in its source (by reading, not building); the licences and newest versions of `fatfs`, `bitflags` and `log` on crates.io and GitHub.

Not verified: that `fatfs` builds for the kernel target, its size, its behaviour on corrupt images, whether `log`'s compare-and-swap stays out of the binary, whether `[patch.crates-io]` builds offline with no index, every driver, every cost above, and anything on real hardware.
