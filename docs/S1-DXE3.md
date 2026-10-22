# Spike s1-dxe3: Rust modules as DXE3 files

Can Rust code be built as a DJGPP DXE3 module, loaded into the core's address space at run time with `dlopen`, and call back into the core? This is the result of the spike. The code is in [`../spikes/s1-dxe3/`](../spikes/s1-dxe3/) and runs from `tools/check.sh`; everything below was run under headless DOSBox Staging 0.83.0 with CWSDPMI r7 in the `ci-dos` image, except where it says QEMU.

## Contents

- [Verdict and recommendation](#verdict-and-recommendation)
- [Method](#method)
- [What works: the commands](#what-works-the-commands)
- [Answers](#answers)
- [The two routes compared](#the-two-routes-compared)
- [What failed on the way](#what-failed-on-the-way)
- [What it changes](#what-it-changes)
- [What was and was not verified](#what-was-and-was-not-verified)
- [Reproducing it](#reproducing-it)

## Verdict and recommendation

**Verdict: yes, with limits.** A Rust `no_std` crate built with the skeleton's pipeline becomes a working `.DXE` with one extra step (`dxe3gen`). The core `dlopen`s it from disk, finds three entry points with `dlsym`, runs a full lifecycle, and the module calls back into the core, allocates through the core, formats strings and floats, and panics without taking the core down. Two modules load side by side with their own state. Load, run and unload 200 times leaves memory unchanged. The limits are real and are listed under [what it changes](#what-it-changes): link-time optimisation is mandatory, a stack overflow in a module cannot be recovered and probably corrupts the core first, a page fault cannot be tested under DOSBox, and one `.DXE` file is one instance.

**Recommendation: modules, route a.** Optional apps are `.DXE` modules that import the SDK by name (`extern "C"` `desk_*` functions), resolved by `dlopen` from a table the core registers with `dlregsym`; this is what `SDK.md` already described, with the corrections below. Route b (a table of function pointers passed to `app_start`) worked equally well, costs a few hundred bytes and some plumbing, and stays the documented fallback; switching is a feature flag in the spike's `rt` crate. The core's built-in apps (Files, Programs, Settings) may still be linked into the core instead of loaded, which keeps the essentials out of the crash path; that is a choice for milestone 4 and the spike does not depend on it. The fallbacks the spec named (apps built into the core, C apps, separate EXEs) are not needed.

## Method

- `spikes/s1-dxe3/modules/` is a Cargo workspace for the DOS target (the skeleton's `core/dos.json`, reused by path) with `rt`, a shared runtime, and three `staticlib` modules: `hello` (a `Vec<String>` history, a stub window), `counter` (its own state, x87 floating point) and `crash` (a panic, a null write, a stack overflow, a divide error, a panic holding heap blocks). `rt` is the global allocator (forwarding to the core), the panic handler, the callbacks and the `desk_app!` macro that defines `app_start`, `app_event` and `app_stop`. The `route-b` feature changes how the callbacks reach the core and nothing else.
- `spikes/s1-dxe3/host/` is a host in C that stands in for the core: `mod.c` loads modules and wraps every call into one in `setjmp`, `api.c` holds the callbacks, the tracked allocator, the export table (route a) and the function table (route b), and `main.c` has the scenarios. In the real Desk the core is Rust; see [what it changes](#what-it-changes) for what that implies.
- `spikes/s1-dxe3/build.sh` builds everything, `run.sh` runs thirteen scenarios through `tools/dosrun.sh` and compares each transcript and exit status with `spikes/s1-dxe3/expected/`, and `qemu-faults.sh` runs the two faults DOSBox cannot.
- The DJGPP libc documentation is not in the toolchain (only the cross compiler is), so the sources of knowledge were `sys/dxe.h`, `dxe.ld`, the help text and strings of `dxe3gen`, and experiments. Nothing here quotes the libc manual.

## What works: the commands

From `spikes/s1-dxe3/build.sh`, for one module (`<crate>` is `hello`, `counter` or `crash`):

```bash
# 1. Rust, as the skeleton does it (RUSTC_BOOTSTRAP=1 for -Zbuild-std; profile: opt-level s, lto, codegen-units 1, panic abort)
(cd spikes/s1-dxe3/modules && RUSTC_BOOTSTRAP=1 cargo build --release --locked)

# 2. The skeleton's converter, unchanged, then unpack the one object it leaves
tools/coff2go32.py modules/target/dos/release/lib<crate>.a obj/<crate>.a i586-pc-msdosdjgpp-ar
i586-pc-msdosdjgpp-ar x obj/<crate>.a

# 3. dxe3gen. It is in <djgpp>/i586-pc-msdosdjgpp/bin, not <djgpp>/bin, and finds dxe.ld through DJDIR.
export PATH=<djgpp>/bin:<djgpp>/i586-pc-msdosdjgpp/bin:$PATH
export DJDIR=<djgpp>/i586-pc-msdosdjgpp
dxe3gen -o HELLO.DXE -E _app_ -U ./<object>.o "$(i586-pc-msdosdjgpp-gcc -print-libgcc-file-name)"
```

- `-E _app_` exports only symbols that start with `_app_` (the C name `app_start` is `_app_start` in COFF); the module's own copy of `core` and `alloc` stays private. `-U` leaves the imports from the core unresolved until `dlopen`; without it `dxe3gen` refuses the module.
- The input must be an object, not the archive: given a `.a`, `ld -r` pulls a member only to satisfy an undefined symbol, nothing asks for it, and the result is a 96-byte module with no entry points that `dlopen` accepts. Forcing the member in with `-u` or `--whole-archive` produced a module with extra unresolved imports (`___register_frame_info`) that `dlopen` refused. With LTO the archive holds one object, so unpacking it is enough.
- libgcc goes in by path, because `dxe3gen`'s own `-L` covers only `$DJDIR/lib` and `-lgcc` is not found. It satisfies `___udivdi3`, which core's float formatter calls. `libc.a` can be added the same way to take `memcpy` and `memset` into the module instead of importing them (tested: the transcript is identical, the module grows 104 bytes).
- The host: `dlregsym(table)` once, then `dlopen("HELLO.DXE", RTLD_LAZY)` and `dlsym(handle, "_app_start")`. **`dlsym` takes the COFF name with its underscore**: `dlsym(h, "app_start")` returns null with an empty `dlerror` text. `SDK.md` did not say so.
- `dxe3gen` options seen in its help: `-o`, `-P` (dependency on another module), `-I` and `-Y` (import libraries for static binding, not tried), `-D` (description), `-E`, `-X`, `-U`, `-V`, `--show-dep`, `--show-exp`, `--show-unres`. There is no `--oformat` (`dxe.ld` sets `coff-go32`) and no `-nostdlib` (nothing from libc is linked unless it is named). `--show-unres` is the useful one: it lists what a module imports.

## Answers

### 1. Build

Yes. The exact invocation is above. Sections and relocations from rustc's output:

- The object has `.text$…` (72 sections in `counter`), `.rdata$…` (112), `.data` and `.bss`. With `panic = abort` there is no `.eh_frame`, `.xdata` or `.pdata`. The relocations are `REL32` (206, which `objdump` calls `DISP32` for go32) and `DIR32` (238). Nothing else appears.
- DXE3 needs nothing beyond the skeleton's converter: its REL32 addend patch and its section folding are both required, and `dxe3gen` is loud about the second. On the unconverted object it stops with `dxe_tmp.o: linker output file has more than one section`, because `dxe.ld` merges everything into one `.text` (`.bss` included) and has no pattern for `.rdata$…`. With `compiler_builtins` kept it floods warnings (`local symbol ___adddf3 has no section`), as the skeleton found.
- `dxe3gen` then writes one section, a table of exports, a table of unresolved symbols with the offsets to patch, and a table of absolute relocations; `DXE_MAGIC` is `DXE1` in ASCII (`0x31455844`) and the extended header is `dxe3_header` in `sys/dxe.h`.
- **Size**: see the table in question 7. `HELLO.DXE` is 7734 bytes and `CRASH.DXE` 8157; `COUNTER.DXE` is 22853 because core's float formatter is about 14 KB. That is small next to the 84 KB the `APP.INF` example in `FORMATS.md` allows.
- **LTO is mandatory.** Without it each module is about 193 KB (all of `core` and `alloc` objects come in whole), exports over a thousand symbols unless `-E` is given, and the module fails to load (`unresolved symbol: ___register_frame_info`).

### 2. Load and call

Yes. `life` runs the full lifecycle twice, with the module `dlopen`ed from disk each time:

```
host: round 1, route a
  window> app 1 opens 'Hello' 200x100 as window 1
  log> hello: start app=1 args='alpha beta' window=1 events=7
  log> hello[1]: event 8 kind=5 x=10 y=20
  log> hello[1]: event 9 kind=6 x=30 y=40
  log> hello[1]: event 10 kind=7 x=50 y=60
  log> hello: stop app=1 events=10 kept=3
host: unloaded HELLO (dlclose 0, reclaimed 0 blocks, 0 bytes)
host: round 2, route a
  window> app 1 opens 'Hello' 200x100 as window 2
  log> hello: start app=1 args='alpha beta' window=2 events=7
```

`DeskEvent` crosses by pointer and its fields arrive intact; the arguments cross as a pointer and a length; only integers, pointers and lengths cross. The `events=7` on the second round is a module static starting over (question 6).

### 3. Callbacks into the core, two routes

Both work, in every scenario. They are compared in [the next section](#the-two-routes-compared). The recommendation is route a.

### 4. Rust runtime in the module

- **`alloc` and floating point work.** `hello` keeps a `Vec<String>` built with `format!`; `counter` sums `f64` values and prints `sum=12.0 mean=4.000 sqrt=3.4641` with `{:.3}` and an x87 `fsqrt` in inline assembly (`counter: stop app=2 events=103 sum=12.0 mean=4.000 sqrt=3.4641`). The allocator forwards to the core's `desk_alloc` and `desk_free` (route a) or the table (route b). `desk_alloc` returns 8-byte aligned blocks; the allocator refuses a stricter alignment with null, as the core's does.
- **`compiler_builtins` does not come with the module, and does not come from the core either.** The converter drops its object, as in the skeleton. What the compiler then calls (`memcpy`, `memset`, and `memmove`, `memcmp` and `bcmp` for other code) is the C library's: imported from the core's table in route a, forwarded through the function table in route b, or linked from `libc.a` by path (above). `___udivdi3` comes from `libgcc.a`, linked into the module. Nothing else is needed for this code.
- **`core::fmt` and the rest of `core` and `alloc` are duplicated in each module.** That is most of the size of `hello`, and `counter` adds the float formatter. Two copies in memory is the price of not having a shared runtime; a shared one would mean exporting Rust's unstable symbols from the core, which is not worth it at these sizes.
- **Two Rust modules at once do not clash.** With LTO the object has only three global symbols (the entry points); everything else is local, so each module has its own `core` and `alloc` and its own statics. `two` loads `HELLO.DXE`, `COUNTER.DXE` and a copy `HELLO2.DXE`, interleaves their events and stops them in turn:

```
  log> hello[1]: event 8 kind=1 x=3 y=0
  log> hello[3]: event 8 kind=1 x=9 y=0
  log> hello[1]: event 9 kind=2 x=4 y=0
  log> counter: stop app=2 events=103 sum=12.0 mean=4.000 sqrt=3.4641
```

  Both `hello` instances are at event 8 after one event each, so the copy has its own state. `two global` does the same with `RTLD_GLOBAL`, and an LTO build with no `-E` at all (only the three entry points exist as globals anyway) behaves the same. Whether duplicate exports would clash was not reached: a non-LTO module exports over a thousand `core` and `alloc` symbols without `-E`, but it does not load (question 1).
- **One module loaded twice is one instance.** `dlopen` of a file that is already loaded returns the same handle (reference counted), so the statics are shared: the second `hello` continued at `events=9`. A separate instance needs a separate file. The Desk therefore keeps one instance per app, and an app that wants several windows opens several windows (`desk_window_new` takes the app handle).

### 5. Panics and crashes

- **A Rust panic is recovered.** `panic = abort` calls the handler, which formats `message (line N)` into a 160-byte stack buffer (no allocation on the failing path) and calls `desk_panic(ptr, len)`; the host's `desk_panic` stores the text and `longjmp`s to the `setjmp` in `mod_guarded`, the single frame through which every call into a module goes. The module's frames are discarded, the host marks it crashed, `dlclose`s it, frees the heap blocks it held and goes on. `panic` loads a module before the crash (`HELLO`, `COUNTER`) and one after (`HELLO2`), crashes `CRASH` twice and then shows all three still working:

```
  log> crash: holding 5 blocks, then panicking
host: module CRASH crashed: panic: panic with heap (line 60)
host: state crashed
host: unloaded CRASH (dlclose 0, reclaimed 6 blocks, 1560 bytes)
  log> hello[1]: event 9 kind=2 x=2 y=2
  log> crash: start app=4
host: module CRASH crashed: panic: deliberate panic number 1 (line 39)
  log> hello: start app=5 args='after' window=2 events=7
  log> hello[5]: event 8 kind=1 x=7 y=7
  log> counter: stop app=2 events=103 sum=18.0 mean=6.000 sqrt=4.2426
host: panic ok, the core kept running
```

  The panic message formatted with `core::fmt` reaches the core, and the six blocks the module held (five buffers and the `Vec` of them, 1560 bytes) are reclaimed from the core's per-module ledger. That is the reason to prefer `desk_alloc` to a bare `malloc`: the core can free everything a crashed app owned. `panic-b` is identical on route b. `panic` is also what the second load of `CRASH` shows: its counter restarted ("number 1").
- **A divide error is recovered.** `signal(SIGFPE, …)` with a handler that `longjmp`s to the same frame turns the CPU exception into the same path: `host: module CRASH crashed: fault: SIGFPE (289)`, then `hello` and a new `counter` carry on (`fault-div-catch`). Without the handler (`fault-div-raw`) DJGPP's default ends the whole program with exit status 255 after `crash: dividing by zero`. DJGPP's signal numbers are not Unix's (`SIGFPE` is 289, `SIGSEGV` 291).
- **A null write is recovered on a machine with a real page-fault path, and cannot be tested under DOSBox.** A write to address 0 (or any unmapped address) makes DOSBox Staging 0.83 abort itself: `ABORT: Pagefault didn't correct page`, exit status 125 from `tools/dosrun.sh`. A plain C program does the same with no module involved (`*(int*)0 = 1` and a read of `0x7fff0000` both do; a read of `0x1000` is fine, so only the first page is guarded). The likely cause is that DOSBox runs a page fault's handler to completion and expects the page to be mapped afterwards, while CWSDPMI reflects the fault to the program, which never returns there; this was not confirmed in DOSBox's source. Under QEMU 9.0.2 with FreeDOS 1.4 and the same `HOST.EXE` and CWSDPMI r7 (`qemu-faults.sh`), the signal handler catches it and the core recovers, like the divide error:

```
=== HOST.EXE fault null catch: exit status 0
host: module CRASH crashed: fault: SIGSEGV (291)
host: survived the fault, state crashed
host: fault null recovered, the core kept running
=== HOST.EXE fault null raw: exit status 255
```

  Without the handler DJGPP prints `Exiting due to signal SIGSEGV`, `Page fault at eip=…` and a register dump to the screen and exits with 255: the Desk dies. So **every handler-less fault kills the Desk, and recovery needs `signal` handlers installed by the core around every module call**. Verified in QEMU only, not on hardware.
- **A stack overflow cannot be recovered, and is worse than a crash.** In QEMU the recursion ends with CWSDPMI itself printing `Page Fault cr2=00400100 at eip=… esp=000022c2 … error=0004` and exiting with status 1, with or without the signal handler and before DJGPP prints anything, so no handler runs. The stack pointer had reached 0x22c2, below the stack range DJGPP reported in the register dump of the null-write run of the same program (0x20044 up to 0xa0040): the recursion had run through whatever lies beneath the stack in the program's one flat segment, which is core data and may be heap blocks holding module images, before it reached the guarded first page. The fatal error itself was not analysed further. Nothing the host does after the fact helps. The mitigations are all prevention: modules do not recurse without a bound, a stack check in debug builds, a generous `stubedit` stack for `DESKCORE.EXE`, and the SDK rule that a module's frames stay small. A fixed-size stack with a guard page for module calls (a separate stack allocated through DPMI with its first page not present) is a possible later hardening; it was not tried.
- **What cannot be caught, in short:** a stack overflow; memory corruption by a module that stays inside the program's address space (a wild write inside the heap is not a fault at all, since every module shares the core's single flat segment); an infinite loop in a module (there is no timer preemption; a watchdog would need an interrupt handler that `longjmp`s, and nothing here tried it); and anything that takes down CWSDPMI. The spec's "an app that panics is unloaded" is true for panics and, with handlers, for most CPU faults; it is not a sandbox.

### 6. Lifecycle and leaks

`leak 200` loads `HELLO.DXE`, runs it, stops it and `dlclose`s it 200 times. `_go32_dpmi_remaining_virtual_memory()`, `_go32_dpmi_remaining_physical_memory()` and `sbrk(0)` are read after round 1 and every 50 rounds and are identical to the byte:

```
mem: after 1      virtual=148750336 physical=15085568 brk=713960
mem: round        virtual=148750336 physical=15085568 brk=713960
mem: after all    virtual=148750336 physical=15085568 brk=713960
leak: virtual memory after round 1 versus after round 200: not lower (no growth)
leak: heap break after round 1 versus after round 200: not higher (stable)
```

- `leak-crash` does the same with a module that panics holding six heap blocks in every round; the core reclaims them each time (`reclaimed 6 blocks, 1560 bytes`) and memory is again unchanged.
- A control proves the measure can see a leak: `leak-control` leaks 4 KB per round on purpose and the break moves from 713960 to 1526824 bytes, reported as `HIGHER (leak)`; `tools/check.sh` fails if that is not reported.
- `dlclose` frees the module's image: 200 loads of a 7.7 KB module would move the break by 1.5 MB if it did not.
- **Static data re-initialises on reload**: `events=7` at the start of every round, from a `static EVENTS: AtomicU32 = AtomicU32::new(7)`, though the previous round ended at 10. It does not re-initialise on a second `dlopen` of a file that is still loaded (question 4).
- A failed `dlopen` leaves nothing behind: without the export table `dlopen` fails with `unresolved symbol: _desk_free`, and the same module loads after the table is registered (`noreg`).

Not measured: how long `dlopen` takes. DOSBox's `cpu_cycles = max` makes timing meaningless (`TOOLCHAIN.md`); a loading time needs a fixed cycle count and an estimate, or hardware.

### 7. Build ergonomics

| Step | Time in the CI image |
|---|---|
| Clean build of the three modules on both routes (two `cargo build`s with `-Zbuild-std`, six conversions and `dxe3gen`s, the host) | about 15 seconds |
| Rebuild after a change to one module | about 0.5 seconds |
| All thirteen headless scenarios | about 11 seconds |

Module sizes in bytes (route a; route b is within 200 bytes):

| Profile | `HELLO.DXE` | `COUNTER.DXE` | `CRASH.DXE` | Loads |
|---|---|---|---|---|
| `opt-level = "s"`, LTO (the default here) | 7734 | 22853 | 8157 | yes |
| `opt-level = "z"`, LTO | 9070 | 24901 | 9433 | built, not run |
| `opt-level = 3`, LTO | 8182 | 26841 | 8637 | built, not run |
| `opt-level = "s"`, no LTO | 193591 | 193059 | 194439 | no |
| `opt-level = 0`, no LTO | 662212 | 660124 | 665636 | not tried |

`opt-level = "s"` is the smallest with LTO; `"z"` is larger here, and speed was not measured. The size is dominated by `core::fmt` and `alloc` code that is duplicated per module, so an app that avoids `format!` and float printing is a few KB.

An **app template** needs: a crate with `crate-type = ["staticlib"]`, a dependency on the SDK crate (what `rt` is today), `src/lib.rs` with `desk_app!(start, event, stop);` and three plain functions, the shared `[profile.release]` (`opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`), a workspace `.cargo/config.toml` of eight lines naming `core/dos.json` and `build-std` (the template reuses `dos.json`, it does not copy it), and a build script that is the spike's step 2 (convert, unpack, `dxe3gen`). One workspace for all apps means `-Zbuild-std` runs once. Two lines in `APP.INF` (`module=`, `sdk=`) describe it to Programs.

## The two routes compared

| | Route a: imports by name, resolved by `dlregsym` | Route b: a table of function pointers to `app_start` |
|---|---|---|
| In the module | `extern "C" { fn desk_log(…); … }` | `static` table pointer set in `app_start`, a wrapper per call, `#[repr(C)] struct DeskApi` |
| In the core | an array of `{name, address}` given to `dlregsym` once | a `const` struct of function pointers |
| Imports in `--show-unres` | the `desk_*` functions and `_memcpy`, `_memset` (or none, with `libc.a` linked in) | none |
| Size of `HELLO.DXE` | 7734 bytes | 7932 bytes (the table and five forwarding functions) |
| `memcpy` and friends | imported, or linked from `libc.a` | forwarded through the table (the module cannot import) |
| A typo or a missing function | found at `dlopen`: `unresolved symbol: _desk_free`, naming one symbol; the module is refused and the core carries on | found at build time if the table is a typed struct |
| Adding a function | add a name to the core's table; old modules are untouched | append to the struct; old modules read the prefix by `size` |
| Version check | the `sdk=` key in `APP.INF`, checked before `dlopen`; a newer module on an older core is refused by name at load | `version` and `size` fields in the table, checked at run time, plus `sdk=` |
| Per-app permissions | one global table | a different table per app is possible |
| From C | ordinary declarations in `desk.h` | calls through the table, usually behind macros |
| Native testing of an app | link the SDK crate against stubs | pass a stub table |
| Failure seen in the spike | none specific to the route | none specific to the route |

Route a is simpler for the author (plain `extern "C"`, no state to bind, direct calls), matches the SDK's existing shape and works from C unchanged. Route b has no dependency on the loader's symbol resolution, a typed table that catches mistakes at build time, and room for per-app tables. They behaved identically in every test here. Route a is recommended because nothing in route b's list of advantages is needed yet; the risks of route a are covered by two build-time checks: every symbol `dxe3gen --show-unres` reports for a shipped module must be in the core's table (a CI step), and `libc.a` and `libgcc.a` pieces are linked into the module by path so the compiler's runtime never depends on the table. If DXE3's resolver proves unreliable on real hardware or another DPMI host, route b is a day's work.

## What failed on the way

- `dxe3gen: command not found`, then `Error: neither DXE_LD_LIBRARY_PATH nor DJDIR are set`. The tool sits beside the cross tools in `i586-pc-msdosdjgpp/bin` and finds `dxe.ld` through `DJDIR`.
- A 96-byte module that loaded without error: `dxe3gen` was given the archive and nothing pulled its member in. The run script now fails any module under 4000 bytes.
- `dlsym` returning null with no error text: the name needs the leading underscore.
- `unresolved symbol: ___udivdi3` from `counter` (the float formatter), fixed by linking `libgcc.a` into the module. `-lgcc` is not found; the path is.
- `unresolved symbol: ___register_frame_info` when giving `dxe3gen` the archive with `-u` or `--whole-archive`, or building without LTO. The cause was not found; an object plus `libgcc.a` does not have it.
- `rustc`'s `suspicious definition of the runtime memcpy symbol` lint on route b's forwarding functions: fixed by declaring them with `c_void` pointers.
- A DOSBox abort (`Pagefault didn't correct page`, exit status 125) on any page fault, shown first by the null-write scenario. It is DOSBox's, not the module's: a C program without modules does it. The two faults moved to QEMU.
- `mallinfo` does not exist in DJGPP; the memory check uses `sbrk(0)` and the DPMI figures.

## What it changes

**`SPEC.md`**

- The **Risk** paragraph no longer says this is the first spike with a fallback to apps built into the core; it records the result, the limits and what is still open. The milestone text says the spike is done for the build, load, call, panic and leak questions, with a page-fault and hardware check left.
- "An app that panics is unloaded" stays, and now says how: the core wraps every call in `setjmp`, a panic and (with handlers) most CPU faults land in one recovery path that unloads the module and frees what it held. A stack overflow is not recoverable.

**`SDK.md`** (corrected, not just extended)

- The core does not "export" the SDK to a linker; it **registers** the `desk_*` functions with `dlregsym`, and a module imports them by name. The COFF symbols have a leading underscore, so the module's entry points are `_app_start`, `_app_event` and `_app_stop`, and `dlsym` takes that spelling.
- New functions: `desk_alloc(size)` and `desk_free(ptr)` (the app heap, tracked per app so a crash frees it), `desk_log(text, len)` and `desk_panic(msg, len)` (called by the panic handler, never returns), plus a note that `memcpy` and friends are the C library's.
- A module that needs a symbol the core does not provide is refused at load with the name of the first missing one.
- A new section on building and loading an app, and a new rule: no unbounded recursion, small frames, because a stack overflow cannot be recovered.

**`FORMATS.md`** (`APP.INF`)

- `module=` names a `.DXE` loaded by path from the app's folder; `sdk=` is checked by the core before `dlopen`, so a module that needs a newer core is refused with a message instead of by an unresolved symbol. One instance per app (a file already loaded is shared), so a copy under another name is a different app.

**Repository layout**

- `apps/` is one Cargo workspace for the DOS target (an `apps/.cargo/config.toml` naming `core/dos.json`, a shared `[profile.release]`), one crate per app (`apps/notes/`, `apps/midi/` …) with its `APP.INF` and icons beside `Cargo.toml`. A script in `tools/` generalises the spike's step 2 into `tools/build-app.sh <app>`.
- `sdk/` holds the Rust crate that is today's `rt` (allocator, panic handler, `desk_app!`, safe wrappers over the `desk_*` imports) and `include/desk.h` for C apps. The core keeps the export table (`dlregsym`), the recovery frame and the per-app ledger.
- The core is Rust, but `setjmp` and `longjmp` are not safe to call from Rust (they return twice and skip frames), so the recovery frame belongs in the C layer next to the skeleton's `c/panic.c`: a `desk_call_guarded(fn, …)` shim the Rust core calls, and `desk_panic` defined in C. The core's frames are never between a module and its recovery frame, so skipped frames are the module's.
- `AGENTS.md` no longer says the layout of `apps/` is waiting for the first spike.

## What was and was not verified

Verified, in `tools/check.sh` (headless DOSBox Staging 0.83.0, CWSDPMI r7, in the `ci-dos` image): build, `dlopen`, `dlsym`, a full lifecycle on both routes, two modules plus a copy, the same file twice, a panic with the modules before and after it unaffected, a divide error recovered by a signal handler and killing the program without one, 200 loads with stable memory (also with a panic in each), a deliberate leak noticed, and a refused load without the export table.

Verified by hand under QEMU 9.0.2 with FreeDOS 1.4 and CWSDPMI r7 (`spikes/s1-dxe3/qemu-faults.sh`, not in `tools/check.sh`): a null write recovered by a `SIGSEGV` handler, its fatal outcome without one, and the unrecoverable stack overflow.

Not verified:

- Anything on real hardware or on a real 486. QEMU emulates a faster, newer CPU; the stack-overflow behaviour in particular should be repeated on hardware or another DPMI host before relying on the description.
- DPMI hosts other than CWSDPMI r7, or other DOS versions (FreeDOS 1.4 was the second).
- How long `dlopen` takes, and the cost of the per-call `setjmp` (small, not measured).
- Modules with frames over 4 KB (`_alloca`; `TOOLCHAIN.md` lists it for the core), modules that hook interrupts, C modules, `-Y` import libraries and static binding, `dxe3res`, and the `-D` description string.
- A module loop that never returns (a watchdog was not tried).
- Non-LTO builds loading (they do not, for a reason not found) and `opt-level` 3, `"z"` and 0 runs (built and sized, not run).
- The GitHub Actions run of the new steps.

## Reproducing it

```bash
tools/setup.sh
spikes/s1-dxe3/build.sh                 # dist/s1/HOST.EXE, the .DXE modules and CWSDPMI.EXE
spikes/s1-dxe3/run.sh                   # thirteen scenarios against spikes/s1-dxe3/expected/; --update rewrites them
tools/dosrun.sh dist/s1 HOST.EXE panic  # one scenario by hand; also life, two, fault div catch, leak 200 …
spikes/s1-dxe3/qemu-faults.sh           # the faults DOSBox cannot run; needs Docker and network access
tools/check.sh                          # all of it, with the rest of the checks
```

`OPT=3 LTO=true spikes/s1-dxe3/build.sh` rebuilds with another profile for the size table.
