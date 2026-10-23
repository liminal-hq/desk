# sdk

The interface apps use to talk to the core: `include/desk.h` (the C ABI) and a Rust crate that wraps it. See `docs/SDK.md`.

Nothing is here yet. Spike 1 settled that optional apps are DXE3 modules (`docs/S1-DXE3.md`), so this directory will hold the Rust crate an app depends on (the allocator that forwards to `desk_alloc`, the panic handler that calls `desk_panic`, the `desk_app!` macro and safe wrappers over the `desk_*` imports) and `include/desk.h`; the spike's `rt` crate is its seed.
