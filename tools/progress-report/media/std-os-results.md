| `os` | Result |
|---|---|
| `unknown` | Refused by rustc before any build: ``error loading target specification: `unknown` os can only be used on particular targets; use `none` for bare-metal targets`` |
| `none`, `desk`, `dos` | `std` fails: **21 errors**, the same for all three |
| `desk` with `"has-thread-local": true` | 18 errors (the thread-local key errors go; the other three gaps remain) |
| `desk` with `"target-family": ["unix"]` | Fails earlier, in the `libc` crate (``unresolved import `unistd```): `libc` does not know the OS |
| `desk` with `"target-family": ["wasm"]` | Fails in `core`: 281 errors, ``the feature named `..` is not valid for this target`` |
| `uefi` | The specification is refused: ``is_like_windows must be set if and only if os is windows, uefi or cygwin`` (needs a PE target) |
| `xous`, `vexos` | `std` fails (`dlmalloc` for `xous`; unknown token in inline assembly for `vexos`) |
| `zkvm`, `trusty` | **`std` builds unmodified** in about 11 s |
