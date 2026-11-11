# Tasks, threads, apps and libraries

The design of what a program is on Desk: the vocabulary, the layers an app talks to, how programs are scheduled and threaded on each track, the window system as a task of its own, the shape of an app, the file kinds, and shared libraries as a concept for later. It ties together the owner's decisions of 2026-11-11 and places them on the roadmap: R77 (tasks, the scheduler and the system-call boundary), R78 (per-task address spaces), R79 (the app interface made real) and the items R141 to R146. The file format and the loader are [`LOADER.md`](LOADER.md); the Rust `std` target for apps is [`DESK-STD.md`](DESK-STD.md).

> **Status.** A design, written on 2026-11-11 against `main` at `eaede0f`. Nothing in it is built: the kernel has no tasks, no threads, no scheduler, no ring 3 and no system calls, and the hosted shell is one loop. The decisions below are the owner's; the numbers (priority levels, the time slice, sizes) are targets and say so. Nothing here has run on real hardware. None of it is in the scope of the loader spike (R48), which stays small.

## Contents

- [Decisions](#decisions)
- [Vocabulary](#vocabulary)
- [The three layers of the SDK](#the-three-layers-of-the-sdk)
- [Scheduling: preemptive native, cooperative hosted](#scheduling-preemptive-native-cooperative-hosted)
- [The staged path on the native track](#the-staged-path-on-the-native-track)
- [Threads](#threads)
- [Wait and wake](#wait-and-wake)
- [Priorities](#priorities)
- [Thread-local storage](#thread-local-storage)
- [The FPU and the kernel's own mutual exclusion](#the-fpu-and-the-kernels-own-mutual-exclusion)
- [The window system as a task](#the-window-system-as-a-task)
- [The shape of an app](#the-shape-of-an-app)
- [File kinds](#file-kinds)
- [Shared libraries: documented, not built](#shared-libraries-documented-not-built)
- [The Desk target for `std`](#the-desk-target-for-std)
- [Built, designed and open](#built-designed-and-open)
- [What changes in SDK.md and FORMATS.md](#what-changes-in-sdkmd-and-formatsmd)
- [Questions for the owner](#questions-for-the-owner)

## Decisions

Recorded by the owner on 2026-11-11, each with its reason in one line.

1. **Native is preemptive, with PIT time slices, in three stages; hosted is cooperative.** Preemption is the only cure for a runaway loop; DOS is not reentrant, so the hosted track cannot switch at an arbitrary instruction.
2. **The SDK defines preemptive semantics.** Code that is correct when it can be interrupted anywhere is also correct when it is interrupted only at yield points, and the reverse is false, so one app targets one model.
3. **Threads from the start, 1:1 kernel threads inside tasks, with cooperative green threads on the hosted track behind the same API.** Concurrency on one CPU (a worker reading a floppy while the window stays live) is worth having before parallelism exists.
4. **One wait and wake primitive, futex-style**, with mutexes, condition variables, semaphores and channels as library code. One kernel mechanism is small to build and to verify, and it is what Rust's `std` already builds its locks on ([`DESK-STD.md`](DESK-STD.md#stdsync-and-the-one-wait-and-wake-call)).
5. **Fully configurable priorities**, chosen over a few fixed levels. The owner wants the control; the costs are set out under [Priorities](#priorities).
6. **The window system is a separate system task that apps reach by messages**, in the manner of X11, Wayland and Windows. A crashing app cannot take the compositor with it.
7. **An app is a main loop that blocks for events**, with a small runtime library keeping the `app_start`, `app_event`, `app_stop` callback shape for those who want it. A loop is the natural shape for threads and blocking calls; callbacks remain a convenience.
8. **One container, two extensions (`.DMD` modules, `.DKX` applications); libraries documented, not built; the packer in Rust; `std` later through a platform layer of our own; no OS asset-pack container.** See [`LOADER.md`](LOADER.md#decisions).
9. **Basic priority inheritance for the futex-style locks**: a low-priority holder runs at the priority of the highest-priority waiter while it holds the lock. Without it a high-priority thread can wait indefinitely behind a medium one ([Priorities](#priorities)).
10. **Native stage order: paging and ring 3 first, then the system-call boundary.** Protection first, so a crash stays inside its task; the system calls then arrive on a working protected task as small stubs resolved by the same named imports.
11. **`std` apps keep inline `lock cmpxchg`**, which a real 486 has: DOSBox's 486 mode rejects dword `CMPXCHG` only in its normal and simple cores, so `std` apps' DOSBox runs use `core = dynamic`, and the strict normal core stays for `shell/`, `hal/` and the existing checks. No helper is imported. The owner first chose a helper and a trap and simplified after a fourth experiment ([`DESK-STD.md`](DESK-STD.md#cmpxchg-on-the-hosted-track), with the tables).
12. **The defaults of the first version of this design are accepted**: 32 levels with the policy under [Priorities](#priorities), a 10 ms slice, ageing, the thread limits, the 32-byte message records, the window system's first native stage in the kernel's address space, thread-local keys first ([Questions](#questions-for-the-owner) lists them with their values).

## Vocabulary

| Word | Meaning on Desk |
|---|---|
| **Module** | A `.DMD` file of kind 1: code placed into the address space of whoever loads it and called directly. The loader spike's subject; the hosted Desk's apps until tasks exist; later, drivers or extensions of the core |
| **Application** | A file of kind 2 (`.DKX`, a Desk executable): loaded as a new task, entered at `desk_main` |
| **Library** | A file of kind 3, reserved: code loaded once that satisfies other files' imports. Not built |
| **Task** | What the kernel isolates and accounts for: an address space (the shared one until R78), its threads, its memory ledger, its handles and windows, its priority ceiling. One running application is one task. Unix would call it a process |
| **Thread** | What the scheduler runs: a stack, saved registers, the x87 state, a priority, a thread-local block. Every task has at least one; the first runs `desk_main` |
| **System service** | A task that offers a service by messages: the window system first, later others (files, sound) if they earn it |
| **System call** | The small, stable entry into the kernel (R77): wait and wake, threads, memory, messages, time |

## The three layers of the SDK

What an app sees as "the SDK" is three layers, as on every modern system:

1. **A small, stable system-call ABI** (R77): threads, wait and wake, memory, time, messages and handles. Few calls, integers and pointers, versioned by the SDK number in the file's header. Before ring 3 these are ordinary calls through import slots; with ring 3 the same names resolve to stubs that trap into the kernel, and the app does not change ([`LOADER.md`](LOADER.md#address-space-assumptions)).
2. **System services reached by messages**: the window system (windows, drawing, events, input focus, the clipboard), and later any service that should survive an app's crash. Each has a message protocol, versioned on its own.
3. **A runtime library on top**, linked into the app: the `desk_*` convenience functions of [`SDK.md`](SDK.md) (`desk_window_new` becomes a function that sends a request and waits for its reply), mutexes, channels and the other primitives over wait and wake, the callback-shaped app runner, and later `std`. It is ordinary Rust, duplicated per app as each module's `core` already is (static linking, [below](#shared-libraries-documented-not-built)).

The rule that follows: an app's binary depends on layer 1's names and layer 2's protocols, never on the kernel's internals, and layer 3 can change between SDK versions without breaking old binaries, because each app carries its own copy.

## Scheduling: preemptive native, cooperative hosted

| | Native (target) | Hosted |
|---|---|---|
| When a thread can lose the CPU | At any instruction, when the PIT ends its time slice or a higher-priority thread becomes ready (an interrupt woke it) | Only inside a blocking call (`next_event()`, `wait`, a file read, `sleep`, a message receive) or at an explicit `yield_now()` |
| Who runs next | The highest-priority ready thread; round-robin within a priority | The same rule, applied at each switch point |
| A runaway loop (`loop {}` with no calls) | Loses the CPU at the end of its slice; the user can kill its task | Holds the machine until it returns. The hosted track cannot stop it (a possible later watchdog is below) |
| Locks | Real: a contended lock blocks the thread in the kernel | Never contended across a switch unless the holder blocked while holding it; the same code works |
| Interrupt handlers | Kernel code; they wake threads, never run app code | `rt`'s handlers; they set flags and fill rings, never switch threads (DOS is not reentrant) |

**What an app must know about both tracks.** Only this: write the code as if it could be interrupted anywhere (which the SDK's semantics already require), and do not rely on being interrupted. Concretely:

- Shared data between threads is protected by the SDK's locks or passed through channels, on both tracks. Code that "knows" it will not be interrupted is wrong on native and the SDK does not promise it anywhere.
- A long computation on the UI thread makes the app's own window stop answering on both tracks; on the hosted track it also stops every other app and the pointer. So long work goes on a worker thread, and a worker that loops for long without a blocking call calls `yield_now()` now and then. On native that call costs little and changes nothing about correctness; on hosted it is what lets the rest of the Desk run.
- There is no API that tells an app which track it is on for scheduling purposes, and none is needed.

**What each track guarantees.**

- Both: the SDK's locks, condition variables, semaphores and channels behave as their documentation says; a thread blocked in a call does not hold the CPU; events are delivered in order per window; a thread that wakes from `wait` re-checks its condition (spurious wake-ups are allowed, as with every futex).
- Native adds: no thread holds the CPU longer than its time slice while another of equal or higher priority is ready; the user can kill any task; from stage 2, a task's crash cannot damage another task's memory.
- Hosted adds nothing beyond the common list, and keeps S1's limits: one address space, panics contained by the guard ([`LOADER.md`](LOADER.md#containing-a-crash)), no protection from a wild write or a runaway loop.

**A possible hosted watchdog, untested.** The hosted IRQ 0 handler could notice that the same app thread has run for seconds without a switch and, if the interrupted `EIP` lies inside that app's image, escape it through the crash guard the way a panic does. Whether CWSDPMI's interrupt frames make that safe was not examined; it is an idea, not part of the plan.

## The staged path on the native track

| Stage | What is built | What it protects against | What it does not |
|---|---|---|---|
| 1. Preemption in ring 0 | Tasks and threads in the kernel's address space, a software context switch (save the general registers and `ESP`, switch stacks, restore) driven by the PIT at 100 Hz, the scheduler, wait and wake, sleep and timers | A runaway loop: the thread loses the CPU and its task can be killed | Memory damage: every task still shares the kernel's flat ring-0 space, as a module does today |
| 2. Paging and ring 3 (R78, with the paging spike R51) | Page tables per task, the image mapped at its link base (relocation becomes a no-op), read-only text, a guard page under every stack, ring-3 code and data segments, a TSS with a ring-0 stack per thread | A wild write and a stack overflow stay inside their task, which is killed cleanly | The calls into the kernel are not yet a stable boundary: the import stubs trap through one interim gate into the same kernel functions, without a numbered table or full argument checking |
| 3. The system-call boundary (R77) | `int 0x80` (or a call gate) with a numbered table on the working protected task; the import names resolve to small stubs that load the number and trap; arguments checked and copied by the kernel | An app can no longer call kernel internals or pass a pointer into kernel memory | Nothing new about scheduling; the semantics apps see are the stage-1 ones |

The order is the owner's: protection first, so a crash stays inside its task, then the system calls arrive on a working protected task as small stubs resolved by the same named imports. Each stage keeps the API the previous one exposed, which is the point of fixing the semantics now: an app built against stage 1 runs on stage 3 unchanged, because its imports are names, not addresses ([`LOADER.md`](LOADER.md#address-space-assumptions)).

**Killing a runaway task.** In stage 1 the kernel removes the task's threads from every queue at a point where none of them is inside the kernel (preemption only happens with the preemption counter at zero, so an interrupted thread is never mid-service), frees its memory through its ledger, closes its handles and tells the window system, which closes its windows. In stage 2 the same, plus its page tables. Locks inside the dead task die with it; a lock shared with another task would be abandoned held, which is why tasks share nothing but messages and explicitly shared buffers.

## Threads

- **Model.** 1:1: each SDK thread is a kernel thread on native and a green thread on hosted. No user-level scheduler on native.
- **Creation and exit.** `desk_thread_spawn(entry, arg, stack_size, priority) -> handle`; `desk_thread_exit(code) -> !`; `desk_thread_join(handle) -> code`. A thread's stack is allocated by the kernel (from stage 2 with a guard page below it); the default size is a target of 16 KiB, which the SDK's small-frames rule makes reasonable. When the first thread returns from `desk_main`, the task ends and its other threads with it, as a C `main` ends a process.
- **Sleep and timers.** `desk_sleep(ms)`, and a timeout on every blocking call. The kernel keeps sleeping threads in a list sorted by wake tick and checks its head on each PIT tick, so a sleep's resolution is the tick (10 ms at 100 Hz). Timers the SDK already describes (`desk_timer`) become messages delivered to the app's event queue. On hosted, sleep is "switch until the tick count passes", with `rt`'s timer.
- **How many.** No fixed limit in the design; the target is 64 threads per task and 256 in the system on an 8 MB machine, each costing its stack, a thread block and, if it uses the x87, 108 bytes of saved state.
- **Hosted green threads.** Each has its own stack from `rt`'s heap; a switch saves `EBX`, `ESI`, `EDI`, `EBP`, `ESP` and the x87 control word (the cdecl convention leaves the x87 stack empty at a call, so the rest of the x87 state need not be saved) and is a few dozen instructions. Switches happen only at the points listed above, all of which are calls the runtime makes, so a switch is never inside a DOS call or an interrupt handler.

## Wait and wake

The one blocking primitive, after Linux's futex:

```rust
/// Blocks while `*addr == expected`, until a wake on `addr` or the timeout. Returns at once if the value differs.
fn desk_wait(addr: *const u32, expected: u32, timeout_ms: u32) -> WaitResult;   // Woken, Changed, TimedOut
/// Wakes up to `count` threads waiting on `addr`; returns how many were woken.
fn desk_wake(addr: *const u32, count: u32) -> u32;
```

- **In the kernel**, the check of `*addr == expected` and the enqueue happen with interrupts off (one CPU, so that is atomic with respect to every wake). Waiters are kept in a small hash table keyed by address, each bucket a list in priority order. `addr` is a task address: in stage 2 the key becomes (task, address), or the physical address for a buffer two tasks share.
- **In the library**: a mutex is one `u32`; condition variables, semaphores, read-write locks, `Once` and channels are written over it. Without inheritance a mutex needs only exchange (lock: `swap(1)`, and if that saw a non-zero value, `swap(2)` and wait while it does not return 0; unlock: `swap(0)`, and if that saw 2, wake one); with the decided basic inheritance the word holds the owner's thread id and a waiters bit, and the contended paths go through the kernel ([Priorities](#priorities)). Either way the SDK's own library stays free of `CMPXCHG`, so it also runs under DOSBox's strict normal core; `std`'s locks use it inline ([`DESK-STD.md`](DESK-STD.md#cmpxchg-on-the-hosted-track)); `std`'s own locks use compare-and-swap, which is the subject of a question in [`DESK-STD.md`](DESK-STD.md#questions-for-the-owner).
- **On hosted**, `desk_wait` with an unchanged value parks the green thread and switches; `desk_wake` moves waiters to the ready list. The same semantics, without a kernel.

## Priorities

**The model.** Every thread has a priority from 0 (lowest) to 31 (highest): 32 levels, a ready queue per level and a 32-bit bitmap of non-empty queues, so picking the next thread is one `BSR` (a 386 instruction). Within a level, round-robin with a 10 ms time slice. Priorities are fully configurable in that an app may set any of its threads to any level up to its task's ceiling, and the user (Settings, or a task list) may change any task's ceiling and any thread's level.

**The default policy (a target).** Levels 24 to 31 are the system band: the input thread (27), the window system's compositor (26), the kernel's own workers. Apps run at 8 to 23 with 16 as the default for an app's threads and 12 for workers it starts; 0 to 7 are for background work that should only use idle time. A task's ceiling defaults to 23, so no app can outrank input or the compositor unless the user raises it.

**The time slice.** 10 ms, one tick of the PIT at the 100 Hz the kernel already programs (`kernel/src/pit.rs`). At 25 MHz that is 250,000 cycles per slice; a context switch is a few hundred instructions without the x87 (an estimate; nothing is measured), well under 1%. A shorter slice costs more switches and buys little, because input and the compositor do not wait for a slice to end: their interrupts wake them and, being higher, they preempt at once.

**What full configurability costs, honestly.**

- **Starvation.** Strict priorities mean a busy thread at 20 starves every thread below it forever. The default mitigation is ageing: a ready thread that has not run for 1 second (100 ticks) is raised one level per further second, up to its task's ceiling, and drops back when it runs. It costs a scan of the lower queues once a second. Without ageing, configurable priorities let one app silently freeze others; with it, the configuration is a preference, not an absolute.
- **Priority inversion and basic inheritance (decided).** With futex locks the kernel does not know who holds a lock unless the lock word says, so a high-priority thread waiting on a lock held by a low-priority thread, while a medium-priority thread runs, would wait as long as the medium one likes. The decision is **basic priority inheritance**: while a thread holds a lock that a higher-priority thread waits on, it runs at the waiter's priority, and drops back when it releases the lock. What it costs:
  - **The lock word holds the owner.** A locked mutex stores the holder's thread id (with a waiters bit), not just "locked", so the kernel can find whom to boost. The library's lock writes its own id; with exchange only (no compare-and-swap) the uncontended acquire is `swap(own id)` and a contended one falls into the kernel, which rewrites the word under interrupts off, so the fast path stays one instruction and the cost is on the contended path.
  - **The wait and wake path.** A contended `desk_wait` on a lock word becomes a lock-wait: the kernel reads the owner from the word, records that the waiter waits on it, and if the waiter's priority is higher, raises the owner's effective priority and moves it to that ready queue. On release the owner calls into the kernel only if the waiters bit is set; the kernel hands the lock to the highest waiter, wakes it, and recomputes the old owner's effective priority from the locks it still holds. That is two small lists per thread (locks held, lock waited on) and a recomputation over the locks held, which is short when a thread holds few locks.
  - **The scheduler.** Each thread has a base priority and an effective one; the ready queues use the effective one, ageing works on the base, and a priority change of a thread that holds locks propagates to nothing beyond its own effective value.
  - **What "basic" means.** One level: the owner is raised to its direct waiters' highest priority. **No transitive chains in the first cut**: if the owner itself waits on another lock, that lock's owner is not raised. Chains are rare when locks are held briefly and not nested, they need a walk with a depth limit and a deadlock check, and they can be added later without changing the ABI. Only the SDK's mutex inherits; semaphores, condition variables and channels do not (they have no single owner), and condition variables re-acquire their mutex, which inherits.
  - **Hosted.** Priorities mean run-queue order only, so inheritance is ordering only: when a green thread blocks on a lock held by a lower-priority green thread, the holder's effective priority is raised in the ready list, so it runs before the medium ones at the next switch point and releases the lock sooner. The same lock word and the same code serve both tracks.
  The system is still arranged so the inversion that matters most cannot happen: the input thread and the compositor share no lock with any app and talk to apps through message queues, whose lock is held for a few instructions with interrupts off.
- **Protecting input and compositor latency.** Their place in the system band, ceilings that keep apps below them, no shared locks, and interrupts that wake them directly: the keyboard and mouse interrupts fill the input ring and wake the input thread, which preempts whatever app is running.
- **Complexity.** Thirty-two queues, a bitmap, ageing, ceilings and basic inheritance are a few hundred lines in the scheduler and the wait table. The simpler fixed-levels design would have been perhaps half that; the difference is the cost of the owner's choice, and it is small.

**On the hosted track**, priorities order the ready list the cooperative scheduler picks from at each switch point: the highest ready green thread runs next, so after any blocking call the window service (high) runs before an app worker (low). They never preempt, and ageing is applied the same way at switch points. A priority on hosted is therefore a promise about order, not about latency.

## Thread-local storage

Every thread has a thread-local block. Two mechanisms, staged:

- **Keys first.** `desk_tls_alloc() -> key`, `desk_tls_get(key) -> u32`, `desk_tls_set(key, value)`, `desk_tls_free(key)`: a small array per thread, indexed by key. This is the "OS key" form `std` falls back to when a target has no native thread-locals, and it is exactly one of the four pieces [`E-STD-STUBS.md`](E-STD-STUBS.md#1-does-std-build-for-an-invented-os) found missing for an invented OS (the thread-local keys); the Desk platform layer implements it over these calls ([`DESK-STD.md`](DESK-STD.md)). It needs nothing from the file format.
- **Native thread-locals later.** Rust's `#[thread_local]` (and `std`'s fast path, `has-thread-local: true`) on x86-32 reaches the block through `GS`. That needs a TLS template in the file (`.tdata` and `.tbss`, and `R_386_TLS_*` relocations) and a `GS` descriptor per thread, switched with the thread. The format's version 1 has no TLS segment and the packer refuses one ([`LOADER.md`](LOADER.md#building-a-module)); adding it is a compatible header addition with a flag bit, when the speed matters.

## The FPU and the kernel's own mutual exclusion

- **Lazy x87 save.** On a switch the kernel sets `CR0.TS` instead of saving the x87; the first x87 instruction of the new thread raises `#NM` (device not available), whose handler saves the previous owner's state with `FNSAVE` (108 bytes; the 486 has no `FXSAVE`), restores this thread's with `FRSTOR`, clears `TS` and returns. Threads that never touch the x87, which is most of them under the integer-only rules of the core, never pay. A new thread starts with `FNINIT` and the core's control word. Whether the SDK promises the x87 to apps at all is R138's open decision; this mechanism serves either answer.
- **Mutual exclusion in the kernel is not spinlocks.** On one CPU a spinning thread waits for a holder that cannot run while it spins. The kernel uses two tools: **interrupts off** for the short sections an interrupt handler also touches (the ready queues, the wait table, the input rings: the kernel already does this, `kernel/src/sync.rs`), and a **preemption-disable counter** for longer sections that only threads touch, during which interrupts are still serviced but the PIT does not switch threads; the switch it wanted happens when the counter returns to zero. A kernel service that must wait releases both and blocks through the scheduler.

## The window system as a task

**The shape.** The window system is a system task that owns the screen, the palette, the pointer and the input focus. Apps never draw to the frame buffer: they send it requests and receive events, as X11, Wayland and Windows' window manager do. The `desk_window_*`, `desk_fill`, `desk_text` and related calls of [`SDK.md`](SDK.md) become client requests in the runtime library.

- **Requests** (app to window system): create a window (title, size, flags) and get its handle; set its title; show, close; describe its menus, toolbar and controls; and drawing. Drawing is either retained requests (`fill`, `line`, `text`, `blit` in a request buffer, executed by the window system against the window's back buffer) or a client buffer the app draws into and then presents with a damage rectangle; the first stage uses requests, because they need no shared memory.
- **Events** (window system to app): the `DeskEvent` records of [`SDK.md`](SDK.md) (paint, mouse, key, char, menu, control, resize, focus, close), queued per app and read by `next_event()`.
- **Replies**: a request that returns a value (a new window's handle) is answered on the app's event queue with a matching serial number, and the library call waits for it.
- **Transport**: one message queue per direction per client, each a ring of fixed-size records with a variable-size payload area, guarded by the kernel's wait and wake. The message format is an open question ([below](#questions-for-the-owner)); the default is fixed 32-byte records with an opcode, a window handle, a serial number and five integer arguments, and strings or pixels in a payload area the record points into.

**A crash.** When an app's task dies (a panic, a fault, a kill), the window system receives a "client gone" notice, destroys the client's windows and repaints what they covered. Its own state is not touched, because nothing of the app's runs in it.

**Native, first stage.** The window system runs as a task in the shell's (the kernel's) address space, in ring 0, with the message queues in shared memory that both sides reach by pointer. It is already a separate task with its own threads and its own state, and apps already reach it only through the queues, so when stage 2 gives it an address space of its own, only the transport changes (the queues become pages shared between two address spaces, or the kernel copies the records), not the API. Until stage 2 a wild write in an app can still damage it, as everything else.

**Hosted.** The window system is a cooperative service in the same DOS process: a high-priority green thread running the same code over the same queues. A request is a push onto its queue and a wake, and it runs at the app's next switch point. A panic in an app is contained by the guard and turns into the same "client gone" notice; a wild write is not contained, as on native stage 1.

## The shape of an app

```rust
// An application (`.DKX`): the first thread runs desk_main and loops on events.
fn desk_main(args: &str) -> i32 {
    let win = desk::Window::new("Notes", 320, 200, desk::Flags::RESIZABLE)?;
    loop {
        match desk::next_event() {          // blocks; other threads and other tasks run meanwhile
            Event::Paint(p) => draw(&win, p),
            Event::Key(k) => edit(k),
            Event::Close(_) => return 0,
            _ => {}
        }
    }
}
```

The runtime library also provides `desk::run(App { start, event, stop })`, which is that loop calling three functions, so a module written in the callback shape of [`SDK.md`](SDK.md#an-apps-entry-points) builds as an application with one line. The loader spike's modules use the callback entry points directly, because modules have no task and no thread of their own ([`LOADER.md`](LOADER.md#format-specification-sketch-dmd)).

## File kinds

| Kind | Extension | Loaded as | Imports resolve to | Entry | Built in |
|---|---|---|---|---|---|
| 1 module | `.DMD` | Placed in the caller's address space and called | The caller's functions | `app_start`, `app_event`, `app_stop` | R48 |
| 2 application | `.DKX` | A new task, its first thread at `desk_main` | Stubs: plain calls before ring 3, system calls after | `desk_main` | R144, on R77 |
| 3 library | `.DMD` | Placed once, shared by the files that import from it | Other libraries' exports, then the system's | None; an export table | R145, an idea |

The container, the relocation and the binding are the same for all three ([`LOADER.md`](LOADER.md#format-specification-sketch-dmd)); only what the loader's caller does with the result differs.

## Shared libraries: documented, not built

**The idea, in two familiar forms.**

- **Windows DLLs.** A PE file has an **export table** (names and ordinals to addresses) and an **import table** (for each DLL it needs, the names it imports and an import address table the loader fills). Each DLL is linked at a **preferred base**; if that address is taken, the loader **relocates** it with its base-relocation table, and the relocated pages can no longer be shared with other processes that loaded it at the preferred base. Versioning was by file name and convention, and the failure of that is **DLL hell**: two apps needing incompatible versions of one `MSVCRT.DLL` in one system directory.
- **Linux `.so`.** Shared objects are **position-independent ELF**: code reaches its data through the **GOT** (a table the dynamic linker fills) and other libraries' functions through the **PLT** (stubs that jump through GOT entries), so the code pages never need relocating and are shared by every process. **`ld.so`**, the dynamic linker, loads them, resolves symbols in load order and applies the GOT relocations. **SONAME** (`libfoo.so.2`) names the compatible major version, and **symbol versioning** lets one library carry old and new versions of a function.

**What a `library` kind would add to `.DMD`.** The export table version 1 reserves (names to image offsets, sorted); imports that name a library as well as a symbol (or a search order: loaded libraries first, then the system's table); a loader that loads a library once per address space and counts its users; and versioning through the SDK version and a library version in the header, refused at load when a file needs a newer one, so the failure is a clear message rather than DLL hell's silent mismatch. The relocation scheme stays as it is: a library is linked at a preferred base like everything else.

**Why static linking is Rust's natural default.** Rust has no stable ABI between crates: a `Vec`, a trait object or a generic function compiled by one rustc version cannot be used by code from another, and generics are instantiated in the crate that uses them, so most of a Rust library's code is not even in the library. Only `extern "C"` functions with C-shaped arguments cross a binary boundary safely. So each Desk app links its own `core`, `alloc` and runtime library (as each DXE3 module did, [`S1-DXE3.md`](S1-DXE3.md#4-rust-runtime-in-the-module)), and a shared library would have to export a C-shaped interface designed for it.

**Why it would matter on this machine.** On 4 to 16 MB, a component every app links (the UI toolkit, `std` once apps use it) duplicated in ten running apps is ten copies. A shared library is one.

**Why it needs paging.** Sharing means the same physical pages appear in several address spaces, which needs per-task page tables (R78). In today's single flat space a library could be loaded once and called by everyone, but that is only sharing an address space, which is the thing the staged path is moving away from. And if a library has to be relocated in one task because its preferred base is taken, those pages differ and cannot be shared, which is why Windows prefers distinct bases per DLL and Linux uses position-independent code.

**The trigger for building it.** A large shared component that most apps link and whose duplication is measured to matter: the UI toolkit, or `std`. Until then, nothing is built; the reserved fields cost twelve bytes of header.

## The Desk target for `std`

`std` is the later direction for apps: through the patched platform layer of the `std` experiment, in a sysroot of our own, over the same `desk_*` names a `.DMD` or `.DKX` imports. The design is long enough for its own document, [`DESK-STD.md`](DESK-STD.md): one target for both tracks (`i486-unknown-desk`), its specification against the kernel's, what the OS must provide (the allocator, stdio and files, time, the random seed, arguments, environment and exit, threads and thread-local keys), how `std::sync` gets every lock from the one wait and wake call, what following each Rust release costs, the staging (the loader spike stays `no_std`; the hosted shim first; native after tasks and threads), and its own questions.

## Built, designed and open

| Part | Built | Designed | Open |
|---|---|---|---|
| The file format and loader | Nothing (R48 is planned in [`LOADER-PLAN.md`](LOADER-PLAN.md)) | [`LOADER.md`](LOADER.md) | — |
| Tasks and the preemptive scheduler (R77, R142) | Nothing; the kernel has a PIT at 100 Hz and interrupt-safe heap and frames | Here: three stages, 32 levels, ageing, 10 ms slices, basic priority inheritance | — (defaults accepted) |
| Threads and wait and wake (R141) | Nothing | Here | — (defaults accepted) |
| Per-task spaces and ring 3 (R78) | Nothing; the paging spike R51 is planned | Stage 2 here, before the system calls (decided) | The interim gate's form |
| The system-call boundary (R77) | Nothing | Stage 3 here | `int 0x80` or a call gate; the numbering |
| The window system as a task (R143) | The compositor and window manager as crates (`shell/`), called directly by the shell | Here: requests, events, queues, the hosted and native first stages | Retained requests or client buffers first |
| The app shape and runtime library (R79) | Nothing | Here | Whether `fn main` or `desk_main` is the entry once `std` arrives |
| Applications as files (R144) | Nothing | [`LOADER.md`](LOADER.md) (kind 2, `.DKX`, decided) | — |
| Shared libraries (R145) | Nothing | Concept only, here | Everything; not time yet |
| `std` for apps (R146) | The experiment E (`spikes/e-std-stubs/`, DOSBox only) | [`DESK-STD.md`](DESK-STD.md) | [`DESK-STD.md`](DESK-STD.md#questions-for-the-owner) |
| Hosted cooperative threads | Nothing; `SHELL.EXE` is one loop | Here | The watchdog idea, untested |

## What changes in SDK.md and FORMATS.md

Not rewritten now; R79 (and R48's result for the format) make these edits.

**[`SDK.md`](SDK.md):**

- The entry points: `desk_main` for applications, the callback shape kept for modules and through `desk::run`; no COFF underscore; imports are slots bound by name, not `dlregsym`.
- `desk_window_*`, `desk_fill`, `desk_line`, `desk_text`, `desk_blit`, `desk_icon`, menus, toolbars, tabs, status bars, popups and pickers: client requests to the window system, with replies for the calls that return a value.
- Events: a blocking `next_event()` (with a timeout form) instead of `app_event` being called; the same `DeskEvent` record.
- Threads: `desk_thread_spawn`, `desk_thread_exit`, `desk_thread_join`, `desk_sleep`, `yield_now`, priorities and ceilings, thread-local keys.
- Wait and wake: `desk_wait`, `desk_wake`, and the library's mutex, condition variable, semaphore, read-write lock and channel over them.
- Memory: `desk_alloc(size, align)` and `desk_free(ptr, size, align)` ([`LOADER.md`](LOADER.md#decisions)).
- The rules: preemptive semantics on both tracks; long work on a worker; `yield_now()` in long loops; the x87 rule as R138 settles it; on hosted, a runaway loop holds the machine.
- The SDK version: what it covers (the system-call names and the service protocols' versions).

**[`FORMATS.md`](FORMATS.md):**

- The container's header (96 bytes, [`LOADER.md`](LOADER.md#format-specification-sketch-dmd)), its tables and the `kind` values.
- Both extensions: `.DMD` for modules and libraries, `.DKX` for applications; `APP.INF` names the one the app ships.

## Questions for the owner

**Decided** (see [Decisions](#decisions)): the application extension (`.DKX`), basic priority inheritance, and the native stage order (paging and ring 3, then system calls).

**Defaults accepted by the owner**, as the design builds them:

1. **Priority levels and policy:** 32 levels (0 to 31), a system band of 24 to 31 (input 27, compositor 26), apps from 8 to 23 at 16 by default and workers at 12, a task ceiling of 23, background work at 0 to 7.
2. **The time slice:** 10 ms, one PIT tick at the kernel's existing 100 Hz.
3. **Ageing:** on, one level per second of waiting after the first second, up to the task's ceiling.
4. **Threads per task:** no hard limit in the design; a target of 64 per task and 256 in the system, with 16 KiB default stacks.
5. **The message format:** fixed 32-byte records (opcode, window handle, serial, five integers) with a payload area for strings and pixels, one queue per direction per client.
6. **The window system's first native stage:** a task in the kernel's address space, reached only through the queues, moving out with stage 2 without an API change.
7. **Thread-local storage:** keys first, native thread-locals later with a TLS segment in the format.

**Still open**, each with its default:

1. **The entry once `std` arrives.** Default: `desk_main` stays the file's entry, and a `std` app's `fn main` is called from it through `lang_start` ([`DESK-STD.md`](DESK-STD.md#questions-for-the-owner)).
2. **Retained drawing requests or client buffers first.** Default: requests first (no shared memory needed); client buffers when stage 2 can share pages.
3. **The interim gate in stage 2.** Default: one software-interrupt vector that the import stubs use to reach the existing kernel functions, replaced by stage 3's numbered table.
