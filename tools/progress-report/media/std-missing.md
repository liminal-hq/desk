| Missing | Where | Why it fails |
|---|---|---|
| `System: GlobalAlloc` | `sys/alloc/mod.rs` | `cfg_select!` has no `_` arm: ``none of the predicates in this `cfg_select` evaluated to true``, then 14 errors for the missing impl |
| `decode_error_kind`, `errno`, `error_string`, `is_interrupted` | `sys/io/error/mod.rs` | no `_` arm |
| `thread_local::key::{Key, LazyKey, get, set}` | `sys/thread_local/mod.rs` | without `target_thread_local` the fallback is the OS-key implementation, which exists only for unix, windows, sgx, xous and motor; the `no_threads` implementation is selected by an OS list (wasm without atomics, uefi, zkvm, trusty, vexos) |
| `sys::fill_bytes` | `sys/random/mod.rs` | no `_` arm (there is one for wasm-unknown, xous and vexos that supplies `hashmap_random_keys` too) |
