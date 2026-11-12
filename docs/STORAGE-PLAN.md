# Storage: the build plan

The task-by-task plan for building [`STORAGE.md`](STORAGE.md): roadmap R74 (the block device and the drivers), R75 read-only (FAT through the vendored `rust-fatfs`), R76 for modules (a `.DMD` loaded from disk) and the code side of R125 (the path type and the mount table). Each task is one pull request, sized for one builder (a Sonnet agent or a person) working from this document, the design and the files it names.

> **Status.** A plan, written on 2026-11-11 against `main` at `68b5a26`, with the owner's decisions of the same day: `/` on a FAT16 IDE disk, `rust-fatfs` vendored, and R80 split into R80a (stage 1, unprotected) and R80b (protected) ([`STORAGE.md`](STORAGE.md#decisions)). No task has started. The API sketches are the design's; a builder may change a signature when the code shows a better one, and says so in the pull request and in [`STORAGE.md`](STORAGE.md) in the same branch. S5 waits for the owner's decision on R125. Every size is an estimate.

## Contents

- [Scope](#scope)
- [Order and parallelism](#order-and-parallelism)
- [Rules for every task](#rules-for-every-task)
- [Checks for the CI stack](#checks-for-the-ci-stack)
- [S0. The rule on vendored crates](#s0-the-rule-on-vendored-crates)
- [S1a. BlockDevice and MemDisk](#s1a-blockdevice-and-memdisk)
- [S1b. Vendor, pin and fit rust-fatfs](#s1b-vendor-pin-and-fit-rust-fatfs)
- [S2. The kernel reads the FAT16 disk and loads a module from it](#s2-the-kernel-reads-the-fat16-disk-and-loads-a-module-from-it)
- [S3. ATA PIO](#s3-ata-pio)
- [S4. The floppy controller (after R80a)](#s4-the-floppy-controller-after-r80a)
- [S5. The path type and the mount table (gated on R125)](#s5-the-path-type-and-the-mount-table-gated-on-r125)
- [Definition of done](#definition-of-done)

## Scope

In: the block-device trait and its mock, the vendored FAT code behind a read-only adapter, the INT 13h block device, a FAT16 test disk, kernel test modes 19 and 20, a module read from disk and loaded through the loader's host (M6 of [`LOADER-PLAN.md`](LOADER-PLAN.md)), ATA PIO, the floppy controller, and the path layer after R125. Out: writing (R75's second half, "S6", planned when this is done), a filesystem of our own for the system volume, a FAT12 boot floppy, the hosted track's `Filesystem` backend over DOS (the hosted lane), a file service for apps (R80a needs none), and `HELLOWIN.DKX` itself (the window-system plan's R80a task uses S2's reader).

## Order and parallelism

At most two tasks run at once. A wave starts when everything it depends on is merged; each task is a pull request off `main`.

| Wave | Tasks, in parallel | Depends on | Size (new lines, roughly, tests included) |
|---|---|---|---|
| 1 | S0 the rule on vendored crates ∥ S1a `BlockDevice` and `MemDisk` | — | documents ∥ 300 |
| 2 | S1b vendor, pin and fit `rust-fatfs` | S0 (with the owner's edit made), S1a | 900, plus about 7,000 vendored lines not counted |
| 3 | S2 kernel reads the disk, modes 19 and 20, a module from disk ∥ S3 ATA PIO, driver and mock half | S2: S1b and M6; S3: S1a | 600 ∥ 450 |
| 4 | S3 kernel half (on S2's `fs` mode) ∥ S5 path type and mount table | S3: S2; S5: S1b and **the owner's R125 decision** | 250 ∥ 900 |
| later | S4 the floppy controller | S1a, after R80a | 1,000 |

**The first file read from disk** is S2, at the end of wave 3: the kernel prints `HELLO.TXT`'s line and the module's `log>` lines on serial in QEMU. Nothing before S2 runs in an emulator; S1a and S1b make sure on the host that what S2 reads is right.

**Against the whole path to R80a** (the coordinator's map, an estimate): S0 and S1a are fillers for any free lane now; S1b fits by about wave 4 of the map, S2 by wave 6 (it needs M6), S5 by wave 7 if R125 is decided by wave 5, S3 by wave 10 and S4 after R80a. R80a needs S2; it does not need S5 or S3 ([`STORAGE.md`](STORAGE.md#r80a-and-r80b)).

Branches: `docs/vendored-crates-rule` (S0), `feat/block-device` (S1a), `feat/vendor-fatfs` (S1b), `feat/native-fs` (S2), `feat/ata-pio` (S3), `feat/floppy` (S4), `feat/path-layer` (S5).

Shared files: S1a and S3 both add to `hal/platform/mock` (different files; the second rebases `lib.rs`'s module list). S2 and the loader's M4 to M7 all edit `kernel/src/main.rs`, `tools/native-image.sh` and `tools/native-test.sh`: S2 adds modes 19 and 20 only, as single lines in each list, and its own section of `tools/native-test.sh` after the loader's, so a rebase keeps both. S3's kernel half extends S2's `fs` section and `tests/native/expected-fs.txt`, which S2 created; that is not a pinned file of another area.

## Rules for every task

- **Read first**: [`AGENTS.md`](../AGENTS.md), [`STORAGE.md`](STORAGE.md), and the documents the task names. Follow `AGENTS.md`: Canadian English, no hard-wrapped markdown, real em dashes, licence headers on new source files (never on files under `third_party/`), Conventional Commits ending with the agent co-author trailer, a PR title without a prefix, labels on the PR (`enhancement`, `rust` and `testing` for code; `documentation` and `design-docs` for documents; no `storage` label exists), merge commits only, and no merging without the owner's go-ahead.
- **Work in a git worktree** under `.claude/worktrees/` on the task's branch off `origin/main`, never in the main checkout.
- **Missing tools run in Docker**, never installed on the host: the CI image `ghcr.io/liminal-hq/ci-dos` (its digest is in `.github/workflows/ci.yml`) has Rust 1.98.0 with `rust-src`, QEMU, NASM, GNU mtools 4.0.43 and dosfstools 4.2 (checked 2026-11-11). Run containers with `--user "$(id -u):$(id -g)"` and a writable `HOME` and `CARGO_HOME`. Do not prune Docker images or volumes.
- **Scratch stays small.** `/tmp` is a RAM-backed filesystem shared by many jobs: disk images go to `$TMPDIR` or `dist/`, at most 16 MiB each, and are removed by the script that made them; no copies of the repository with `target/` directories.
- **Do not edit `tools/check.sh` or `.github/workflows/*`.** A CI rework is in progress (a stack that refactors `tools/check.sh` into groups and adds a scope script). Each task adds its own check script or a section in `tools/native-test.sh`, and lists it under [Checks for the CI stack](#checks-for-the-ci-stack) in its pull request; the CI stack wires it in. A task that cannot avoid editing either file says so in its description and why. Editing `tools/check-headers.sh` and `tools/check-docs.py` (S1b, to skip `third_party/`) is allowed and said.
- **The rules of the crates**: `desk-fs` (`storage/fs`) follows `shell/`'s rules (`no_std` + `alloc`, `#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]`, no compare-and-swap through a `clippy.toml` copied from `hal/`, no clocks or randomness) with one exception: it depends on the vendored `fatfs` in `third_party/` and on nothing else outside the repository. `desk-path` (`shell/path`) follows `shell/`'s rules with no exception. Drivers in `hal/drivers/` have no `unsafe` and no compare-and-swap ([`HAL.md`](HAL.md)). Kernel `unsafe` carries a one-line `SAFETY:` comment. Files under `third_party/` are never edited by hand; a change is an update ([`STORAGE.md`](STORAGE.md#where-it-lives-and-how-it-is-pinned)).
- **Nothing writes to a disk.** Every device refuses writes or is wrapped by the read-only adapter; a test proves it for each new path.
- **The pinned files do not change**: `tests/native/expected.txt` and every existing `tests/native/expected-*.txt` (the BIOS run's included), the shell goldens and their transcripts, and the loader's module transcripts. Storage code prints only in its own test modes. A task that needs to change a pinned file has found a regression and stops to report it.
- **Test modes**: 19 `fs` and 20 `fs-bad` (reserved in the coordinator's map; 16 to 18 are the loader's, 21 and later the tasks and window plans'). S3 and S4 extend `fs`; a task that needs another number asks for it in the roadmap first.
- **Fix a bug with a check that fails before the fix.** Every refusal is a test that feeds a crafted input and expects that error, and each new emulator check has a negative run that must fail.
- **Verify before the PR**: the task's acceptance commands, `python3 -I tools/check-docs.py`, `tools/check-headers.sh`, `tools/check.sh` in the CI image where the task touches what it runs, and the task's own new check script. The PR's test plan says exactly what ran and what did not, and that nothing ran on real hardware.
- **Update the documents the task changes** in the same branch, and correct [`STORAGE.md`](STORAGE.md) where the code chose differently.

## Checks for the CI stack

New checks the tasks add, for the CI rework to wire into `tools/check.sh` (one line each) and its path scopes. None of the tasks edits `tools/check.sh` itself.

| Task | Script or section | What it runs | Path scope (suggested) | Time (estimate) |
|---|---|---|---|---|
| S1a | none new | `hal/` is already checked; the new file and mock are covered by the existing HAL step | `hal/**` | +1 s |
| S1b | `storage/check.sh` | `cargo fmt --check`, `clippy --all-targets -D warnings` and `test` in `storage/`; `clippy -D warnings` for `kernel/i486-desk-none.json` with `-Zbuild-std=core,alloc`; `storage/images.sh` and the image tests (needs `mtools`); the fuzz run; the disassembly checks (no x87, no `cmpxchg`) | `storage/**`, `third_party/**`, `hal/hal/src/storage.rs` | 1 to 3 min cold |
| S1b | `tools/check-third-party.sh` | the hashes, licences, `NOTICE` rows and registry-free `Cargo.lock` files | `third_party/**`, `NOTICE`, `**/Cargo.lock` | under 1 s |
| S2 | the "Storage" section of `tools/native-test.sh` | the `fs` and `fs-bad` runs and their negative runs, after the loader's sections | `kernel/**`, `storage/**`, `third_party/**`, `tools/native-*.sh`, `tools/fat-damage.py`, `tests/native/expected-fs*.txt` | +30 to 60 s |
| S3 | `hal/` step (existing) and the `fs` section | the ATA driver's host tests; the `fs` run reads through both devices | `hal/drivers/ata/**`, as S2's | +10 s |
| S4 | `hal/` step and the `fs` section | the floppy and DMA drivers' host tests; the `fs` run with a second floppy | `hal/drivers/fdc/**`, `hal/drivers/dma8237/**` | +20 to 40 s |
| S5 | `shell/` step (existing) and `storage/check.sh` | `desk-path`'s tests and the mount table's | `shell/path/**`, `storage/**` | +5 s |

## S0. The rule on vendored crates

**Goal.** The documentation change the roadmap requires before any code is vendored (R74, R75): the project's rule on external crates scoped to `shell/` (and the crates under `shell/`'s rules), with the conditions for vendoring kernel-side code. `AGENTS.md` and `CLAUDE.md` are the owner's files: **the task proposes the wording and the owner makes the edit**; the task's pull request changes only the documents below and carries the proposed wording in its description.

**Files.** [`ROADMAP.md`](ROADMAP.md): the "Reuse and vendoring" section records the decision to vendor `rust-fatfs`, `bitflags` and `log` (not yet done), and points at [`STORAGE.md`](STORAGE.md#fat-the-vendored-rust-fatfs); R74's and R75's "may be adapted" sentence points at the proposed wording. `tools/progress-report/roadmap.json` if it carries the same text.

**Proposed wording**, for the owner to apply or change:

- `AGENTS.md`, "Code Conventions", the `shell/` bullet: replace "no external crates" with "no external crates (the rule is `shell/`'s; kernel-side code may use a vendored crate under the rule below)".
- `AGENTS.md`, "Code Conventions", a new bullet after the HAL one: "**Vendored crates (kernel side only).** Code that `shell/`, `hal/` and apps never depend on, such as the FAT code under the kernel's storage, may use a third-party crate when the owner agrees. It is vendored in `third_party/<crate>/` as published, never edited by hand (a fix goes upstream, a local patch is recorded in `PATCHES.md`), pinned to an exact version or commit with the hashes of its files in `third_party/README.md`, listed in `NOTICE` with its licence, which must be compatible with Apache-2.0 OR MIT, and admitted only after a fit check: it builds for `kernel/i486-desk-none.json`, its built code has no x87 instructions and no `cmpxchg`, its size is measured, and it survives fuzzed corrupt input without a panic. `tools/check-third-party.sh` checks the hashes and the listings."
- `AGENTS.md`, "Repository Layout": "`third_party/` — vendored third-party crates for kernel-side code, as published and pinned (`third_party/README.md`); outside the licence-header and markdown checks". And `storage/` — "the native track's storage: one Cargo workspace (`desk-fs`, the read-only adapter over the vendored FAT code and the HAL's block devices), tested on the host and built for the kernel's target ([`docs/STORAGE.md`](STORAGE.md))".
- `AGENTS.md`, "Licence and Copyright": "Files under `third_party/` keep their upstream licence and headers and get none of ours."
- `CLAUDE.md`, the `shell/` paragraph: "no external crates" stays, followed by "(kernel-side code may vendor a crate under `AGENTS.md`'s rule: `third_party/`, `NOTICE`, a pin and a fit check)". In "State of the repository", once S1b lands: "`storage/` holds the native track's read-only FAT access over the vendored `rust-fatfs` (`third_party/`)."

**Acceptance.** `python3 -I tools/check-docs.py` passes; the PR description holds the wording above (adjusted if the owner asks); the owner has made the `AGENTS.md` and `CLAUDE.md` edit, in this pull request or their own, before S1b merges.

**Depends on**: nothing. **Parallel with**: S1a. **Labels**: `documentation`, `design-docs`, `rust`.

**Risks.** The rule must not read as permission for `shell/` or `hal/` to take crates; the wording names the sides explicitly.

## S1a. BlockDevice and MemDisk

**Goal.** The HAL's storage surface as [`STORAGE.md`](STORAGE.md#the-block-device) defines it, and a memory-backed mock with a read log and injected errors, host-tested. No filesystem.

**Files.**

- `hal/hal/src/storage.rs`: `BlockError` (with `Display`, and `From<BlockError> for HalError`: `NoMedia` to `NoDevice`, `Timeout` to `Timeout`, `Busy` to `Busy`, the rest to `InvalidArgument` or `Platform(status)`), `BlockDevice`, and `Window<D>`: a `BlockDevice` over a range of another (offset and length, refusing blocks outside with `OutOfRange`). `hal/hal/src/lib.rs`: the module and its re-exports.
- `hal/platform/mock/src/disk.rs`: `MemDisk` (`new(blocks)`, `from_bytes(Vec<u8>)` refusing a length that is not a multiple of 512, `reads() -> &[(u32, u32)]`, `writes()`, `fail_at(lba, BlockError)`, `set_read_only`). `hal/platform/mock/src/lib.rs`: the module.
- `hal/hal/tests/storage.rs` or tests in the files: as below.
- [`HAL.md`](HAL.md): the storage surface marked built, with the changes from the sketch; `MemDisk` in the platforms table.

**API.** As in [`STORAGE.md`](STORAGE.md#the-block-device).

**Acceptance.**

- `cargo test` in `hal/`: a read of one block and of several returns the stored bytes; a buffer of 0 bytes or of 513 is `BadLength` and the device is not called; a block past the end, and a multi-block read whose last block is past the end, are `OutOfRange` with nothing copied; an injected `Io(0x40)` at block 7 fails a read of 5 to 9 and not of 0 to 6; a write to a read-only `MemDisk` is `ReadOnly` and changes nothing; the read log records each call once.
- `Window`: offsets, bounds and `OutOfRange` at its own end, not the parent's; a window that does not fit its parent is refused at construction.
- The existing HAL step passes for the host and both cross targets; no `cmpxchg` in a throwaway build of the new code for the kernel target (as [`HAL.md`](HAL.md#what-was-and-was-not-verified) did).

**Depends on**: nothing. **Parallel with**: S0. **Labels**: `enhancement`, `rust`, `testing`.

**Risks.** Small. Keep `BlockDevice` object-safe (no generic methods), in case a platform later wants `&mut dyn BlockDevice`.

## S1b. Vendor, pin and fit rust-fatfs

**Goal.** The vendored FAT code in `third_party/`, pinned and listed; `desk-fs` in a new `storage/` workspace: the read-only `BlockIo` adapter with its cache and read budget, MBR partitions, and the `Disk` and `Volume` API; host tests on `mtools` images; the fit check of [`STORAGE.md`](STORAGE.md#the-fit-check) passed and its figures recorded. This task is a spike in its first half: if the fit check fails in a way that cannot be fixed (see Risks), it stops and reports, and the owner chooses between a later pin and the [fallback](STORAGE.md#the-fallback-a-read-only-fat-crate-of-our-own).

**Files.**

- `third_party/rust-fatfs/` (from commit `38b3a7950bf0e765e34286d1dfe76c519ee347e2`, or a later one with the reason recorded: `src/`, `Cargo.toml`, `LICENSE.txt`, `README.md`, `CHANGELOG.md`), `third_party/bitflags/` and `third_party/log/` (each the published `.crate` of a release at least two weeks old, unpacked: `src/`, `Cargo.toml`, the licence files, `README.md`), `third_party/README.md` (the table and the hashes), `NOTICE` (an entry for each: name, version or commit, copyright line, licence, "vendored in `third_party/` and linked into the native kernel").
- `storage/Cargo.toml` (workspace, `members = ["fs"]`, `[workspace.package]` as `shell/`'s, `[patch.crates-io]` for `bitflags` and `log`), `storage/Cargo.lock`, `storage/clippy.toml` (a copy of `hal/clippy.toml`), `storage/rustfmt.toml`, `storage/fs/Cargo.toml` (`desk-hal` by path, `desk-hal-mock` as a dev-dependency, `fatfs` by path with `default-features = false, features = ["alloc", "lfn"]`), `storage/fs/src/lib.rs`, `blockio.rs` (`BlockIo`, the cache, the budget), `mbr.rs`, `volume.rs`, `error.rs`, `storage/fs/tests/images.rs`, `storage/fs/tests/fuzz.rs`, `storage/fs/tests/readonly.rs`.
- `storage/images.sh`: builds the host images of [`STORAGE.md`](STORAGE.md#test-images) into a directory given as its argument, with fixed serials, labels and dates, and writes a manifest of each file's path, size and CRC-32 computed from the source files (not from the image). `storage/check.sh`: the checks in the [table](#checks-for-the-ci-stack). `tools/check-third-party.sh`.
- `tools/check-headers.sh`, `tools/check-docs.py`: skip `third_party/` (said in the PR).
- [`STORAGE.md`](STORAGE.md): the fit figures (the kernel size before and after, the number of fuzz cases, panics found and what was done, the read counts), and corrections. [`ROADMAP.md`](ROADMAP.md): "Reuse and vendoring" says what was vendored.

**What the image tests check.** `storage/check.sh` runs `storage/images.sh` into `$TMPDIR` and `cargo test` with `DESK_FAT_IMAGES` set to it; without the variable the image tests print that they were skipped (so a bare `cargo test` on a host without `mtools` still runs the rest), and `storage/check.sh` fails if `mtools` is missing rather than skip.

**Acceptance.**

- **Fit**: builds for `kernel/i486-desk-none.json` (clippy `-D warnings` on `desk-fs`) `--locked` and offline; the disassembly of a throwaway static library for the kernel target has no x87 instruction and no `cmpxchg`; no `unsafe` in `third_party/` (a search; the result in the PR); `kernel.bin` with `desk-fs` linked (a throwaway kernel build that calls `Disk::open` on an empty `MemDisk`-like stub, not committed) is measured and its size and sector count are in the PR and in [`STORAGE.md`](STORAGE.md).
- **Images**: the FAT12 floppy, the FAT16 superfloppy and the FAT16 partitioned disk each mount with the expected `VolumeInfo` (the FAT type checked against what was asked for, the label and serial as given); listing every directory and reading every file equals the manifest (names, sizes, CRC-32), long names included; a name looked up in another case is found; the 100-entry directory lists 100 entries; the fragmented file reads correctly.
- **Read-only**: mount, list, read and drop on a `MemDisk` record no write attempt; `BlockIo`'s `write` returns `ReadOnly` when called; a `MemDisk` set to read-only mounts.
- **Partitions**: an MBR with four entries lists four; a partition that runs past the disk, two that overlap and a type of 0 are refused or skipped with a named error; a superfloppy is found when sector 0 has no sane MBR; a disk with neither is `NoVolume`.
- **Refusals**: block size other than 512 (a crafted device), a BPB with 0 bytes per sector, a zero cluster size, a FAT count of 0, a truncated image: each a named `FsError` and no panic.
- **Fuzz**: 10,000 fixed-seed mutations of each image, each mounted and walked: every result `Ok` or an error, none a panic (the test harness catches it) and none past the read budget; the targeted damage of [`STORAGE.md`](STORAGE.md#the-fit-check) (a looped chain, a chain into a free or reserved cluster, a cluster past the end, an entry larger than its chain) each ends in an error.
- **Reads**: reading the 300,000-byte file and listing the 100-entry directory make at most the device reads the first run states, pinned in the test and reviewed in the PR.
- `tools/check-third-party.sh` passes, and fails with one byte of a vendored file changed and with a `NOTICE` row removed (both negative runs in the PR).

**Depends on**: S0 (the owner's edit made), S1a. **Parallel with**: nothing (wave 2). **Labels**: `enhancement`, `rust`, `testing`, `spike`.

**Risks.** A panic in `fatfs` on corrupt input: report it upstream, and either guard against the case in `desk-fs` before calling `fatfs` (preferred while upstream fixes it) or wait for a fixed pin; never patch `third_party/` silently. `[patch.crates-io]` needing a registry index offline: then change the two dependency lines in the vendored `Cargo.toml` to paths, recorded in `PATCHES.md`. `log`'s logger code reaching the binary with `cmpxchg`: it should be dead code; if it is not, the check fails and the task stops. A size that pushes the kernel near the 1,000-sector cap: the task stops and reports. If `fatfs` follows a looped chain within one directory read without returning to the adapter, the budget still ends it, because every cluster is a read.

## S2. The kernel reads the FAT16 disk and loads a module from it

**Goal.** The first file read from disk and R76 for modules: `BiosDisk` over INT 13h, `desk-fs` mounting `/` from the FAT16 IDE disk read-only, a test disk made by `mtools`, test modes 19 `fs` and 20 `fs-bad`, and `HELLO.DMD` read from `/DESK/MODULES/` and run through the loader's host (M6), with the same module lines as the loader's own modes.

**Files.**

- `kernel/Cargo.toml`, `kernel/Cargo.lock`: `desk-fs` by path and the `[patch.crates-io]` lines of `storage/`. `kernel/src/disk.rs`: `BiosDisk` ([`STORAGE.md`](STORAGE.md#int-13h-the-stopgap-s2)). `kernel/src/fstest.rs`: the scenarios below. `kernel/src/main.rs`: `TestMode::Fs = 19`, `FsBad = 20`, run after the loader's modes in the boot order, then `boot complete`.
- `tools/native-image.sh`: `fs` and `fs-bad` (one line each in the list and the `case`). `tools/native-fsimage.sh` (new): the FAT16 disk of [`STORAGE.md`](STORAGE.md#test-images) into `dist/native-fs.img`, with `--damaged` for `dist/native-fs-bad.img` through `tools/fat-damage.py` (new; Python, `-I`, standard library only, reading the BPB to find the offsets it patches). `tools/native-test.sh`: a "Storage" section after the loader's. `tests/native/expected-fs.txt`, `tests/native/expected-fs-bad.txt`.
- [`NATIVE-BOOT.md`](NATIVE-BOOT.md): the two modes and the disk they need; [`STORAGE.md`](STORAGE.md): the transcript as built.

**What the `fs` mode does and prints** (the lines are the design's; the builder settles them and the first run pins them):

```text
fs: drive 0x80: {N:1..} blocks of 512 bytes, LBA extensions: yes
fs: drive 0x80: partition 1, type 0x06, from block 63, {N:1..} blocks, active
fs: / is FAT16, label DESKROOT, serial 1234-5678, {N:4085..65524} clusters of 2048 bytes, mounted read-only
fs: / lists BIG.BIN, DEEP, DESK, FRAG.BIN, HELLO.TXT, MANY, Long name.txt
fs: /HELLO.TXT: 30 bytes: hello from the FAT16 disk
fs: /BIG.BIN: 300000 bytes, equal to the generator: yes, {N:1..} device reads
fs: /FRAG.BIN: {N:2..} runs, equal to the generator: yes
fs: /MANY lists 100 entries
fs: /DEEP/1/2/3/4/5/6/7/LEAF.TXT: found
fs: a write through the adapter is refused: read-only
fs: floppy through INT 13h by CHS: the kernel's {N:1..1000} sectors equal the running kernel: yes
fs: the longest INT 13h call took {N:0..} ticks
module: HELLO.DMD read from /DESK/MODULES/HELLO.DMD, {N:1000..20000} bytes
```

then the loader's lines for `hello` as M6's `life` scenario prints them (`  log> hello from a loaded program` and the rest, compared with `tests/module/scenarios.txt`'s `life` block, so the file from disk gives the same output as the file embedded), and `fs: memory as before: yes` (frames and heap, as the loader's modes compare). The floppy line exercises the CHS path across tracks and heads with reads that must split.

**`fs-bad`** boots with `dist/native-fs-bad.img` and must print one refusal per damage and carry on: the looped file ends with a named error (corrupt or budget), the chain past the end and the oversized entry with theirs, `module: refused HELLO.DMD: bad checksum (stored {HEX}, computed {HEX})`, the second partition `not a FAT volume`; then `boot complete`, exit status 0.

**Acceptance.**

- `tools/native-test.sh`: the `fs` run with `--hd dist/native-fs.img` matches `expected-fs.txt` and the `fs-bad` run with the damaged disk matches `expected-fs-bad.txt`, both exit 0, on `-cpu 486`.
- Negative runs that must fail: the `fs` mode with no `--hd` (the kernel prints `fs: drive 0x80: not present` and exits 1); `expected-fs.txt` with the `HELLO.TXT` line changed; the `fs-bad` transcript against `expected-fs.txt`; the `fs` mode against a disk formatted FAT12 by mistake (`mformat` without `-c`) must fail on the FAT16 line.
- Every existing native check is unchanged (`git diff origin/main -- tests/native/expected*.txt` shows only the two new files), the BIOS run's included.
- `kernel.bin`'s size before and after is in the PR, under the 1,000-sector cap.
- Optional, reported if tried: the `fs` image under Bochs (`NATIVE_EXTRA=1`); Bochs has not run the LBA path before ([`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#what-was-and-was-not-verified)).

**Depends on**: S1b; M6 of [`LOADER-PLAN.md`](LOADER-PLAN.md) for the module through the host (if M6 is late, the module is run through M4's single-module path and the `life` comparison waits for M6; the PR says which). **Parallel with**: S3's driver half. **Labels**: `enhancement`, `rust`, `testing`.

**Risks.** The kernel's size (the storage code and `fatfs`); test modes 19 and 20 colliding with another open branch (they are reserved); the disk image's FAT type (forced by the cluster size and checked); BIOS differences on the multi-sector CHS read (some refuse a read across a track, which `BiosDisk` never asks for). The heap: `read_to_vec` of a large file and `fatfs`'s allocations come from the 4 MiB kernel heap; the scenario's largest file is 300,000 bytes.

## S3. ATA PIO

**Goal.** `desk-ata` in `hal/drivers/ata`, polled LBA28 reads over `PortIo` with a mock ATA device; then in the kernel an `AtaDisk` that the `fs` mode reads through as well as `BiosDisk`, with the same results, so IDE no longer depends on the BIOS (needed before R80b's paging, not before R80a).

**Files.**

- Driver half: `hal/drivers/ata/Cargo.toml`, `src/lib.rs` (`Ata<P: PortIo>`, `identify`, `read`, `BlockDevice` for a selected drive), `hal/platform/mock/src/ata.rs` (the device model over a `MemDisk`: task-file registers, `BSY` and `DRQ` sequencing with scripted delays, `ERR` and the error register, `IDENTIFY` data, `SRST`), tests. `hal/Cargo.toml`: the member. [`HAL.md`](HAL.md): `desk-ata` built.
- Kernel half (after S2): `kernel/src/ata.rs`, `kernel/Cargo.toml`; `kernel/src/fstest.rs`: the `fs` scenario run through `BiosDisk` and then `AtaDisk`, with the same lines prefixed by the device, and a line comparing them; `tests/native/expected-fs.txt` extended.

**Acceptance.**

- Host: `identify` decodes the model string (byte-swapped words) and the LBA28 count; a read of 1, of 256 and of 300 sectors (split into two commands) equals the model's data; a `BSY` that never clears is `Timeout` after its bound and a reset follows; `ERR` with `ABRT` is `Io(0x04)`; a drive that is not there (status `0xFF` or `0x00`) is `NoMedia`; writes are `ReadOnly`; the drive is selected and the 400 ns delay made before every command (checked in the model's port log); no `unsafe`, no compare-and-swap.
- QEMU: the `fs` run reads the disk through both devices and prints `fs: ATA PIO and INT 13h read the same bytes: yes`; negative: the ATA half against a disk on the secondary channel only must report `NoMedia` for the primary.
- Every other native check unchanged.

**Depends on**: S1a (driver half), S2 (kernel half). **Parallel with**: S2 (driver half), S5 (kernel half). **Labels**: `enhancement`, `rust`, `testing`.

**Risks.** QEMU's IDE is more forgiving than real drives (timing, the 400 ns rule, drives that need `SRST` after power-up); the mock is written from the ATA specification, not from QEMU, so the two can disagree, and the emulator is then right. The BIOS also drives the controller between the kernel's commands; the driver keeps no state in the device across calls.

## S4. The floppy controller (after R80a)

**Goal.** `desk-fdc` and `desk-dma8237` over `PortIo`, IRQ 6 and `PhysMem` DMA memory, reading the boot floppy and a FAT12 floppy in drive B in QEMU, so the native kernel can read floppies without the BIOS.

**Files.** `hal/drivers/fdc/`, `hal/drivers/dma8237/` (or the sound milestone's, if it exists by then), mock models of both in `hal/platform/mock/`; `kernel/src/floppy.rs` (the IRQ 6 handler sets a flag the driver waits on, bounded; the DMA buffer from the frame allocator below 16 MiB not crossing 64 KiB); `tools/native-run.sh`: `--fdb FILE` for a second floppy (`-drive if=floppy,index=1`); `kernel/src/fstest.rs`: the boot floppy's kernel sectors through the FDC equal the running kernel, and drive B mounted as FAT12 and read; `tests/native/expected-fs.txt` extended.

**Acceptance.** Host: the command sequences (specify, recalibrate, seek, read data with MFM and the multi-track bit, sense interrupt) against the model; the DMA programming (mask, flip-flop, mode `0x46`, address, page, count, unmask); a missing IRQ is `Timeout`; a buffer that would cross 64 KiB is refused. QEMU: the floppy lines; negative: with IRQ 6 masked the read times out and the transcript says so. Every other check unchanged.

**Depends on**: S1a, the native `PhysMem` (or a kernel-local DMA allocation the task adds), and after R80a. **Labels**: `enhancement`, `rust`, `testing`.

**Risks.** The highest of the storage tasks: DMA below 16 MiB, motor and head-settle timing, QEMU's forgiving controller, and real drives untested. After K2, the IRQ wait should be a `desk_wait`, not a busy wait.

## S5. The path type and the mount table (gated on R125)

**Goal.** The path layer of [`STORAGE.md`](STORAGE.md#the-path-layer-gated-on-r125): `desk-path` (the generic path, parsing of the native, hosted and stored spellings, normalisation, the canonical stored form, presentation per track), the mount table with `/`, `/mnt/<name>` and the virtual `/mnt` and `/dev`, the per-task `Context`, and the `Filesystem` trait with `desk-fs` implementing it. **Built only after the owner decides R125**; if the decision differs from the proposal, this section is revised in the roadmap PR that records the decision, before the task starts.

**Files.**

- `shell/path/Cargo.toml`, `src/lib.rs`, `name.rs` (`Name`), `path.rs` (`Path`, `Root`, `VolumeId`), `parse.rs` (`parse_native`, `parse_hosted` with a letter table, `parse_stored`), `stored.rs` (`to_stored`, `from_stored`), `present.rs` (`present(track, &Path)`), `tests/`. `shell/Cargo.toml`: the member.
- `hal/hal/src/fs.rs`: `Filesystem` (`list`, `stat`, `open`, `read`; `NodeKind` of `File`, `Dir`, `Device`). `storage/fs/src/service.rs`: `Volume` as a `Filesystem`. `storage/fs/src/mount.rs` (or a `storage/namespace` crate if it grows): `MountTable` (`/` and `/mnt/<name>`), the virtual `/mnt` and `/dev`, `resolve(&self, input, &Context) -> Result<Resolved, PathError>`.
- `kernel/src/fstest.rs`: the `fs` mode resolves its files through the namespace (`/hello.txt`, `/desk/modules/hello.dmd`) as well as by components, and prints both agreeing; `tests/native/expected-fs.txt` extended. [`SPEC.md`](SPEC.md) and [`FORMATS.md`](FORMATS.md): stored paths as decided.

**Acceptance.**

- Host: the proposal's sandbox cases as tests: `/music/cocoa.mid` and `C:\MUSIC\COCOA.MID` resolve to the same node on the system volume and store as `[system]/music/cocoa.mid`; `/mnt/a/music/boss.mid` stores as `[4A1F-22C0]/music/boss.mid` and fails cleanly (`no disk`) when that volume is out, whichever letter it was in; `..` at the top clamps; `/` and `\` both separate; native refuses `C:`; a real `MNT` folder on the system volume is shadowed by `/mnt`, while the hosted literal `C:\MNT\A` names the real folder; `/dev/com1` is a `Device`; two contexts with different current folders resolve the same relative name to different nodes, and changing one does not change the other; a move across volumes is reported as a copy and a delete; every `Name` refusal (empty, `.`, `..`, a separator, NUL, 256 bytes); stored forms round-trip for every case.
- No input to the parsers panics: a fixed-seed generator of 10,000 strings over the separators, brackets, letters and dots.
- QEMU: the `fs` lines agree between the namespace and the components; every other check unchanged.

**Depends on**: S1b, **the owner's decision on R125**. **Parallel with**: S3's kernel half. **Labels**: `enhancement`, `rust`, `shell`, `testing`.

**Risks.** It is the shape every later app depends on (the picker, Files, Terminal, the `DESK.INI` stored paths); a hasty mount-naming rule is expensive to change once stored paths exist on disks. Keep `Context` a value the task owns (decision 20), never a global, so stage 2's per-task spaces change nothing. The hosted backend is not in this task; keep the trait free of native types so the hosted lane can implement it over DOS.

## Definition of done

Storage for R80a is done when, on `main`:

- the rule on vendored crates is in `AGENTS.md` and `CLAUDE.md` as the owner worded it, and `third_party/` holds `fatfs`, `bitflags` and `log`, pinned, hashed, listed in `NOTICE` and checked by `tools/check-third-party.sh`;
- `desk-fs` reads FAT12 and FAT16 images made by `mtools` on the host, refuses every crafted damage with a named error, survives the fuzz run, and never writes;
- the kernel in QEMU as a 486 reads the FAT16 IDE disk through INT 13h in the `fs` mode, refuses the damaged disk's faults in `fs-bad`, and loads `HELLO.DMD` from disk with the same module lines as the embedded file;
- the new checks are listed for the CI stack and run there once wired;
- [`STORAGE.md`](STORAGE.md) records the fit figures and what the code chose differently.

S3 (before R80b), S4 (after R80a) and S5 (after R125) each end with their own acceptance above; S5 is what makes the namespace of the R125 decision real.
