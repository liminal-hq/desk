# Rust/DOS integration: methodical analysis

Assessment date: 2026-10-24. Scope: the repository at the start of this assessment, covering the hosted integration layers, their native counterparts where they share contracts, and the plans that connect them. This is an architectural and source analysis, not a hardware certification or a performance measurement. Recommendations below do not change the project direction or establish new requirements by themselves.

## Assessment method

The analysis follows each boundary from build input to shutdown: target specification, linker, executable packaging, protected-mode entry, DPMI services, DOS calls, allocation, interrupts, display and input, shared shell, module loading, standard-library support and DOS-program handoff. For each boundary it asks who owns the resource, which representation crosses it, how failure is reported, how state is restored, and what evidence exercises the contract.

Evidence has three levels: **observed** means directly visible in source or scripts; **recorded** means an earlier spike documents a run that was not repeated here; **inferred** means a consequence of the design that needs a targeted experiment or language-level audit. A successful transcript demonstrates the path exercised; it does not establish soundness for all callers or compatibility with other hosts.

The primary sources are [Direction](DIRECTION.md), [Toolchain](TOOLCHAIN.md), [D0](D0-RUST-DOS.md), [DXE3](S1-DXE3.md), [the HAL](HAL.md), [DOS input](B-DOS-INPUT.md), [the first window](FIRST-WINDOW.md), [standard-library support](E-STD-STUBS.md), [the SDK](SDK.md) and [handoff](HANDOFF.md). Source and executable check scripts take precedence over status summaries. No external implementation or reference-project code was used.

## Overall assessment

The strongest result is the shared shell's separation from its environment. Both the DOS adapter and the kernel use the same shell and presentation crates, and the repository records matching frames at both colour depths. DOS is already a useful development platform for the native system, rather than just an obsolete dependency to remove.

The largest gap is composition. The Rust-only runtime, the C-hosted module loader and the experimental `std` platform each work in their own setting, but there is no demonstrated product path combining their lifecycle and failure contracts. Promoting `rt` is worthwhile; deleting the C path before replacing the loader and recovery services would remove capabilities that the Rust-only path has not demonstrated.

Optimisation should start with bounded resource use and device traffic. A smaller compiler pipeline is useful, but it does not by itself make the shell faster on a 486. The next useful measurements are presentation traffic, input latency during blocking work, heap retention, and the cost of real-mode service calls.

## What exists and how it fits

| Layer | Observed implementation | Evidence and remaining boundary |
|---|---|---|
| Original hosted executable | [Rust core](../core/src/lib.rs), [C entry](../c/main.c), [build script](../tools/build.sh), MinGW-flavoured COFF conversion and DJGPP linking | A walking skeleton with explicit normal and panic transcripts; it remains the `DESK.EXE` built by the main build script |
| Rust-only DOS executable | [D0 runtime](../spikes/d0-rust-dos/rt/src/lib.rs), freestanding ELF, `rust-lld`, [ELF packer](../tools/elf2go32.py), DJGPP stub and CWSDPMI | A working route used by subsequent graphics, input and shell consumers; the runtime still lives under `spikes/` |
| Machine services | [DPMI wrappers](../spikes/d0-rust-dos/rt/src/dpmi.rs), [DOS file calls](../spikes/d0-rust-dos/rt/src/dos.rs), [DOS HAL](../spikes/d0-rust-dos/rt/src/hal.rs), [IRQ adapter](../spikes/d0-rust-dos/rt/src/irq.rs) | Real implementations exist; compatibility evidence is concentrated on CWSDPMI and DOSBox |
| Shared shell | [Shell workspace](../shell/Cargo.toml), [DOS wiring](../shell/dos/src/main.rs), [kernel wiring](../kernel/src/shell.rs) | Shared deterministic behaviour, damage accounting and platform frame comparison; no complete app-host integration |
| Loadable apps | [DXE3 host](../spikes/s1-dxe3/host/mod.c) and [module runtime](../spikes/s1-dxe3/modules/rt/src/lib.rs) | Load, call, unload, panic and leak scenarios exist in a separate DJGPP/C host; no equivalent loader in the Rust-only executable |
| `std` services | [Patched platform ABI](../spikes/e-std-stubs/patch/overlay/library/std/src/sys/pal/desk/abi.rs) and [DOS shim](../spikes/e-std-stubs/shim/src/desk.rs) | Useful subset recorded under DOS; ordinary Rust start-up, files and native implementation remain incomplete |
| Launcher and public SDK | [Launcher directory](../launcher/README.md), [SDK directory](../sdk/README.md) | Descriptions only; their implementations remain in plans or spikes |

### Build and addressing concepts

The original route solves object-format compatibility: LLVM's PE-flavoured COFF is repaired into the form DJGPP accepts. The Rust-only route resolves symbols and relocations in ELF first, then packages the finished image for the stub. These are different operations. The ELF packer is simpler because it inherits a narrower input contract, not because it solves arbitrary linking.

Likewise, Rust-only does not mean independent of DOS infrastructure. The stub supplies the initial selectors and host discovery; CWSDPMI supplies protected-mode services. The runtime extends the data selector and converts linear addresses to offsets relative to the image base. Pointer-taking DOS calls copy through conventional memory and simulate a real-mode interrupt. A protected-mode pointer is therefore not interchangeable with DOS `segment:offset`, even when both fit in 32 bits. [D0](D0-RUST-DOS.md) records a negative experiment demonstrating that distinction.

Keep these contracts explicit when extracting the runtime. Removing the C compiler from the active executable pipeline is a reasonable objective. Replacing CWSDPMI would introduce a substantially different compatibility project, without a demonstrated need here.

## Gaps and failure contracts

### 1. Runtime migration lacks an integration acceptance gate

**Observed:** the first window uses `rt` by path, while DXE3 still uses DJGPP's resolver, C library, signal handling and `setjmp`/`longjmp`. [The main check script](../tools/check.sh) checks these paths separately. It does not load an app into `SHELL.EXE`.

**Consequence:** the spikes prove component feasibility, not that the replacement runtime can host the planned app system. The native module format is also unresolved, so choosing an ELF executable target does not settle the app format.

**Recommendation:** define the next vertical slice as one app loaded into the shared shell, calling a real SDK function, receiving events, stopping and unloading, with a second app surviving an approved failure scenario. Decide whether to retain a small C integration shim, replace DXE3 services, or adopt a different loader through that slice. Retire each legacy service only when an equivalent scenario passes in its replacement.

### 2. Clean-up registration is fallible but often treated as guaranteed

**Observed:** [the clean-up list](../spikes/d0-rust-dos/rt/src/cleanup.rs) has eight slots and returns `false` when full. The display adapter, timer hook, basic VGA helper and several shell registrations ignore that result. The general IRQ adapter already checks it before installing its vector.

**Consequence:** under saturation, an acquisition can succeed without an exit action. This is a source-level failure path; the existing ordinary exit and panic checks do not establish behaviour with a full list. `timer::unhook` also discards a vector-restoration error and clears its installed state; the general IRQ adapter preserves its state when restoration fails.

**Recommendation:** make acquisition transactional: reserve the restoration action, acquire the resource, and roll back on failure. Reuse the IRQ adapter's treatment of restoration errors. Explicitly test a full list, failed acquisition and failed restoration. Increasing the slot count alone leaves the contract unresolved.

**Additional observed mismatch:** registration fills the first vacant slot and execution walks slots backwards. After removing an early entry and registering a new one, execution is no longer strictly reverse registration order. Either enforce chronological LIFO or document the actual ordering and keep dependencies out of slot reuse.

### 3. Framebuffer validity outlives the API's ownership model

**Observed:** [the `Display` trait](../hal/hal/src/video.rs) returns an owned `FrameBuffer` without a borrow tying it to the display. [The DOS implementation](../spikes/d0-rust-dos/rt/src/hal.rs) stores one live VBE mapping globally and unmaps it on restoration or a subsequent VBE mode change. A caller can retain the previous region and continue to call its safe methods.

**Consequence, inferred:** the public safe interface does not enforce the mapping lifetime promised by [the unsafe `MmioRegion` constructor](../hal/hal/src/mmio.rs). The current single-screen consumer follows a narrower discipline, but future mode switching or multiple display objects could invalidate a retained region.

**Recommendation:** give the active mode an exclusive owner that also owns the mapping, or borrow its framebuffer through a mode session. Mode changes must consume or invalidate access before unmapping. Add an API-level check showing that access cannot survive restoration, together with runtime checks for repeated mode changes. This needs resolution before exposing mode switching to apps.

### 4. Panic recovery is narrower than app isolation

**Recorded:** [the DXE3 result](S1-DXE3.md) demonstrates panic recovery and selected CPU faults, while documenting unrecoverable stack overflow, undetected writes within the shared address space and infinite loops. The host's per-app allocation ledger is useful resource accounting; it is not a memory boundary.

**Inferred integration risk:** the recovery guard must remain valid when SDK calls enter a Rust host. A C guard below the module does not by itself ensure that a later fault or panic cannot abandon a Rust SDK callback, a borrow, a lock or partially updated host state. Successful execution of the spike does not settle the language-level validity of every non-local exit.

**Recommendation:** audit the exact Rust/C control flow before carrying this mechanism into the shell. Define which frames may be abandoned, forbid re-entry unless it has an explicit model, and defer callbacks or events where needed. Treat hardware faults and Rust panics as separate contracts. Keep the documented trusted-module limitations visible when deciding the native process model.

### 5. The two `desk_*` interfaces are not yet one ABI

**Observed:** [the SDK](SDK.md) describes `desk_alloc(size)` with host ownership tracking, while the experimental `std` ABI declares `desk_alloc(size, align)` and layout-aware deallocation. The DXE3 module allocator refuses alignments above eight bytes. The names overlap while the signatures and ownership models differ.

**Consequence:** these contracts cannot simply be linked together under the same symbol names. Adding `std` to a loadable app also introduces system allocations and cached state whose owner must be identified for unload and crash reclamation.

**Recommendation:** separate the namespaces or define one deliberately versioned service contract before combining the experiments. Specify integer widths, alignment, ownership, failure values, buffer validity and calling convention. Add ABI checks for event layouts and version negotiation. The scalar/pointer convention avoids struct-by-value disagreements, but does not remove layout obligations for structs behind pointers.

### 6. The packer relies on linker discipline more than it validates it

**Observed:** [the packer](../tools/elf2go32.py) checks basic ELF identity, executable type, machine, load-segment count, text memory size and some placement constraints. It does not comprehensively validate file-backed segment bounds, program-header bounds, segment permissions or all size/address arithmetic before slicing and packing.

**Probe run in this assessment:** a synthetic ELF whose load segment claimed bytes at file offset 4096 in an 84-byte input was accepted by `parse_elf`. A second input with `p_filesz > p_memsz` was also accepted by that parser. The executable path has an additional text-tail check, so parser acceptance alone is not evidence that every malformed input reaches a runnable output. It does demonstrate that the parser's contract is permissive.

**Recommendation:** validate the supported ELF subset explicitly and reject truncated or inconsistent inputs with a clear diagnostic. Add small malformed-image fixtures and a normal image round trip when hardening this tool. This protects against linker-layout drift and damaged inputs; it does not require turning the packer into a general ELF implementation.

### 7. Filesystem and handoff semantics remain product work

**Observed:** DOS file operations cover basic reading, creation, seeking and removal. `put_path` copies UTF-8 bytes directly and does not reject embedded NULs. The planned SDK uses UTF-8, while filename encoding and DOS-compatible naming need an explicit policy. `read_to_vec` allocates the reported whole file length, so its `chunk` argument bounds each read rather than total memory consumption.

**Recommendation:** define path validation, encoding conversion, bounded reads, directory enumeration and error mapping before building Files or broad `std::fs` support. Exercise disk-full and partial-I/O behaviour as well as successful transfers.

**Observed:** [handoff](HANDOFF.md) describes a process exit and restart, but the launcher is unimplemented. Serialization, conventional-memory release, failed execution and stale `DESK.RUN` recovery have no demonstrated end-to-end path.

**Recommendation:** treat handoff as a state machine with durable checkpoints. Specify when the run request becomes valid, how it is consumed, how execution failure is recorded and how restart avoids replay loops. Prove a small launcher/core round trip before generalising app session restoration.

### 8. Status documents lag behind implemented consumers

**Observed:** the opening status in [HAL](HAL.md) says the runtime and kernel are unchanged and neither real platform has run. The code and later spike reports show both platforms in use. [The target matrix](RUST-TARGETS.md) retains statements that the pure-Rust route is planned and shared crates have no binary consumers, alongside newer rows describing those consumers. [Direction](DIRECTION.md) also retains early milestone wording.

**Recommendation:** reconcile these status summaries in a focused documentation update, distinguishing historical spike results from current capability. Preserve the useful historical evidence, but give each capability one current status and its acceptance command. The discrepancies are recorded here rather than silently changing architectural decisions during an analysis.

## Optimisation opportunities and measurement design

| Opportunity | Existing basis | Next measurement and acceptance condition |
|---|---|---|
| Reduce framebuffer traffic | Damage tracking and cursor overlays already avoid full repaint; `FrameStats` counts repainted and presented pixels | Record full redraw, cursor movement, dragging and text entry separately; preserve identical output and reduce bytes or device operations for the same scenario |
| Improve linear copies | `LinearPresent` calls `MmioRegion::write_bytes`, which uses volatile byte stores; `fill8` already uses aligned word stores | Compare a framebuffer-specific aligned word or assembly copy against the byte baseline, including short spans, tails and nontrivial pitch; validate read-back at both depths |
| Reduce planar register traffic | `PlanarPresent::span` converts a row and selects four planes for each span | Count plane selections and bytes before trying batching; include conversion work and temporary memory, since fewer register writes can cost extra RAM or passes |
| Bound idle and blocking latency | The DOS interactive loop continuously polls input and presents; `std` sleep busy-waits | Measure idle service calls, longest poll gap and key-to-frame delay during file reads, loading and drawing; choose a DOS/DPMI-compatible wait strategy only after checking host behaviour |
| Improve heap predictability | First-fit coalescing allocator with 256 KiB minimum growth and no release to the host before exit | Track live bytes, reserved bytes, largest free block, scan length and high-water mark across repeated app lifecycles; consider arenas or reusable buffers before replacing the allocator |
| Reduce DOS I/O overhead | A shared 32 KiB conventional-memory buffer batches larger transfers | Compare call counts, copied bytes, memory footprint and latency for realistic file sizes; retain segment-boundary and partial-transfer correctness |
| Reduce code and build duplication | Multiple workspaces rebuild custom-target libraries; each DXE carries Rust library code | Measure clean and incremental builds plus executable and resident sizes; retain the recorded `opt-level = "s"` baseline until alternatives win on actual workloads |

At 640 × 480 an 8-bit RAM back buffer is 307,200 bytes, or 300 KiB, even when presenting to four VGA planes. The physical planar image is 153,600 bytes across those planes. These are derived storage costs, not measured process footprints. Add fonts, stacks, runtime image, heap slack and modules before comparing the product with the [8 MB machine target](SPEC.md). File size alone is an inadequate memory budget.

Keep correctness runs and timing runs distinct. DOSBox's maximum-cycle setting is useful for regressions but cannot establish a real 486 frame rate. Use reproducible scenarios and fixed emulator settings for comparisons, report the exact environment, and reserve hardware claims for hardware measurements. Keep read-back, capture and full-frame digest costs separate from normal presentation costs. Wider CPU stores also do not guarantee proportionally fewer physical bus cycles on every VGA card.

## Reflection on the architectural concepts

**The HAL is valuable because it exposes meaningful differences.** DPMI interrupt simulation and a native BIOS trampoline are two ways to obtain a BIOS service, with different blocking and interrupt constraints. Shared interfaces should preserve those constraints where callers need them. Sharing a driver algorithm is a useful result even when the platform resource lifecycle differs.

**Determinism is an architectural asset.** The shared shell accepts events and produces a frame without reading time or randomness internally. That supports replay, differential comparison and optimisation against a stable reference. Future clocks, filesystem completions and app events should enter as explicit inputs so this property survives.

**Ownership is broader than Rust allocation.** Interrupt vectors, PIC masks, low-memory transfer buffers, display mappings and app handles all have lifetimes and restoration obligations. `unsafe` concentrated in a platform module helps review, but safe outward methods must still preserve those obligations. The framebuffer and clean-up findings are examples of a resource contract that types or transactional acquisition can express more strongly.

**Cooperation creates a latency obligation.** A trusted app that eventually returns can still make the desktop feel stuck through long drawing or I/O. Small bounded tasks, deferred work and explicit event budgets fit the current design. Preemption and address-space isolation are separate decisions with substantial native-kernel consequences.

**A useful `std` subset is a compatibility promise.** The experiment shows that familiar APIs can run over a small platform layer. It does not make unsupported files, processes or threads available. Ordinary `fn main`, buffered-output shutdown, error semantics and Rust-upgrade testing belong in that promise before advertising general crate compatibility. Keep the shell and low-level runtime on `no_std` as planned.

**Extraction should follow stable consumers.** The DOS shell and a small standalone DOS program are good consumers for an eventual independent runtime. Extracting after lifecycle contracts and build inputs stabilise avoids publishing the spike directory's incidental structure as an API. Neither a new extender nor a general module framework is required to establish that boundary.

## Recommended sequence and completion criteria

1. **Close lifecycle gaps:** make clean-up acquisition fallible throughout, preserve failed-restoration state and enforce framebuffer validity. Complete when saturation, rollback and mode-lifetime checks demonstrate the intended contracts.
2. **Reconcile current architecture:** update the HAL and target status summaries, name the active hosted executable path and document which C services remain required. Complete when each claimed capability points to an implementation and acceptance command.
3. **Build one combined app slice:** resolve the loader decision, recovery audit and overlapping ABI names through a single real shell app. Complete when normal lifecycle, rejected ABI, repeated unload and a supported app failure leave the shell and another app usable.
4. **Measure representative workloads:** record RAM high-water marks, presentation traffic, service-call counts and input latency before selecting copy, allocator or scheduling optimisations. Complete when results are reproducible and compared against the current implementation.
5. **Prove hosted handoff:** implement the minimal real-mode launcher with request and resume semantics. Complete when success, failed execution and restart after interruption follow a documented state machine.
6. **Broaden compatibility deliberately:** repeat selected runtime and restoration scenarios with FreeDOS under a PC emulator and another relevant host configuration; add hardware when available. Advance `std` startup, filesystem support and upgrade checks as a separate workstream with an explicit supported-API list.

## Validation performed for this assessment

Source and plan inspection covered the build scripts, target specifications, linker script, packer, runtime services, HAL memory and display contracts, shell adapters, DXE3 guard and SDK, standard-library shim, launcher plan and check orchestration. The synthetic ELF parser probes described above ran successfully and confirmed the stated acceptance behaviour.

`python3 -I tools/check-docs.py` passed for repository Markdown, including this report and its index link. `git diff --check` passed for tracked changes, and `git diff --no-index --check /dev/null docs/RUST-DOS-INTEGRATION-ANALYSIS.md` passed for the new report. No implementation was modified and no new permanent test suite was added for this report.

The full `tools/check.sh` gate, DOS builds, emulator scenarios, browser checks and timing experiments were not run in this assessment. The checkout has no `vendor/` DOS tools, and installing them was unnecessary for the requested source analysis. Earlier spike results are attributed as recorded evidence, not fresh verification. Nothing here establishes real-hardware behaviour or audible output; audio cannot be checked headless.
