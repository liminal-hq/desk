# Tasks, preemption, threads and wait and wake: the build plan

The task-by-task plan for the native track's first stage of [`APP-MODEL.md`](APP-MODEL.md): roadmap R77 stage 1 (tasks, threads and preemption in ring 0), R141 (threads and the one wait and wake primitive) and R142 (the scheduler's priority policy), plus `desk-sync`, the SDK's locks and channels as library code. It starts with an audit of what in today's kernel is and is not safe once a thread can lose the CPU at any instruction, then four tasks: K1, K2, K3 and Y1. Each task is one pull request, sized for one builder (a Sonnet agent or a person) working from this document, the design and the files it names.

> **Status.** A plan, written on 2026-11-11 against `main` at `68b5a26`. No task has started and nothing here is built or measured; every size and every timing is an estimate and says so. The API sketches are the design's; a builder may change a signature when the code shows a better one, and says so in the pull request and in [`APP-MODEL.md`](APP-MODEL.md) in the same branch. Nothing here has run on real hardware, and none of it will in these tasks: the evidence is QEMU as a 486 and host tests.

## Contents

- [Decisions this plan builds on](#decisions-this-plan-builds-on)
- [Scope](#scope)
- [The preemption-safety audit of the existing kernel](#the-preemption-safety-audit-of-the-existing-kernel)
- [Shape of the code](#shape-of-the-code)
- [Order and parallelism](#order-and-parallelism)
- [Coordination with R134](#coordination-with-r134)
- [Rules for every task](#rules-for-every-task)
- [K1. Tasks, threads and preemption in ring 0](#k1-tasks-threads-and-preemption-in-ring-0)
- [K2. Wait and wake, sleep, spawn, exit and join, thread-local keys, lazy x87](#k2-wait-and-wake-sleep-spawn-exit-and-join-thread-local-keys-lazy-x87)
- [K3. Ageing, ceilings and basic priority inheritance](#k3-ageing-ceilings-and-basic-priority-inheritance)
- [Y1. desk-sync: the SDK's locks and channels](#y1-desk-sync-the-sdks-locks-and-channels)
- [What R80b later changes](#what-r80b-later-changes)
- [New checks for the CI stack](#new-checks-for-the-ci-stack)
- [Questions for the owner](#questions-for-the-owner)
- [Definition of done](#definition-of-done)

## Decisions this plan builds on

Besides the twenty decisions in [`APP-MODEL.md`](APP-MODEL.md#decisions), two made by the owner on 2026-11-11 shape this plan:

1. **R80 is two named milestones.** **R80a** is the first OS milestone at stage 1: preemption in ring 0, apps as tasks, **unprotected**, and labelled so; **R80b** is the protected milestone: paging, ring 3 and the system-call boundary. Reason: the app does not change between the stages by design, so the milestone that proves the model can come before the one that proves protection, as long as the label is honest. K1 to K3 and Y1 are the foundation of R80a; [What R80b later changes](#what-r80b-later-changes) says what stage 2 and 3 do to them.
2. **The float policy (R138).** The ban on floating point stays in `shell/`, in the HAL drivers and in the core's integer-only rules, while apps and Fizz-style game code may use `f32` and `f64`. So the kernel saves and restores the x87 state **per thread, lazily** through `#NM` (K2), interrupt handlers never touch the x87, and the kernel's own code stays float-free. Reason: games and some apps want floats and the 486DX has the x87, but the shared core keeps its frames a function of integer steps, and laziness means a thread that never uses the x87 pays nothing.

## Scope

In scope: tasks and kernel threads in the kernel's flat ring-0 address space; a software context switch; PIT preemption at 100 Hz with a preemption-disable counter; 32 ready queues and a bitmap; an idle thread; killing a task; `desk_wait` and `desk_wake`; sleep and timeouts; thread spawn, exit and join; thread-local keys; lazy x87 saving; ageing, ceilings and basic priority inheritance; and `desk-sync` (mutex, condition variable, semaphore, read-write lock, `Once`, channel) on both the kernel and a deterministic host model.

Not in scope: applications as files run as tasks (R144, A2), the window system as a task (R143, G2), the first real SDK (R79, A1), hosted green threads (R141 hosted, H1), paging, ring 3 and system calls (R78, R77 stages 2 and 3, R80b), shared buffers, messages, and `std` (R146). The calls built here are kernel functions with the SDK's names and semantics; A1 and A2 bind them to import slots.

## The preemption-safety audit of the existing kernel

Today's kernel assumes one flow of control plus interrupt handlers. Stage 1 adds a second kind of concurrency: a thread can be preempted at any instruction where interrupts are on and the preemption counter is zero, and another thread runs kernel code meanwhile. On one CPU a preemption can only start from an interrupt, so **every section that runs with interrupts off is already safe from preemption**; what needs a look is state touched with interrupts on.

| Item | What it is | Under preemption | Stage 1 change |
|---|---|---|---|
| `idt.rs`: `static mut IDT` | The 256 gates | Written by `init` before interrupts are on and by `set_irq_gate` with interrupts off; read only by the CPU | None. K2 adds the `#NM` gate (vector 7) in `init` |
| `idt.rs`: `exception_handler` | The fatal dump | Entered through interrupt gates (IF clear), never returns | None, apart from K2 routing vector 7 to its own handler |
| `hal.rs`: `IRQS` (`UnsafeCell<[Option<IrqHandler>; 16]>`) | The IRQ handler table | Writers inside `without_interrupts`; readers in interrupt context or with interrupts off (`run_handler`) | None |
| `hal.rs`: `irq_dispatch` | Handler, then end-of-interrupt | Runs with IF clear | K1: after the end-of-interrupt, a call to the scheduler's interrupt-exit hook, which may switch threads |
| `hal.rs`: `PORTS`, `PIC`, `SERIAL`, `CLOCK` | Stateless port wrappers and an atomic tick counter | The PIC's mask read-modify-write is always inside `without_interrupts` (`pic.rs`, `register`, `unregister`); the clock is atomics | None |
| `pit.rs`: the IRQ 0 handler | `CLOCK.counter()` registered with `from_shared` | Counts ticks only | K1: a kernel-side service that counts the tick and then runs the scheduler's tick (slice accounting; sleepers from K2). No HAL change |
| `sync.rs`: `without_interrupts` | `pushfd; cli`, restore IF | Still correct and still the tool for state an interrupt also touches; on one CPU it also excludes preemption | K1 adds the preemption counter beside it. New rule: only the scheduler calls the switch with interrupts off; a resumed thread restores its own IF |
| `heap.rs`: `HEAP` (`UnsafeCell<FreeList>`) | The first-fit allocator | Every `alloc` and `dealloc` runs inside `without_interrupts`; `free_bytes` is a one-word read | None. Note: the walk runs with interrupts off and grows with fragmentation; thread stacks come from frames, not the heap, partly for that reason |
| `memory.rs`: `static mut BITMAP`, `FREE` | The frame allocator | `alloc_contiguous` and `free_frame` run inside `without_interrupts`. `reserve` (and `set_used` through it) does not, but its only callers are boot-time (`video::init`, and `biostest::show_mode` in the boot thread) | K1 wraps `reserve` in `without_interrupts` (one line), so a later caller in a thread is safe. Note: `alloc_contiguous` scans from frame 0 with interrupts off, which is up to 65,536 iterations when memory is nearly full (an estimate of a few milliseconds at 25 MHz); not a safety issue, a latency one for later |
| `vga.rs`: `static mut STATE` (row, column) | The text console's cursor | **Unsafe under preemption.** `put` increments `row` and only scrolls when it equals 25; a thread preempted between the two lets another thread's `put` take `row` to 26, after which the scroll test never fires again and later writes run past the text buffer, towards the BIOS ROM and eventually RAM | K1: the console is guarded (below) |
| `console.rs`: `_print` (serial and VGA) | `println!` | Two threads' lines interleave byte by byte, which breaks the pinned transcripts' line matching even where nothing is corrupted | K1: `_print` runs with preemption disabled (not with interrupts off: a line at 115,200 baud takes about 87 µs per byte, and a long line with interrupts off would cost ticks). Each `println!` is one `_print` call, so each line stays whole |
| `serial.rs`, the 16550 driver | Port I/O with a bounded wait | Stateless | Covered by the console guard |
| `input.rs`: `KEY_BYTES`, `MOUSE_EVENTS` (`SpscRing`), `STATS` | The interrupt-to-core rings | Producers run in interrupt context. The consumer ends live in one `Ps2Input`, whose `pop` takes `&mut self`, so the type allows one consumer; a consumer preempted mid-`pop` resumes with the indices as it left them, because only it writes `tail`. `split` is a `swap` | **None.** This answers [`HAL.md`](HAL.md#open-questions)'s preemption question for stage 1: the single-consumer side stays correct as long as one thread owns the consumer, which the types already force; sharing it between threads needs a lock or a channel. K1 records that in `HAL.md` |
| `input.rs`: `idle` (`sti; hlt; cli`) | The input loops' wait | Correct under preemption (the timer interrupt that ends the `hlt` may switch threads, and the thread resumes after it), but a thread halting there wastes the rest of its slice | None in K1. The input thread of G2 blocks in `desk_wait` instead |
| `bios.rs`: `READY`, `BUSY` and the re-entry `Guard` | One BIOS call at a time | **Two problems under preemption.** A thread preempted between `Guard::acquire` and the `cli` in `call` makes every other thread's call fail with `Busy`, which callers treat as fatal (`expect`, `unwrap`). And `NativeBios::int` writes the bounce buffer **before** `call` takes the guard and reads it **after** the guard is dropped, so two threads' VBE calls can see each other's buffer | K1: `bios::call` and the whole of `NativeBios::int` (copy in, call, copy out) run with preemption disabled. `Busy` then means what it was written for: a call from inside an interrupt handler or a nested call |
| `bios.rs`: `bios_int`'s tick make-up | Ticks the BIOS handler counted | Adds them with `CLOCK.counter().tick()` directly, so the scheduler's tick hook does not see them | None, if the scheduler compares deadlines with `CLOCK.now()` and never counts its own hook calls (a rule for K2's sleep list). A sleeper whose deadline passed during a BIOS call wakes at the next real tick, up to 10 ms late |
| `bios.rs`: a BIOS call from a thread's stack | The blob saves `ESP` and runs on its own real-mode stack at `0x13000` | Should work from any thread, since the thread stacks are identity-mapped frames above 1 MiB and the blob restores `ESP` | None expected; K1's `tasks` mode makes one call from a spawned thread to prove it |
| `bios.rs`: `STATS`, `biostest.rs`: `PROBE` | Atomic counters | Atomics | None |
| `main.rs`: the panic handler's `PANICKING` | A `swap` | A panic message can be interleaved with another thread's lines, or a thread can be switched in mid-message | K1: the panic handler turns interrupts off first, so nothing else runs while it prints and exits (the pinned panic transcript stays the same) |
| `main.rs`: the 64 KiB boot stack and `kmain` | The only flow of control today | Becomes the **boot thread** of the kernel task, keeping its stack | K1 |
| `fpu.rs`: `init` and `demo` | Turns the x87 on; the boot demo's `sqrt(2)` and `H(10)`, the only x87 code in the kernel | Runs in the boot thread before any other thread exists | None in K1 (spawned threads must not touch the x87 until K2). K2 sets `CR0.TS` on switches. The demo stays as the boot-time FPU check, the one exception to "kernel code is float-free" |
| `gdt.rs`: `GDT` | Read-only | — | None (stage 2 adds a TSS) |
| `shell.rs`, `gfx.rs`, `video.rs`, `biostest.rs` | The test modes' bodies | Run in the boot thread only; nothing else draws or reads input in those modes | None. The pinned transcripts of those modes run with the scheduler live (K1 starts it in every mode), which is itself a regression check |

The audit's changes are small (an estimated 60 lines): the preemption counter, the console guard, the BIOS guard widened to `NativeBios::int`, `reserve` under `without_interrupts`, and `cli` in the panic handler. K1 lands them as its first commit, before the switch exists, and the pinned transcripts must not change.

## Shape of the code

- **`tasks/`**, a new Cargo workspace beside `shell/` and `hal/`, with `[workspace.package]` as `shell/`'s, `tasks/clippy.toml` a copy of `shell/clippy.toml` (no compare-and-swap) and `tasks/rustfmt.toml` as `shell/`'s. Two crates, both `no_std`, built for the host, `kernel/i486-desk-none.json` and `spikes/d0-rust-dos/dos-elf.json`, with no external crates and no clocks or randomness:
  - **`desk-sched`** (`tasks/sched/`): the scheduler's state as plain data, `#![forbid(unsafe_code)]`, no `alloc`: a fixed table of thread slots indexed by a `ThreadId`, 32 ready queues linked through the slots and a `u32` bitmap (`31 - leading_zeros`, a `BSR`), the sleep list sorted by deadline, the wait table keyed by address, and later the lock records and ageing. No allocation, so the tick and wake paths can run in interrupt context; every decision the kernel makes about who runs next is a function in this crate, tested on the host, deterministically.
  - **`desk-sync`** (`tasks/sync/`): the SDK's locks and channels over a small platform trait, `#![deny(unsafe_code)]` with one module (the cell behind `Mutex<T>` and `RwLock<T>`) allowed `unsafe` with a `SAFETY:` line on each block, because a lock that hands out `&mut T` cannot be written without it. Its host tests run against a deterministic model of a uniprocessor scheduler.
- **The kernel** keeps what needs `unsafe` or the machine: `kernel/src/sched.rs` (the one scheduler instance in a static, the switch, preemption, the idle thread, kill), `kernel/src/switch.rs` (the context switch in `global_asm!`), `kernel/src/task.rs` (task records), and from K2 `kernel/src/wait.rs` and `kernel/src/fpu.rs` additions. It depends on `desk-sched` and, from K2's `wait` mode, on `desk-sync`.

Why a separate crate for the scheduler's state: K3's inheritance and Y1's lock protocol are where the subtle bugs will be, and a pure data structure can be tested exhaustively on the host, while the kernel can only be tested by transcripts.

## Order and parallelism

At most two tasks run at once. A wave starts when everything it depends on is merged; each task is a pull request off `main`.

| Wave | Tasks, in parallel | Depends on | Size (new lines, roughly, tests included; estimates) |
|---|---|---|---|
| 1 | K1 tasks and preemption ∥ Y1 `desk-sync` | — | 1,300 ∥ 900 |
| 2 | K2 wait and wake, threads, timers, keys, x87 | K1 (and Y1 for its `desk-sync` scenario, which waits for Y1 if Y1 is late) | 1,000 |
| 3 | K3 ageing, ceilings, inheritance | K2, Y1 | 800 |

In the whole-path map's lanes K1 sits beside M3 in wave 1 and Y1 may start in any free slot, since it touches no kernel file; the map placed it later, and starting it with K1 gives K2 its `desk-sync` scenario at once. If Y1 runs later than wave 2, K2 ships without that scenario and Y1 adds it.

Branches: `feat/tasks-preemption` (K1), `feat/desk-sync` (Y1), `feat/tasks-wait` (K2), `feat/tasks-priorities` (K3).

Shared files within a wave: K1 and Y1 both create `tasks/Cargo.toml` (whichever merges second adds its member when it rebases; the first creates the workspace, `tasks/clippy.toml`, `tasks/rustfmt.toml` and `tasks/check.sh`) and both add a line to `tasks/check.sh`. Nothing else is shared.

**Test modes.** The whole-path map reserves 21 `tasks`, 22 `tasks-runaway` and 23 `wait`. This plan also needs two negative runs that cannot be expressed in those modes, and proposes **29 `tasks-nopreempt`** (K1) and **30 `wait-nosave`** (K2), the next free numbers after the map's 28; they go into the registry in the roadmap pull request before K1 starts, as the map asks. If the owner prefers no new numbers, the two negatives become host tests only and the plan says so in the result.

## Coordination with R134

R134 (the hosted PIT timer from Fizz 2, [`FIZZ2-COMPARISON.md`](FIZZ2-COMPARISON.md)) changes `hal/hal/src/irq.rs`: `Chain`, fixed at registration, becomes a per-interrupt `Action` returned by the handler. K1 changes how the kernel dispatches IRQ 0. Its build plan does not exist yet, and the whole-path map puts the reunification's builds from wave 9.

- **K1 lands first.** It is ready for wave 1 and does not wait for a plan that is not written.
- **K1 does not touch `hal/hal/`.** The scheduler's tick is a kernel-side `SharedIrqService` in `kernel/src/pit.rs` that calls `CLOCK.counter().tick()` and then `sched::tick()`, registered with `Chain::None` as today; the switch is a call at the end of `irq_dispatch` in `kernel/src/hal.rs`. So R134's change to `irq.rs` applies to the HAL without conflict.
- **What R134 must rebase over** (to be written into its plan): the kernel's tick service in `kernel/src/pit.rs` returns whatever `Action` means "the platform sends the end-of-interrupt" (natively always); `irq_dispatch` and `IrqTable::run_handler` in `kernel/src/hal.rs` act on the returned `Action` but keep K1's order unchanged: **handler, then end-of-interrupt, then the scheduler's interrupt-exit hook**, and the hook runs for every line, whatever the handler returned, because a keyboard interrupt that wakes the input thread (G2) must be able to preempt too. `IrqTable::run_handler` (called by `bios_int` with interrupts off) never runs the hook. K2 adds nothing at that junction.
- If R134's HAL change happens to be ready before K1 starts, K1 rebases onto it instead; the junction is the same three call sites either way.

## Rules for every task

- **Read first**: [`AGENTS.md`](../AGENTS.md), [`APP-MODEL.md`](APP-MODEL.md) (the decisions, the staged path, threads, wait and wake, priorities, thread-local storage, the FPU and the kernel's mutual exclusion), this plan's audit, and the documents the task names. Follow `AGENTS.md`: Canadian English, no hard-wrapped markdown, real em dashes, licence headers on new source files, Conventional Commits ending with the agent co-author trailer, a PR title without a prefix, labels on the PR, merge commits only, and no merging without the owner's go-ahead.
- **Work in a git worktree** under `.claude/worktrees/` on the task's branch off `origin/main`, never in the main checkout.
- **Missing tools run in Docker**, never installed on the host: the CI image `ghcr.io/liminal-hq/ci-dos` (its digest is in `.github/workflows/ci.yml`) has Rust 1.98.0 with `rust-src`, QEMU and NASM. Run containers with `--user "$(id -u):$(id -g)"` and a writable `HOME` and `CARGO_HOME`. Do not prune Docker images or volumes.
- **The pinned files do not change**: `tests/native/expected.txt` and every existing `tests/native/expected-*.txt`, the shell goldens and their transcripts. The scheduler is live in every mode from K1, and it prints only in the task test modes. A task that needs to change a pinned file has found a regression and stops to report it. A task may change the expected transcripts an earlier task of this plan created (K3 extends K2's `expected-wait.txt`) and says so.
- **`tools/check.sh` and `.github/workflows/*`**: a CI rework is in progress (a stack that groups `tools/check.sh` and adds a scope script). Tasks here **do not edit `.github/workflows/*`**. The plan's one edit to `tools/check.sh` is a single line calling `tasks/check.sh`, made by whichever of K1 and Y1 merges first, placed where the CI rework's grouping says if it has landed; the emulator checks go into `tools/native-test.sh` (a "Tasks" section), which `tools/check.sh` already runs. [New checks for the CI stack](#new-checks-for-the-ci-stack) lists what the rework should wire in.
- **The crate rules**: `desk-sched` and `desk-sync` as in [Shape of the code](#shape-of-the-code); no compare-and-swap anywhere in this plan's code, the kernel's included (interrupts off makes it unnecessary on one CPU); `unsafe` in the kernel carries a one-line `SAFETY:` comment saying what makes it sound.
- **Interrupt context**: no allocation, no x87, bounded work. The scheduler's tick, `desk_wake` and the interrupt-exit hook are called from interrupt context and allocate nothing (the `desk-sched` tables are fixed-size).
- **Kernel code is float-free.** The only x87 code in the kernel is `fpu::demo` in the boot thread. Test threads that use the x87 (K2) do it through inline assembly on purpose, in the test code only.
- **Fix a bug with a check that fails before the fix.** Every new emulator check has a negative run that must fail; every host test of a protocol has a deliberately broken variant that the test must catch.
- **Verify before the PR**: the task's acceptance commands, `python3 -I tools/check-docs.py`, `tools/check-headers.sh`, and `tools/check.sh` (in the CI image where the host lacks a tool). The PR's test plan says exactly what ran and what did not, and that nothing ran on real hardware.
- **Update the documents the task changes** in the same branch: [`APP-MODEL.md`](APP-MODEL.md) where the code chose differently, [`HAL.md`](HAL.md) (concurrency), [`NATIVE-BOOT.md`](NATIVE-BOOT.md) (test modes), `kernel/README.md`. `AGENTS.md` and `CLAUDE.md` are the owner's: the repository-layout line for `tasks/` is proposed in the PR description, not edited.

## K1. Tasks, threads and preemption in ring 0

**Goal.** Stage 1's core: the boot flow becomes the boot thread of a kernel task; the kernel can create tasks and threads, switch between them in software, preempt them from the PIT at 100 Hz (one 10 ms slice per tick), hold preemption off with a counter, pick the next thread from 32 ready queues with a bitmap, run an idle thread when nothing is ready, and kill a task. A thread spinning in `loop {}` with no calls loses the CPU at the end of every slice and its task can be killed.

**Files.**

- First commit, the audit's fixes: `kernel/src/sync.rs` (`preempt_disable`, `preempt_enable`, `PreemptGuard`; the counter is a no-op until the scheduler starts), `kernel/src/console.rs` (`_print` under `PreemptGuard`), `kernel/src/bios.rs` (`call` and `NativeBios::int` under `PreemptGuard`), `kernel/src/memory.rs` (`reserve` under `without_interrupts`), `kernel/src/main.rs` (`cli` first in the panic handler). Every pinned transcript passes unchanged at this commit.
- `tasks/Cargo.toml`, `tasks/Cargo.lock`, `tasks/clippy.toml`, `tasks/rustfmt.toml`, `tasks/check.sh` (`fmt --check`, `clippy --all-targets -D warnings`, `test`, and the cross `clippy` for `kernel/i486-desk-none.json` and `check` for `spikes/d0-rust-dos/dos-elf.json`, as `tools/check.sh` does for `shell/`), if Y1 has not created them.
- `tasks/sched/Cargo.toml`, `tasks/sched/src/lib.rs`, `table.rs` (thread slots, states, base priority, slice counters), `ready.rs` (the queues and the bitmap), `tasks/sched/tests/ready.rs`.
- `kernel/Cargo.toml`, `kernel/Cargo.lock`: a path dependency on `desk-sched`.
- `kernel/src/switch.rs`: `switch_stacks` in `global_asm!` and the first frame of a new thread.
- `kernel/src/sched.rs`: the scheduler static, `init`, `spawn`, `yield_now`, `tick`, `irq_exit`, `schedule`, the idle thread, thread end by return, the reaper. `kernel/src/task.rs`: task records, `kill`.
- `kernel/src/pit.rs`: the tick service (counter, then `sched::tick`). `kernel/src/hal.rs`: `irq_dispatch` calls `sched::irq_exit()` after the end-of-interrupt.
- `kernel/src/taskstest.rs`: the `tasks`, `tasks-runaway` and `tasks-nopreempt` scenarios. `kernel/src/main.rs`: `TestMode::Tasks = 21`, `TasksRunaway = 22`, `TasksNoPreempt = 29`; `sched::init()` in every mode, right after `heap::init()` and before interrupts come on.
- `tools/native-image.sh`: the three mode names. `tools/native-test.sh`: a "Tasks" section. `tests/native/expected-tasks.txt`, `tests/native/expected-tasks-runaway.txt`, `tests/native/expected-tasks-nopreempt.txt`.
- `tools/check.sh`: one line calling `tasks/check.sh`, if Y1 has not added it.
- `docs/HAL.md` (Concurrency: natively, kernel threads exist from stage 1; "interrupts off" still excludes preemption on one CPU; the open question on preemption answered for the rings), `docs/NATIVE-BOOT.md` (the modes and the boot thread), `docs/APP-MODEL.md` (the Built column), `kernel/README.md`.

**API.**

```rust
// desk-sched
pub const LEVELS: usize = 32;
pub struct ThreadId(u16);
pub enum State { Ready, Running, Blocked, Dead }
pub struct Sched<const N: usize> { /* slots, 32 queues, bitmap, current, idle */ }
impl<const N: usize> Sched<N> {
    pub const fn new() -> Self;
    pub fn add(&mut self, task: u16, priority: u8) -> Option<ThreadId>;   // a Ready thread at the tail of its level
    pub fn remove(&mut self, t: ThreadId);                                // from whichever queue; for exit and kill
    pub fn pick(&mut self) -> ThreadId;                                   // highest ready level, its head; idle when none
    pub fn tick(&mut self) -> bool;                                       // the slice ended: true if a switch is wanted
    pub fn made_ready(&mut self, t: ThreadId) -> bool;                    // true if it outranks the current thread
    pub fn slices(&self, t: ThreadId) -> u32;                             // for the tests and a later task list
}

// kernel/src/sched.rs
pub const SLICE_TICKS: u32 = 1;          // 10 ms at 100 Hz
pub const MAX_THREADS: usize = 256;      // the design's system target
pub const STACK_BYTES: usize = 16 * 1024;
pub fn init();                                                           // the boot thread at 16 and the idle thread
pub fn spawn(task: TaskId, entry: extern "C" fn(u32), arg: u32, priority: u8) -> Result<ThreadId, SpawnError>;
pub fn yield_now();
pub fn current() -> ThreadId;
pub(crate) fn tick();                    // from the IRQ 0 service
pub(crate) fn irq_exit();                // from irq_dispatch, after the end-of-interrupt
// kernel/src/task.rs
pub fn create(name: &'static str, ceiling: u8) -> Result<TaskId, SpawnError>;
pub fn kill(t: TaskId) -> Killed;        // threads removed and their stacks freed; never the caller's own task
```

**How the switch works.** All threads run in ring 0 on their own stacks, so an interrupt pushes its frame onto the interrupted thread's stack. `irq_dispatch` runs the handler and sends the end-of-interrupt (so the PIC is not left blocked while another thread runs), then `irq_exit` switches if a switch is wanted and the preemption counter is zero; the interrupted thread resumes later through the same stub's `popad; iretd`. A voluntary switch (`yield_now`, and K2's blocking calls) calls `schedule` inside `without_interrupts`. `switch_stacks(old: *mut u32, new: u32)` pushes `EBP`, `EBX`, `ESI`, `EDI` and `EFLAGS`, stores `ESP` in the old thread's slot, loads the new one and pops the same; it is always entered with interrupts off, and each thread gets its own interrupt flag back on its own return path. A new thread's stack starts with a frame that returns into `thread_start`, which turns interrupts on, calls `entry(arg)` and, when it returns, marks the thread dead and switches away; its stack is freed by the next thread to run through the scheduler (a thread cannot free the stack it stands on). Stacks are four contiguous frames from `memory::alloc_contiguous`, page-aligned so stage 2 can put a guard page below each. The preemption counter: `preempt_enable` returning it to zero with a switch pending switches at once. The idle thread is in no queue: `pick` returns it when the bitmap is zero, and it loops on `sti; hlt`.

**Kill.** `kill` runs with interrupts off: each of the task's threads is removed from its queue and marked dead, its stack and slot freed, and the task record freed. In stage 1 every thread not running is either in a ready queue at an instruction of its own (a preempted thread, never inside a preemption-disabled section, since it could not have been preempted there) or, from K2, blocked in the scheduler with nothing held; so a killed thread never holds kernel state. The task's memory ledger, handles and windows are A2's and G2's; K1's tasks own only their threads.

**The scenarios and their lines** (after the normal boot's, which stay as they are; ranges are estimates the first run will pin, reviewed in the PR):

`tasks` (21):

```text
tasks: scheduler on, 32 levels, slice 10 ms, boot thread at 16, idle thread ready
tasks: three spinners at 16 spawned in task spin
tasks: after 100 ticks: boot {N:20..30} slices, spinners {N:20..30} {N:20..30} {N:20..30}
tasks: preemption disabled for 20 ticks: the spinners did not run: yes
tasks: preemption enabled: a spinner ran within {N:0..2} ticks
tasks: a thread at 20 spawned: it ran before spawn returned: yes
tasks: a thread at 8 spawned: it has not run while the level-16 threads are ready: yes
tasks: INT 1Ah from a spawned thread: ok
tasks: killed task spin (3 threads): its counters stopped: yes
tasks: frames as before: yes, heap as before: yes
```

The spinners are `loop { COUNT[i].store(COUNT[i].load() + 1) }`, with no calls. The level-8 thread is killed with its task at the end.

`tasks-runaway` (22): a task whose one thread is `loop {}` with a counter store and no calls, at the boot thread's level; the boot thread keeps running beside it for 50 ticks and then kills it.

```text
tasks: a runaway loop at 16 spawned in task runaway
tasks: the boot thread kept running for 50 ticks: yes, the runaway ran {N:20..30} slices
tasks: killed task runaway: its counter stopped: yes
```

`tasks-nopreempt` (29), the negative: the same as `tasks-runaway`, but the runaway disables preemption before its loop (which a ring-0 kernel thread can do and an app never will). The boot thread must never run again: the transcript ends at `tasks: a runaway loop at 16 spawned in task runaway`, and the run ends by the watchdog (status 124), proving that preemption is what rescued the boot thread in mode 22.

**Acceptance.**

- `cargo test` in `tasks/`: `pick` takes the highest non-empty level and its head; round-robin rotates within a level; the bitmap is zero exactly when every queue is empty, after every one of 10,000 random operations from a fixed-seed generator (add, remove, pick, tick, made-ready) checked against a naive model (a `Vec` per level); `made_ready` is true only for a higher level; `remove` from the middle of a queue; a full table refuses `add`; idle is picked only with no ready thread.
- `tools/native-test.sh`: `tasks` and `tasks-runaway` match their expected transcripts with exit status 0, `tasks-nopreempt` matches its expected transcript with status 124, on `-cpu 486`.
- Negative runs that must fail: `expected-tasks-runaway.txt` against the `tasks-nopreempt` transcript; `expected-tasks.txt` with the `did not run: yes` line changed to `no`.
- Every existing native check is unchanged, now with the scheduler live in every mode (`git diff origin/main -- tests/native/expected*.txt` shows only the three new files); the first commit passes them before the scheduler exists.
- `kernel.bin`'s size before and after is in the PR, under the image tool's 1,000-sector limit; the number of `cmpxchg` instructions in `kernel.elf` before and after (`objdump -d`, in the CI image) is in the PR and does not grow.
- Optional, reported if tried: `tasks` and `tasks-runaway` under Bochs (`NATIVE_EXTRA=1`).

**Depends on**: nothing. **Parallel with**: Y1. **Labels**: `enhancement`, `rust`, `performance`, `spike`.

**Risks.** The switch is the sharpest code in the plan: write `switch_stacks` and the first frame against the cdecl rules, test it first with two threads that only `yield_now`, then with preemption. A switch from inside `irq_dispatch` must come after the end-of-interrupt, or IRQ 0 stays blocked until the interrupted thread runs again; the `tasks` mode's slice counts catch that (they would be zero). A thread stack of 16 KiB must hold an interrupt frame plus the deepest handler path (the mouse decoder, `println!` formatting): K1 fills new stacks with a pattern and the `tasks` mode prints the deepest use it saw, as a measurement for the PR. The fairness ranges are guesses until the first run. If the shell modes' timing changes with the scheduler live (they wait on ticks with `hlt`), the shell goldens are byte-identical by construction but a timeout could move; the pinned runs are the check. Test mode numbers 29 and 30 must be in the registry before K1 starts.

## K2. Wait and wake, sleep, spawn, exit and join, thread-local keys, lazy x87

**Goal.** The rest of R141 in the kernel: the one blocking primitive, sleep and timeouts, the thread API in the SDK's shape, thread-local keys, and per-thread x87 state saved lazily through `#NM`, so threads block instead of spinning and any thread may use the x87 without disturbing another's.

**Files.**

- `tasks/sched/src/wait.rs` (the wait table: 64 buckets keyed by address, an estimate; each bucket a list in priority order, first in first out within a level), `sleep.rs` (the sleep list sorted by absolute deadline, compared by wrapping subtraction against `now`), `tasks/sched/tests/wait.rs`, `tasks/sched/tests/sleep.rs`.
- `kernel/src/wait.rs`: `desk_wait`, `desk_wake`, `desk_sleep`, `wake_from_irq`. `kernel/src/sched.rs`: blocking, timeouts on the tick, the thread API, task end when its first thread returns. `kernel/src/tls.rs`: keys.
- `kernel/src/idt.rs`: vector 7 gets its own stub and handler. `kernel/src/fpu.rs`: `CR0.TS` on switches, the `#NM` handler, the per-thread save area.
- `kernel/src/waittest.rs`: the `wait` and `wait-nosave` scenarios; with Y1 merged, the `desk-sync` scenario (`kernel/Cargo.toml`: a path dependency on `desk-sync`, and `kernel/src/sys.rs`, the kernel's implementation of `desk-sync`'s platform trait over these calls). `kernel/src/main.rs`: `TestMode::Wait = 23`, `WaitNoSave = 30`.
- `tools/native-image.sh`, `tools/native-test.sh`, `tests/native/expected-wait.txt`, `tests/native/expected-wait-nosave.txt`.
- `docs/APP-MODEL.md` (wait and wake, threads and the FPU as built, the x87 costs below), `docs/NATIVE-BOOT.md`.

**API.**

```rust
#[repr(u32)] pub enum WaitResult { Woken = 0, Changed = 1, TimedOut = 2 }
pub const FOREVER: u32 = u32::MAX;
/// Blocks while `*addr == expected`, until a wake on `addr` or `timeout_ms` (rounded up to ticks; 0 checks and returns).
pub fn desk_wait(addr: *const u32, expected: u32, timeout_ms: u32) -> WaitResult;
/// Wakes up to `count` waiters on `addr` (u32::MAX for all), highest priority first; returns how many. Callable from interrupt context.
pub fn desk_wake(addr: *const u32, count: u32) -> u32;
pub fn desk_sleep(ms: u32);
pub fn desk_thread_spawn(entry: extern "C" fn(u32) -> u32, arg: u32, stack_size: u32, priority: u8) -> Result<ThreadHandle, SpawnError>;
pub fn desk_thread_exit(code: u32) -> !;
pub fn desk_thread_join(h: ThreadHandle) -> u32;     // once per thread; a second join or a join of oneself is refused
pub fn desk_yield();
pub fn desk_tls_alloc() -> Option<TlsKey>;           // per task, 64 keys (an estimate)
pub fn desk_tls_get(k: TlsKey) -> u32;               // 0 until set, in every thread
pub fn desk_tls_set(k: TlsKey, v: u32);
pub fn desk_tls_free(k: TlsKey);
```

In stage 1 these are kernel functions; A1 and A2 bind them to import slots with the same names. `desk_wait` reads `*addr` and enqueues with interrupts off, so no wake can fall between the check and the sleep (one CPU). A spurious wake is allowed and callers re-check, as with every futex. When a task's first thread returns (from `desk_main`, in A2) or exits, the task ends and its other threads are killed with it.

**The x87, lazily, per thread** (the owner's float policy above):

- **The mechanism.** The kernel records which thread's state is in the x87 (the owner, initially the boot thread). On every switch to a thread that is not the owner it sets `CR0.TS`; to the owner it clears it. The first x87 instruction a non-owner executes raises `#NM`; the handler (an interrupt gate, so interrupts are off) runs `CLTS`, saves the owner's state with `FNSAVE` into the owner's 108-byte area, then restores this thread's with `FRSTOR`, or, on its first use ever, runs `FNINIT` and loads the new-thread control word; this thread becomes the owner, and the instruction is retried. The save area is part of each thread's slot, so `#NM` never allocates (256 threads cost 27 KiB, an estimate).
- **What a thread's first use costs.** One `#NM` trap, `CLTS`, one `FNSAVE` of the previous owner (if there is one) and an `FNINIT` plus `FLDCW`: an estimated few hundred cycles on a 486, once. Later, a thread pays one trap plus an `FNSAVE` and `FRSTOR` (about 108 bytes each way; an estimate of 200 to 400 cycles in all) only when it uses the x87 after another thread has used it since; a thread that is the only x87 user keeps the unit with no trap at all.
- **What a thread or task that never uses the x87 pays.** Nothing beyond setting `CR0.TS` on the switch to it (one `mov cr0`), which the switch does anyway when the owner differs; no save, no restore, no trap. Interrupt handlers and the kernel never touch the x87, so they never trap.
- **When a thread exits or is killed.** Its x87 state is discarded, not saved: if it was the owner, the owner becomes none and the next user's `#NM` saves nothing. Its save area goes with its slot. A pending x87 exception of a dead thread cannot reach anyone else, because the next user's `FRSTOR` or `FNINIT` replaces the whole state.
- **The control word of a new thread.** One constant, `0x027F` (53-bit precision, as Fizz 2 uses) by default, or `FNINIT`'s `0x037F` if the owner prefers ([Questions](#questions-for-the-owner)); a thread can always set its own.
- **The test.** In `wait`, two threads at the same level, preempted every slice, each run a loop of x87 work in inline assembly for 100 ticks: thread A sets the control word to `0x027F`, loads 1.0 and repeatedly adds 1.0 to the top of the stack; thread B sets `0x037F`, loads 3.0 and repeatedly multiplies by 1.0. After every step each checks its own control word, its stack depth (`FNSTSW`'s top) and the value it expects (integer counts, compared with `FIST` into an integer, so the test code itself does no float maths in Rust). Each prints its step count and `x87 kept: yes`; the `#NM` count is printed and must be at least the number of switches between them. `wait-nosave` (30) builds the same scenario with the `#NM` handler skipping the save and restore: B must see A's control word or value, print `x87 kept: no` and panic, so the run ends with status 1.

**The scenario and its lines** (`wait`, 23; ranges are estimates):

```text
wait: a waiter on a word, the value unchanged, woken by a wake: Woken
wait: a wait with a stale expected value: Changed at once
wait: a wait with a 50 ms timeout: TimedOut after {N:5..6} ticks
wait: a wake of 2 out of 3 waiters woke 2, the highest first: yes
wait: desk_sleep(100) slept {N:10..11} ticks
wait: 8 sleepers with different deadlines woke in deadline order: yes
wait: a wake from the keyboard interrupt woke its waiter: yes
wait: spawn, exit 42, join: 42
wait: the task ended when its first thread returned: its other 2 threads are gone: yes
wait: TLS key 0: thread A sees 7, thread B sees 9, a new thread sees 0: yes
wait: x87 thread A {N:1..100000} steps, x87 kept: yes
wait: x87 thread B {N:1..100000} steps, x87 kept: yes
wait: device-not-available traps {N:2..10000}
wait: desk-sync: 4 threads x 10000 increments under one Mutex: 40000
wait: desk-sync: a Channel of 32 carried 1000 values in order: yes
wait: desk-sync: Condvar, Semaphore, RwLock and Once: ok
wait: frames as before: yes, heap as before: yes
```

The keyboard line uses the injected-key mechanism of the input modes (`tools/native-run.sh --inject`) with one key; the kernel wraps the keyboard driver's handler in a kernel-side service that calls `wake_from_irq` after it, which is also what G2's input thread will use. If that proves awkward in this mode, the line uses the PIT's tick instead and the PR says so.

**Acceptance.**

- `cargo test` in `tasks/`: the wait table and the sleep list against naive models under 10,000 fixed-seed random operations; wake order by priority then arrival; deadlines across the counter's wrap; a sleeper removed by a kill is never woken.
- `tools/native-test.sh`: `wait` matches `expected-wait.txt` (exit 0), `wait-nosave` matches `expected-wait-nosave.txt` (status 1).
- Negative runs that must fail: `expected-wait.txt` against the `wait-nosave` transcript; `expected-wait.txt` with `woke in deadline order: yes` changed to `no`. A host test with a deliberately lost wake (the check and the enqueue split by a simulated interrupt) must find the lost wake.
- Every existing native check is unchanged, `fpu: results check out` included (the boot thread is the first x87 owner); K1's transcripts are unchanged.
- The `#NM` handler's disassembly is in the PR; nothing in interrupt context executes an x87 instruction (checked by reading the handlers and by a debug assertion that `#NM` never fires with an interrupt in service, using the PIC's in-service register).

**Depends on**: K1; Y1 for the `desk-sync` lines (without Y1 merged, they are left out and Y1 adds them). **Parallel with**: nothing (wave 2). **Labels**: `enhancement`, `rust`, `spike`.

**Risks.** Lost wakes are the classic futex bug: the check of `*addr` and the enqueue must sit in one interrupts-off section, and the host model's lost-wake test is the review checklist. `FNSAVE` re-initialises the x87 after saving (unlike `FXSAVE`), which is right here but surprising; the `#NM` handler must never be entered from kernel code that thinks it owns the x87 (only `fpu::demo` does, in the boot thread, which starts as the owner). A thread killed while it is the x87 owner must not leave `owner` pointing at a freed slot. Timeouts converted from milliseconds to ticks round up, so a 50 ms wait is 5 or 6 ticks; the transcript allows both.

## K3. Ageing, ceilings and basic priority inheritance

**Goal.** R142's policy on top of K1's queues: ageing so a busy thread cannot starve lower ones for ever, task ceilings and priority changes, and basic (one-level) priority inheritance for the SDK's mutex, with the lock protocol below, which needs no compare-and-swap.

**Files.**

- `tasks/sched/src/age.rs` (the once-a-second scan), `prio.rs` (base and effective priority, ceilings), `lock.rs` (lock records keyed by address: the kernel's view of an owner and its waiters, the held and waited-on lists, the recomputation of effective priority), `tasks/sched/tests/age.rs`, `tasks/sched/tests/lock.rs`.
- `kernel/src/sched.rs`, `kernel/src/wait.rs`: `desk_lock_wait`, `desk_lock_wake`, `desk_thread_set_priority`, `desk_task_set_ceiling`; ageing on the tick.
- `tasks/sync/src/mutex.rs`: the inheriting protocol behind a `Platform` capability (platforms without lock calls, the hosted one at first, keep Y1's protocol). `tasks/sync/tests/model/`: the model's kernel side uses `desk-sched`'s `lock.rs`, so the host tests run the kernel's real record logic.
- `kernel/src/waittest.rs` and `taskstest.rs`: the inversion and ageing sections. `tests/native/expected-wait.txt` and `expected-tasks.txt` extended (files of this plan, changed deliberately).
- `docs/APP-MODEL.md`: the lock protocol as built and the two lock calls.

**The policy, as built.** Levels and bands as [`APP-MODEL.md`](APP-MODEL.md#priorities) (input 27, compositor 26, apps 8 to 23 at 16, workers 12, background 0 to 7; a task ceiling of 23 by default, the kernel task's 31). A ready thread that has not run for 100 ticks is raised one level for each further 100 ticks, up to its task's ceiling, and drops back to its base when it runs; the scan of the queues below the current level runs once a second, on the tick, in interrupt context, and is bounded by the number of ready threads. `desk_thread_set_priority` clamps to the ceiling; lowering a ceiling clamps the task's threads.

**The lock protocol** (an exchange-only mutex with an owner, which is the design's sketch made precise; the model check is what proves it):

- The word: 0 is free; otherwise the low 31 bits name a thread and bit 31 (`W`) says the kernel has waiters for it. The word is a hint; the kernel's lock record, created on first contention, is the truth.
- **Lock, fast:** `v = swap(me)`. If `v == 0`, the caller owns the lock. Otherwise it has overwritten the word, and calls `desk_lock_wait(addr, v)` with what it saw.
- **Unlock, fast:** `v = swap(0)`. If `v == me`, nobody contended and the unlock is done. Otherwise call `desk_lock_wake(addr)`.
- **In the kernel, with interrupts off:** `lock_wait` reads the word. If it is 0, the lock is free: write `me` (with `W` if the record has waiters) and return owning it. Otherwise the record's owner is updated from the best knowledge so far: the thread named by `v` unless the record knows that thread released the lock or is itself a waiter, else the thread named by the word; the caller is queued by priority, the word is rewritten as `owner | W`, and if the caller outranks the owner the owner's effective priority is raised to the caller's and it moves to that ready queue. `lock_wake` hands the lock to the highest waiter (word `waiter | W` if others remain, else `waiter`), wakes it, notes that the old owner released, and recomputes the old owner's effective priority from the locks it still holds.
- **Why it holds without compare-and-swap.** Only an owner's own `swap` can leave its exact id without `W` in the word, so an unlock that sees anything else knows to call the kernel; a contender that clobbered the word carries the old value into the kernel; and a wrong guess at the owner (a contender preempted between its `swap` and its call) only boosts a thread that is a few instructions from entering the kernel itself, where it corrects the record. A barging thread that finds the word 0 after a release owns the lock, and the kernel adopts it as the owner and sets `W` so its unlock comes to the kernel.
- **Basic means one level.** An owner that itself waits on another lock does not pass the boost on. Only the mutex inherits; condition variables re-acquire their mutex, which inherits.

If the model finds a hole the builder cannot close, the fallback is that every contended lock and every unlock with `W` set enters the kernel and the kernel alone writes the word; the fast paths stay one `swap` each. The PR says which was built.

**Lines added.** To `tasks`: `tasks: a spinner at 20 for 6 s: the boot thread at 16 aged to 20 and ran within {N:4..6} s: yes`, then it kills the spinner. To `wait`: `wait: inversion: L at 8 holds the mutex, M at 16 spins, H at 20 waits: L ran at 20, H got the mutex within {N:1..3} ticks: yes`, `wait: L back at 8 after the unlock: yes`, `wait: a ceiling of 12 clamped a request for 20 to 12: yes`.

**Acceptance.**

- `cargo test` in `tasks/`: ageing raises and resets as specified; ceilings clamp; the lock protocol in the deterministic model (Y1's harness with `desk-sched`'s lock records as its kernel): every interleaving of 2 threads and 2 lock-unlock rounds exhaustively, and 3 to 4 threads with barging under 10,000 fixed-seed schedules, checking mutual exclusion, no lost wake, that a waiting higher thread's priority reaches the owner within one kernel entry of the owner's last pending contender, and that every effective priority returns to its base when no lock is held.
- Negative: the model with the `W` bit never set must find a stranded waiter; with inheritance turned off, the inversion scenario on the host must show H waiting while M runs.
- `tools/native-test.sh`: the extended `tasks` and `wait` transcripts; the `tasks-runaway`, `tasks-nopreempt` and `wait-nosave` transcripts unchanged; every pinned transcript unchanged.

**Depends on**: K2, Y1. **Parallel with**: nothing (wave 3). **Labels**: `enhancement`, `rust`, `spike`.

**Risks.** The lock protocol is new; it is the plan's main design risk and the reason the model uses the kernel's own record code. The scan of ageing in interrupt context must stay bounded (it walks ready threads only). Priority changes of a thread that holds locks must recompute from its records, not overwrite its effective priority.

## Y1. desk-sync: the SDK's locks and channels

**Goal.** The layer-3 primitives of [`APP-MODEL.md`](APP-MODEL.md#wait-and-wake) as one `no_std` crate written once over the one wait-and-wake call: `Mutex<T>`, `Condvar`, `Semaphore`, `RwLock<T>`, `Once` and a bounded `Channel<T, N>`, using only load, store, `swap` and `fetch_add` and `fetch_sub`, so it also runs under DOSBox's strict normal core; tested on the host against a deterministic model of a uniprocessor that can switch threads between any two atomic steps.

**Files.** `tasks/Cargo.toml` (or the member, if K1 created it), `tasks/sync/Cargo.toml`, `tasks/sync/src/lib.rs`, `platform.rs`, `mutex.rs`, `condvar.rs`, `semaphore.rs`, `rwlock.rs`, `once.rs`, `channel.rs`, `cell.rs` (the one module allowed `unsafe`), `tasks/sync/tests/model/mod.rs` (the deterministic model), `tasks/sync/tests/*.rs`. `tasks/check.sh` and the `tools/check.sh` line if K1 has not added them. `docs/APP-MODEL.md`: a pointer to the crate and its rules.

**API.**

```rust
/// What a platform gives the primitives: a 32-bit word with the four allowed operations, and wait and wake on it.
pub trait Platform: 'static {
    type Word: Word;
    fn wait(w: &Self::Word, expected: u32, timeout_ms: u32) -> WaitResult;
    fn wake(w: &Self::Word, count: u32) -> u32;
    fn thread_id() -> u32;            // non-zero; used by K3's mutex and by debug checks
}
pub trait Word: Sync { fn new(v: u32) -> Self; fn load(&self) -> u32; fn store(&self, v: u32); fn swap(&self, v: u32) -> u32; fn fetch_add(&self, v: u32) -> u32; fn fetch_sub(&self, v: u32) -> u32; }

pub struct Mutex<P: Platform, T> { /* word, cell */ }      // lock -> MutexGuard; try_lock
pub struct Condvar<P: Platform> { /* sequence word */ }    // wait(guard), wait_timeout, notify_one, notify_all
pub struct Semaphore<P: Platform> { /* count, waiters */ } // acquire, try_acquire, acquire_timeout, release(n)
pub struct RwLock<P: Platform, T> { /* readers, writer */ }// read, write
pub struct Once<P: Platform> { /* state */ }               // call_once
pub struct Channel<P: Platform, T, const N: usize> { /* ring, two semaphores, a mutex */ }  // send, try_send, recv, recv_timeout
```

The mutex without inheritance is the exchange-only one the design gives: lock `swap(1)`, and if that saw non-zero, `swap(2)` and wait on 2 until a `swap(2)` returns 0; unlock `swap(0)`, and if it saw 2, wake one. The condition variable is a sequence number (`fetch_add` on notify, wait on the value seen before releasing the mutex), with the mutex re-acquired in the contended state. The semaphore keeps its count in one word with `fetch_sub` and undoes an over-take with `fetch_add`, plus a waiter count; the read-write lock is a reader count and a writer mutex (writers preferred, so readers cannot starve a writer); `Once` is three states over `swap`. Every operation that blocks has a timeout form where the SDK will need one.

**The deterministic model.** `tests/model/` runs N simulated threads as host threads of which exactly one runs at a time; the model's `Word` hands control to a controller before every operation, the controller picks the next thread from a fixed-seed generator (or exhaustively, for small cases), and the model's `wait` and `wake` are a futex with the kernel's semantics (atomic check and enqueue, spurious wakes injected at a fixed-seed rate). Each test checks an invariant on data guarded by the primitive (a non-atomic counter's final value, at most one writer, FIFO order through a channel, `Once` running its closure exactly once) and that no schedule deadlocks (every thread finishes within a step bound). It uses `std` in tests only.

**Acceptance.**

- `cargo test` in `tasks/`: each primitive exhaustively for 2 threads and 2 rounds, and under 10,000 fixed-seed schedules for 3 to 4 threads, with the invariants above; timeouts expire in the model's virtual time.
- Negatives, each a deliberately broken variant behind a test-only feature that the model must catch: an unlock that never wakes (a deadlock found), a condition variable that releases the mutex after reading the sequence (a lost wake found), a channel without its mutex (a lost or duplicated item found).
- `clippy -D warnings` with `tasks/clippy.toml` passes; the cross builds for both targets pass; the kernel-target build's disassembly has no `cmpxchg` (`objdump -d` in the CI image, stated in the PR).

**Depends on**: nothing. **Parallel with**: K1. **Labels**: `enhancement`, `rust`, `sdk`, `spike`.

**Risks.** A model that switches only at `Word` operations cannot see a bug that needs a switch between two non-atomic instructions, which on one CPU only matters for data the primitive does not guard; the review checks that every shared word goes through `Word`. Generic `Platform` types on every primitive are heavy for apps; A1 decides whether the SDK re-exports them with the Desk platform fixed. Where the crate finally lives (here, or in A1's SDK workspace) is A1's call; moving it is a path change.

## What R80b later changes

R80b (paging, ring 3, the numbered system calls; R78 and R77 stages 2 and 3) keeps the semantics built here and changes their plumbing:

- **K1.** Each thread gets a ring-0 stack for traps besides its ring-3 stack, and a TSS whose `ESP0` the switch updates; a ring-3 thread's interrupt frame lands on its ring-0 stack, so the preemption path is the same with one more field to switch. The page-aligned stacks get a guard page below. A task gains page tables and the switch loads `CR3` when the task changes. Kill also frees the page tables. `cli` in apps becomes a fault (`IOPL` 0), which retires the review rule; `tasks-nopreempt`'s trick stays possible only for kernel threads.
- **K2.** `desk_wait` and `desk_wake` become system calls; the wait table's key becomes (task, address), or the physical address for a shared buffer; the kernel must check that `addr` is a mapped, aligned user address before reading it. The `#NM` mechanism is unchanged (it is per thread, not per privilege level). Thread-local keys stay; native thread-locals through `GS` may arrive later with a TLS segment in the format.
- **K3.** The lock calls become system calls with the same checks; the lock record's key changes like the wait table's. Inheritance through messages is decided with the file service (decision 18), not here.
- **Y1.** Nothing: `desk-sync` sees only its `Platform`, whose functions become stubs that trap. That is the point of fixing the semantics in stage 1.

## New checks for the CI stack

For the CI rework to wire in (this plan adds them without touching `.github/workflows/*`):

- `tasks/check.sh` (host tests, lints and the two cross builds of `desk-sched` and `desk-sync`), called by one line in `tools/check.sh`; it touches only `tasks/`, so a scope script can skip it when `tasks/` and its dependants are unchanged.
- The "Tasks" section of `tools/native-test.sh`: modes 21, 22, 23, 29 and 30, an estimated 5 to 10 seconds each in QEMU. It belongs in the native group and runs when `kernel/` or `tasks/` changes.
- Optional, not in CI: a count of `cmpxchg` in `kernel.elf`, and Bochs runs of the task modes.

## Questions for the owner

Each with the default this plan builds if no answer comes before the task that needs it.

1. **Two more test-mode numbers.** 29 `tasks-nopreempt` and 30 `wait-nosave`, for negatives that need their own boot. Default: yes, recorded in the registry in the roadmap pull request.
2. **The x87 control word of a new thread.** `FNINIT`'s `0x037F` (64-bit precision) or Fizz 2's `0x027F` (53-bit, which matches `f64` and makes results the same as on other machines). Default: `0x027F` for app threads, because the float policy allows Fizz-style game code and that is what it expects; it is one constant in K2 and the `wait` mode checks whichever is chosen.
3. **The boot thread's priority.** Default: 16 until G2 turns the shell into the compositor task at 26, so K1's tests can share a level with it.
4. **Where `desk-sync` lives.** Default: `tasks/sync`, beside `desk-sched`, moved by A1 only if the SDK's workspace needs it.
5. **Two lock calls for inheritance** (`desk_lock_wait`, `desk_lock_wake`) beside `desk_wait` and `desk_wake`, rather than flags on `desk_wait`. Default: two calls; the system-call set grows by two names.

## Definition of done

The plan is done when, on `main`:

- `tasks/check.sh` runs `desk-sched`'s and `desk-sync`'s host tests (the deterministic model included, with its broken variants caught) and both cross builds, called from `tools/check.sh`;
- the native checks run `tasks`, `tasks-runaway`, `tasks-nopreempt`, `wait` and `wait-nosave` in QEMU as a 486 with their expected transcripts, and every pinned transcript is unchanged with the scheduler live in every mode;
- a `loop {}` thread loses the CPU every slice and its task is killed with memory as before; threads block and wake, sleep, time out, exit and join; two threads' x87 states do not leak; a higher-priority waiter boosts a lower owner and ageing lets a starved thread run;
- [`APP-MODEL.md`](APP-MODEL.md), [`HAL.md`](HAL.md) and [`NATIVE-BOOT.md`](NATIVE-BOOT.md) describe what was built, with what was not verified (anything on real hardware, any timing on a real 486, and stage 2's protection, which none of this provides: R80a is unprotected and labelled so).
