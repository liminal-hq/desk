# core

The shell, in Rust (`no_std` with `alloc`): window manager, compositor, palette, event loop, module loader and the handoff. It becomes `DESKCORE.EXE`. Today it is the walking skeleton's seed: one function, `desk_main`, that builds a result line from a `Vec`, a `String`, 64-bit integer maths and an x87 square root, plus the global allocator (DJGPP's `malloc` and `free`) and a panic handler that reports through C.

| File | What it is |
|---|---|
| `dos.json` | The custom i486 target specification; `docs/TOOLCHAIN.md` explains each field |
| `.cargo/config.toml` | Makes `dos.json` the default target and builds `core`, `alloc` and `compiler_builtins` from `rust-src` (`-Zbuild-std`) |
| `Cargo.toml` | A `staticlib` in its own workspace, so the DOS target does not apply to the rest of the repository |
| `src/lib.rs` | The C ABI exports, the allocator and the panic handler |

Build it with `tools/build.sh`, which sets `RUSTC_BOOTSTRAP=1` for the unstable cargo options and converts the objects for DJGPP; a bare `cargo build` in this directory also works with `RUSTC_BOOTSTRAP=1` but gives an archive that DJGPP's linker cannot use correctly. Only integers, pointers and lengths cross the boundary to C (`docs/SDK.md`).
