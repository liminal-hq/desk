# Prior art: what earlier systems teach Desk

Desk is starting from scratch on hardware that Windows 3.x, Windows 95, OS/2 and their contemporaries were written for, and it can look back on how they turned out. This note records what those systems did well and badly, sourced, and what Desk should take or avoid, tied to the decisions in [`APP-MODEL.md`](APP-MODEL.md), [`LOADER.md`](LOADER.md), [`DESK-STD.md`](DESK-STD.md), [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md) and the items of [`ROADMAP.md`](ROADMAP.md). It is roadmap item R147.

> **Status.** Research, not an experiment, written on 2026-11-11 against the loader and app-model design branch (R48, R77, R141 to R146), which is not yet merged. Nothing was built or measured for it. Every historical or technical claim about another system links to the page it comes from; those pages are listed under [Sources](#sources), and most are archived Microsoft, Apple, Be, QNX and Bell Labs documents or Wikipedia. Each "What it means for Desk" paragraph is an inference about Desk, not a sourced fact, and says which decision or item it applies to. Where a well-known claim could not be sourced, it is left out or marked "as usually told". The owner decided the note's eight questions on 2026-11-11; they are recorded under [The owner's decisions](#the-owners-decisions).

## Contents

- [Windows 3.x, 95, 98 and ME](#windows-3x-95-98-and-me)
- [AmigaOS](#amigaos)
- [BeOS and Haiku](#beos-and-haiku)
- [OS/2 Presentation Manager](#os2-presentation-manager)
- [Classic Mac OS](#classic-mac-os)
- [Windows NT](#windows-nt)
- [QNX and Photon](#qnx-and-photon)
- [X11 and Wayland](#x11-and-wayland)
- [RISC OS](#risc-os)
- [Plan 9](#plan-9)
- [Cross-cutting lessons](#cross-cutting-lessons)
- [The lessons in one table](#the-lessons-in-one-table)
- [The owner's decisions](#the-owners-decisions)
- [Sources](#sources)

## Windows 3.x, 95, 98 and ME

Windows 9x is the closest relative Desk has: a 32-bit protected-mode system that started from DOS, kept 16-bit code and DOS programs running beside new 32-bit ones, and was replaced by a system designed for protection from the start.

**Enhanced mode and the virtual machines.** Windows 3.0 (May 1990) had a 386 enhanced mode that was "a 32-bit virtual machine that ran a copy of 16-bit Standard mode" and ran "multiple copies of MS-DOS in virtual 8086 mode", so DOS programs could run windowed beside Windows programs ([Windows 3.0](https://en.wikipedia.org/wiki/Windows_3.0)). Windows 95 kept the shape: its Virtual Machine Manager (VMM) is "a 32-bit protected-mode operating system" that creates the system virtual machine when it starts, and Windows then runs inside it ([Win95 DDK: Virtual Machine Manager](https://techshelps.github.io/MSDN/WIN95DDK/HTML/S22E9.HTM)). Windows 95 creates "a separate virtual machine (VM) for each MS-DOS-based application" and shares the processor among those VMs and "the system VM (which contains all Windows-based applications)" ([MS-DOS-Based Applications](https://techshelps.github.io/MSDN/WINBASE/devdoc/live/pdwbase/win95/chiarc_7p4j.htm)). The VMM itself "provides single-threaded, pre-emptive multitasking" between virtual machines ([Win95 DDK](https://techshelps.github.io/MSDN/WIN95DDK/HTML/S22E9.HTM)).

**Cooperative and preemptive side by side.** In Windows 95, "all 32-bit applications are scheduled preemptively", while 16-bit applications keep the cooperative model of Windows 3.x and share one address space; a 16-bit program that stops yielding "may cause Windows to hang as well", and a 32-bit program that calls a 16-bit API can be caught by it ([KB Q117567](https://www.infania.net/misc/kbarchive/kb/117/Q117567/index.html)). Each Win32 thread could have its own message queue, but "all Win16-based applications share a common message queue" ([Windows 95: User](https://techshelps.github.io/MSDN/DNWIN95/HTML/S72C4.HTM)). Raymond Chen describes the effect at the bottom: input that arrives while the system is busy is appended to a queue, and if a 16-bit application hangs, that buffer fills and "the window manager emits a little tick sound as a mini-error message" ([Windows 95's ticking death](https://devblogs.microsoft.com/oldnewthing/20090918-00/?p=16663)).

**The Win16 mutex.** Windows 3.x's 16-bit components were never written to be re-entered, because nothing preempted them. Windows 95 let preemptive Win32 threads call them through thunks, so it guarded them with Win16Mutex, "a global semaphore that is used to protect the 16-bit Windows 95 components", which lets one thread at a time into the 16-bit code ([KB 125867, Understanding Win16Mutex](https://ftp.zx.net.nz/pub/archive/ftp.microsoft.com/MISC/KB/en-us/125/867.HTM)). The same article spells out the cost. A Win16 process takes the mutex when it receives a message and gives it up only when it yields, so one that never yields holds it indefinitely; most 32-bit USER and GDI functions thunk to 16-bit code and take the mutex; and a Win32 thread that needs it then sleeps until the 16-bit program yields. Its advice to developers was to keep the UI on one thread and do useful work on threads that avoid UI and graphics calls. The lock was the bridge that let old code run under a new scheduler, and it also carried the old code's hangs into the new code.

**Thunking.** Windows 95 needed a special compiler and linker that understood both the 16-bit and 32-bit worlds for two of its own DLLs ([Raymond Chen, Windows 95 Unplugged](https://learn.microsoft.com/en-us/previous-versions/technet-magazine/cc742531(v=msdn.10))). Applications had two documented mechanisms: generic thunks, which let a 16-bit application call a Win32 DLL and work on both Windows 95 and NT, and flat thunks, which go both ways but exist only on Windows 95 and 98 and need a thunk compiler, with the warning "these thunks are not portable to other platforms" ([KB 125710](https://ftp.zx.net.nz/pub/archive/ftp.microsoft.com/MISC/KB/en-us/125/710.HTM)). Win32s had a third, universal thunks, for Win32 programs on Windows 3.1 ([Q125710 on techshelps](https://techshelps.github.io/MSDN/KB/Source/win32sdk/q125710.htm)).

**VxDs and the DOS underneath.** VxDs were the driver model from Windows/386 2.x to the 9x line; they "have access to the memory of the kernel and all running processes, as well as raw access to the hardware", and they cannot be used on Windows NT ([VxD](https://en.wikipedia.org/wiki/VxD)). The VMM and the virtual devices "run in a single, 32-bit, flat model address space at privilege level 0" ([Windows 9x](https://en.wikipedia.org/wiki/Windows_9x)). Chen describes MS-DOS in Windows 95 as two things: the boot loader, and "the 16-bit legacy device driver layer"; once the 32-bit file system took over, file operations went to it, but DOS stayed so that programs which patched it kept working ([What was the role of MS-DOS in Windows 95?](https://devblogs.microsoft.com/oldnewthing/20071224-00/?p=24063)).

**The shared arena and protection.** Microsoft's own map of the Windows 95 address space has four arenas: a DOS compatibility arena (0 to 4 MB) that holds the current VM's first megabyte; a private arena per Win32 process (4 MB to 2 GB); a shared arena (2 to 3 GB), "readable and writable by Win32 and 16-bit processes alike", holding the 16-bit global heap with every 16-bit application and DLL, the shared system DLLs `USER32`, `GDI32` and `KERNEL32`, and views of file mappings; and a reserved system arena (3 to 4 GB) for the VMM and VxDs that is "not addressable by ring 3 code" ([KB 125691](https://ftp.zx.net.nz/pub/Patches/ftp.microsoft.com/MISC/KB/en-us/125/691.HTM)). Windows 9x also "does not protect the first megabyte of memory from userland applications for compatibility reasons" ([Windows 9x](https://en.wikipedia.org/wiki/Windows_9x)). So a Win32 process was protected from another Win32 process's private memory, but any program could write the shared system DLLs' data and the whole 16-bit world.

**DLL hell.** Runtime DLLs were distributed as shared system components, an installer could downgrade a system library, and some tools did not add a version resource to the libraries they built; the fixes were Windows File Protection, side-by-side assemblies that load a separate copy per application, and private DLLs in the application's own folder ([DLL Hell](https://en.wikipedia.org/wiki/DLL_Hell)).

**Compatibility at any cost.** As Joel Spolsky tells it, SimCity "used memory right after freeing it", and Windows "added special code that checked if SimCity was running" and ran the allocator "in a special mode in which you could still use memory after freeing it" ([How Microsoft Lost the API War](https://www.joelonsoftware.com/2004/06/13/how-microsoft-lost-the-api-war/)).

**ME and the end of the line.** Windows ME (retail September 2000) restricted access to real-mode DOS "to decrease system boot time", so programs that needed real mode, and real-mode drivers such as `ANSI.SYS`, no longer worked in the default setup ([Windows Me](https://en.wikipedia.org/wiki/Windows_Me)). It was the last of the line before Windows XP, which brought the consumer and business systems together on Windows NT ([Windows 9x](https://en.wikipedia.org/wiki/Windows_9x)).

**What it means for Desk.**

- **Never let the hosted world's lock reach the kernel core.** The Win16 mutex existed because preemptive code had to call into code that assumed it could not be interrupted, and the fix serialised the new code behind the old. Desk's decision that the SDK defines preemptive semantics on both tracks (APP-MODEL decision 2) means no Desk app is the "16-bit code" of that story. The remaining risk is the DOS side: a FreeDOS V86 task (R49) and the BIOS trampoline. The trampoline is already a whole-machine lock (interrupts off, PICs reprogrammed for the length of a call, [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#interrupts-the-pics-the-timer-and-the-keyboard)); that is acceptable for mode sets and early disk reads, and the reason the V86 upgrade exists.
- **Thunks stay narrow and invisible to apps.** The trampoline (`NativeBios`) and `rt`'s DPMI 0300h calls are thunks in Windows' sense: a register block, a bounce buffer below 1 MiB, pointers translated by copying. Windows' flat thunks show what happens when that layer becomes an application interface: a non-portable mechanism that had to be supported. Desk's `Bios` trait and the rule that the core reaches the machine only through the HAL ([`HAL.md`](HAL.md)) keep the thunk below the SDK; the SDK should never offer an app a BIOS or DOS call.
- **The BIOS is a fallback, not the driver layer.** Windows 95 kept DOS as its legacy driver layer for compatibility, and ME's removal of it broke things. Desk's native track should treat the trampoline as temporary for each service it covers, replaced by native drivers (R74 for disks), so there is never a set of programs that depend on it.
- **No shared writable arena.** Stage 1 of the native path is one shared ring-0 space by design, which is Windows 9x's shared arena for everything. Stage 2 (R78) should not keep a writable region mapped into every task for convenience. The trampoline's identity-mapped low megabyte must stay supervisor-only, as [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#paging) already says, and the window system's queues should become pages shared by exactly the two parties.
- **Libraries refuse rather than mismatch.** Static linking by default and a version refused at load ([`APP-MODEL.md`](APP-MODEL.md#shared-libraries-documented-not-built), R145) avoid DLL hell's failure mode; a shared library, if it comes, belongs to one task's import list, never to a system directory that installers overwrite.
- **No per-application special cases.** Compatibility comes from a versioned SDK and service protocols, not from the kernel recognising a program.

## AmigaOS

**What it was.** Exec, the Amiga kernel first released in 1985 and designed by Carl Sassenrath, is "a 13 KB multitasking microkernel which enabled pre-emptive multitasking in as little as 256 KB of memory", with "prioritized round-robin scheduling" ([Exec (Amiga)](https://en.wikipedia.org/wiki/Exec_(Amiga))). There was one address space and no memory protection between tasks, and a program could stop multitasking by calling `Forbid()` or `Disable()` and never calling `Permit()` or `Enable()` (same source). Messages were not copied: a message is "a temporary license for the receiving task to use a portion of the memory space of the sending task", and the sender "should not access the message until it has been replied" ([Exec Messages and Ports](https://wiki.amigaos.net/wiki/Exec_Messages_and_Ports)). The Guru Meditation alert appeared "when there was a fatal problem with the system" ([Guru Meditation](https://en.wikipedia.org/wiki/Guru_Meditation)).

**What it means for Desk.** Stage 1 of the native path is the Amiga's situation: preemption, priorities and message queues in one unprotected space. The Amiga shows it is a good machine to use (preemption and cheap messages on very little memory) and where it stops: a wild write or a stray `Forbid()` takes everything down. Two things follow. First, in stage 1 every app runs in ring 0 and can execute `cli`, which is `Disable()` by another name; that is acceptable for a short stage, but the SDK must never export a call that disables preemption or interrupts, so nothing depends on it when ring 3 arrives. Second, the Amiga's messages lend memory by pointer, which is cheap only while there is one address space. Desk's queues carry copied fixed-size records with a payload area ([`APP-MODEL.md`](APP-MODEL.md#the-window-system-as-a-task)), and the owner has decided that large data such as pixel buffers is shared rather than copied ([decision 3](#the-owners-decisions)); the Amiga's lesson is that sharing needs explicit ownership, which is what that decision asks of the design.

## BeOS and Haiku

**What it was.** In BeOS "each window has its own client-side thread" ([Be Newsletter 1-4, Peter Potrebic](https://haiku-os.org/legacy-docs/benewsletter/Issue1-4.html)); creating a window establishes a connection to the Application Server, which "receives drawing instructions from the application and interprets them to render images within windows" and sends the application keyboard and mouse messages ([The Interface Kit](https://www.haiku-os.org/legacy-docs/BeBook/TheInterfaceKit_Overview_Introduction.html)). Haiku's app_server keeps the shape on its side: "When a BWindow is created, it spawns a ServerWindow object to handle the new window" ([Haiku app_server: graphics](https://haiku-os.org/docs/develop/servers/app_server/graphics.html)). The cost was locking: Be's own engineers wrote that "whenever a window thread locks another object, it's holding at least two locks, opening the door for a deadlock", with two windows locking each other in opposite orders as the example, and the remedy "acquiring the locks in a consistent order" ([Be Newsletter 1-4](https://haiku-os.org/legacy-docs/benewsletter/Issue1-4.html)).

**What it means for Desk.** Desk takes the server half (the window system as a task, R143) and not the mandatory thread per window: an app is one blocking main loop with workers it chooses to start (APP-MODEL decision 7). That keeps a single-window app free of locks and leaves concurrency where the app asks for it. The Be lesson stays relevant for an app with several windows and workers: the SDK's documentation should give one lock-order rule (the app's own locks before any call that waits on the window system), and the window system should never call back into an app synchronously, so an app's lock can never be part of a cycle with the server's.

## OS/2 Presentation Manager

**What it was.** Presentation Manager arrived in OS/2 1.1 in late 1988, and its known weakness was "the single input queue": "a non-responsive application could block the processing of user-interface messages, thus freezing the graphical interface"; OS/2 later addressed it in a FixPack "using a timer to determine when an application was not responding to events" ([Presentation Manager](https://en.wikipedia.org/wiki/Presentation_Manager)). Windows 3.x had the same arrangement ("all applications receive input from the same queue", and "a 16-bit Windows application can lock up the subsystem by halting the queue"), while under Windows NT "each application has its own input queue" ([Input Queues](https://techshelps.github.io/MSDN/DNWINNT/HTML/D1E/S856F.HTM)).

**What it means for Desk.** A single input queue that waits for each app to take its message is the design that makes one hung app freeze the shell. Desk's window system already has a queue per client per direction ([`APP-MODEL.md`](APP-MODEL.md#the-window-system-as-a-task)) and an input thread in the system band that no app can outrank (R142). The owner has decided the rule that makes those queues safe, the window system only posts to apps and never waits on them, and the timer-based detection that OS/2 and Windows both arrived at ([decisions 1 and 2](#the-owners-decisions)).

## Classic Mac OS

**What it was.** MultiFinder (August 1987) added cooperative multitasking to the Mac and became part of the system in System 7; the model was abandoned for the preemptive, Unix-based Mac OS X ([MultiFinder](https://en.wikipedia.org/wiki/MultiFinder)). Apple's documentation was explicit: the CPU belongs to the current application, which "can be interrupted only by hardware interrupts", and "you must periodically call the Event Manager's `WaitNextEvent` or `EventAvail` function" to give other applications time, which is why it is called a cooperative environment ([Inside Macintosh: Processes](https://developer.apple.com/library/archive/documentation/mac/Processes/Processes-13.html)). The move to Mac OS X needed a compatibility environment, Classic, which "runs a nearly complete Mac OS operating system", and a transitional API, Carbon, for programs not yet ported ([Classic Mac OS](https://en.wikipedia.org/wiki/Classic_Mac_OS)).

**What it means for Desk.** The hosted track is a cooperative system of exactly this kind, by necessity (DOS is not reentrant). The Mac shows that it works when every blocking call is a yield point and applications cooperate, and that the transition is expensive when the API itself assumed cooperation. Desk's hosted design already makes every SDK blocking call a switch point and asks long loops for `yield_now()` ([`APP-MODEL.md`](APP-MODEL.md#scheduling-preemptive-native-cooperative-hosted)), and because the SDK's semantics are preemptive, a hosted app needs no Classic or Carbon to move to the native track. The part that stays open is the one the Mac never solved without a new OS: a hosted app in a loop with no calls holds the machine, and only the untested watchdog idea addresses it.

## Windows NT

**What it was.** Windows NT 3.1 (July 1993) was written by a team led by Dave Cutler, designed for hardware and software portability, with per-object access control lists and environment subsystems (Win32, OS/2, POSIX) above the kernel ([Windows NT](https://en.wikipedia.org/wiki/Windows_NT)). It ran 16-bit Windows programs in a compatibility box: "Windows NT 4.0 runs 16-bit Windows applications as separate threads in a single NTVDM process with a shared address space", only one of those threads runs at a time, and one of them "can run at the same time as threads of other processes" ([Windows 16-bit Environment](https://techshelps.github.io/MSDN/NT40RES/S5903.HTM)). Windows 2000 could run a 16-bit application "in separate, unshared NTVDM process with its own memory space", "making the 16-bit application thread fully multitasking and preemptive" ([Running 16-bit Windows Applications in a Separate Process](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-2000-server/cc938548(v=technet.10))).

**What it means for Desk.** NT is the model for hosting a cooperative world inside a preemptive one: put the whole cooperative world in one ordinary process, so it is one schedulable unit with its own lock, and let a hang inside it stop only the programs inside it (an inference from the structure the sources describe, not a sourced claim about every NT version). Desk's R49 should take that shape: a FreeDOS V86 machine is one task (or one task per DOS box), scheduled like any other, its monitor holding no kernel lock while V86 code runs, so a DOS program that hangs stops only its own box. NT is also, as usually told, the system that put protection first and layered compatibility on top, where 9x put compatibility first and added protection where it could; that is the order Desk chose (APP-MODEL decision 10).

## QNX and Photon

**What it was.** QNX Neutrino uses "message passing as the fundamental means of IPC"; drivers "can be started and stopped as standard processes", and filesystems and the Photon GUI are processes too ([QNX Neutrino system architecture](https://www.qnx.com/developers/docs/6.5.0SP1/neutrino/sys_arch/intro.html)). A sending thread blocks until the server receives and replies; servers receive in priority order and "inherit the priority (but not the scheduling policy) of the sending thread", which "avoids priority-inversion problems" ([QNX: Interprocess Communication](https://www.qnx.com/developers/docs/6.5.0SP1/neutrino/sys_arch/ipc.html)). Photon is "a team of cooperating processes" around a small server that cannot "draw" anything or manage a pen, mouse or keyboard itself; "graphics drivers are implemented as processes" ([Photon microGUI](https://www.qnx.com/developers/docs/6.4.1/neutrino/sys_arch/photon.html)).

**What it means for Desk.** The window system as a task reached by messages (R143) is QNX's model applied to one service. QNX's priority inheritance through messages answers a question Desk's design leaves open: the window system runs in the fixed system band, which is right for input and the compositor, but a later file service serving a low-priority worker and a high-priority foreground app should serve them at their priorities. Desk's basic inheritance is on futex-style locks only (APP-MODEL decision 9); inheritance on message receive is a candidate for when the second service arrives. Drivers as restartable processes are further off; the HAL's drivers contain no `unsafe` and are written against traits ([`HAL.md`](HAL.md)), which keeps that move possible.

## X11 and Wayland

**What it was.** X (MIT, 1984) uses a client-server model in which the server "accepts requests for graphical output (windows) and sends back user input", and compositing window managers are external applications ([X Window System](https://en.wikipedia.org/wiki/X_Window_System)). Wayland puts window management and composition "entirely in the server", and clients "allocate their own off-screen buffers and render their window contents directly"; the Wayland documentation says that X's implementation of composition "adds additional context switches" ([Wayland: Introduction](https://wayland.freedesktop.org/docs/book/Introduction.html)).

**What it means for Desk.** Desk's window system is one task holding both the window manager and the compositor, which is Wayland's arrangement and avoids X's extra hop. The choice between retained drawing requests and client buffers is the X and Wayland split in miniature: requests need no shared memory, which suits stage 1 and the hosted track; client buffers with damage rectangles are the Wayland end state once stage 2 can share pages between two tasks. The owner has chosen the Wayland end state for large data ([decision 3](#the-owners-decisions)).

## RISC OS

**What it was.** RISC OS, first released by Acorn in 1987 as Arthur, "employs cooperative multitasking", handled by its window manager module; "a third party attempt to add preemptive multitasking was started in 1999", and a similar goal was added to the RISC OS Open roadmap in 2015 ([RISC OS](https://en.wikipedia.org/wiki/RISC_OS)).

**What it means for Desk.** A cooperative desktop can be pleasant and long-lived, and retrofitting preemption onto one is hard enough that, by the source's account, it is still an aim decades later. Desk's preemptive SDK semantics are the guard against ending up there with the hosted track's API.

## Plan 9

**What it was.** In Plan 9 "every resource in the system, either local or remote, is represented by a hierarchical file system", and "a user or process assembles a private view of the system by constructing a file name space"; `mount` and `bind` build it, and the window system 8½ serves files such as `/dev/cons` so that "each client sees a distinct copy of these files in its local name space" ([The Use of Name Spaces in Plan 9](https://9p.io/sys/doc/names.html)).

**What it means for Desk.** The path and mount model (R125) and the shared commands (R126) are still proposals. Plan 9 shows that a per-task namespace is what makes a window's console, a task's view of devices and a sandbox the same mechanism. Desk does not need that first; it needs the task's handle table (part of R77's task) and path resolution to be per task from the start, so a private view can be added later without changing the system calls.

## Cross-cutting lessons

**Preemption without protection.** Preemption fixes the runaway loop: the Amiga and Windows 95's 32-bit side keep running when one program spins. It does not fix the wild write, the program that disables multitasking, or a lock held by the faulty program; the Amiga's `Forbid()` and the Win16 mutex are both ways for one program to stop everyone despite a preemptive scheduler. Desk's stage 1 has the first benefit and none of the protections, which the stage table already says ([`APP-MODEL.md`](APP-MODEL.md#the-staged-path-on-the-native-track)); the lesson is to keep stage 1 short and to export nothing in it that stage 2 must take away.

**Hosting a cooperative world inside a preemptive one.** The two Microsoft answers differ in one respect. Windows 95 let preemptive threads call into the cooperative world through a global lock, so the cooperative world's hangs spread. NT put the cooperative world in one process in which only one 16-bit thread runs at a time, beside the threads of every other process, so (by inference) the hangs stayed inside it. For Desk: a FreeDOS V86 machine (R49) is a task like any other; its monitor emulates the sensitive instructions and reflects interrupts on that task's own time and never holds a kernel lock across V86 execution; and no native service waits on the DOS machine. On the hosted track, where the whole Desk is the cooperative world inside DOS, the same rule applies one level down: `rt`'s interrupt handlers set flags and fill rings and never switch threads ([`APP-MODEL.md`](APP-MODEL.md#scheduling-preemptive-native-cooperative-hosted)), so DOS's non-reentrancy never becomes a lock the handlers wait on.

**Input and hung applications.** Every system that let input wait on an application (Presentation Manager's single queue, Windows 3.x's, Windows 9x's 16-bit side) produced the frozen-desktop failure; the cures were a queue per application and a timer that notices one that stopped (OS/2's FixPack; Windows' five-second rule, after which the Desktop Window Manager shows a "ghost" with "Not Responding" in its title that can be moved, minimised and closed ([Preventing Hangs in Windows Applications](https://learn.microsoft.com/en-us/windows/win32/win7appqual/preventing-hangs-in-windows-applications))). Synchronous calls between windows spread a hang: one hung window in the clipboard viewer chain "has caused a chain of windows to stop responding" ([The case of the recursively hung WM_DRAWCLIPBOARD](https://devblogs.microsoft.com/oldnewthing/20221223-00/?p=107612)). For Desk the rules, now decided ([decisions 1 and 2](#the-owners-decisions)), are: the window system only posts to a client and never waits on it; a client that has not handled an event for 5 seconds is marked hung, its window shows it, and Close terminates its task (R77's kill path). On the hosted track a hung app holds the CPU and nothing can be drawn, so this applies only to an app stuck in a blocking wait, not to a spinning one.

**Library sharing and versioning.** Windows' DLLs failed through shared system directories and absent versions, and were fixed by protecting system files and giving each application its own copy. Desk starts at the fixed end: static linking, a version in every header, a clear refusal at load (R145, [`LOADER.md`](LOADER.md)). Sharing, if it is built, should be by name and version per task, never by overwriting one system file.

**Compatibility layers and their long-term cost.** Windows 9x carried DOS as boot loader and driver layer, a shared 16-bit heap, a global lock and per-program special cases, and removing any of them (ME's real-mode DOS) broke someone. Classic Mac needed a whole hosted OS (Classic) and a transitional API (Carbon) to move. Desk's compatibility layers are: the hosted track itself, the BIOS trampoline and DPMI calls, and FreeDOS for DOS programs. Each sits behind a boundary (the HAL, the `Bios` trait, a V86 task) that apps never see, which is what makes each removable.

**Which Desk decisions already avoid these, and which do not yet.** Already avoided by decision: one scheduling semantics for all apps (no "16-bit" class of code), no global lock shared between apps and the input or compositor path, a queue per client, the window system as a separate task, protection before the system-call boundary, static linking with versioned refusal, and the HAL keeping BIOS and DOS below the SDK. Decided since by the owner ([below](#the-owners-decisions)): hung-client detection and what the user sees, a window system that only posts to clients, copied small messages with shared large buffers, `cli` in stage 1, one task per DOS box, no per-program special cases and per-task path resolution. Still not addressed: priority inheritance through messages (decided with the file service), restartable drivers and services, and a hosted watchdog for a spinning app.

## The lessons in one table

| Lesson | Source system | Desk decision or roadmap item | Status |
|---|---|---|---|
| One scheduling semantics for every app; no class of code that assumes it cannot be interrupted | Windows 95 (Win16 code under preemptive Win32), RISC OS | APP-MODEL decision 2 (preemptive semantics on both tracks), R79 | applied |
| Never share a global lock between the cooperative world and the rest of the system | Windows 95 Win16 mutex | APP-MODEL priorities (input and compositor share no lock with apps); decision 5 (one task per DOS box, R49) | applied |
| Host the cooperative world as one schedulable unit with its own lock | Windows NT (NTVDM) | Decision 5, R49 (FreeDOS in a V86 task) | applied |
| A queue per client; input never waits on an application | OS/2 PM, Windows 3.x and 9x | R143 (one queue per direction per client); decision 2 | applied |
| Detect a hung client by timer and let the user close it | OS/2 FixPack, Windows ghost windows | Decision 1 (R143, R77 kill path) | applied |
| Post, never send-and-wait, from the server to a client | Windows clipboard viewer chain, BeOS locking | Decision 2 (R143) | applied |
| Window manager and compositor in one server task | Wayland (against X's separate compositor) | R143 | applied |
| Large data in client buffers shared with the compositor, as in Wayland | X11 and Wayland | Decision 3 (R143, R78) | applied |
| Preemption does not give protection; keep the unprotected stage short and export nothing it must later remove | AmigaOS (`Forbid()`, Guru Meditation), Windows 9x shared arena | APP-MODEL stages 1 and 2, R78; decision 4 (`cli`) | applied |
| Small messages copied; shared buffers only with explicit ownership and lifetime, so protection changes the transport only | AmigaOS Exec messages, Wayland | Decision 3 (R143) | applied |
| No writable region mapped into every task; the low megabyte supervisor-only | Windows 9x shared arena and unprotected first megabyte | R78, [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#paging) | partly |
| Thunks narrow, below the SDK, never an app interface | Windows flat and generic thunks | HAL `Bios` trait, `rt`'s DPMI 0300h, R79 | applied |
| BIOS and DOS as temporary fallbacks, replaced by native drivers | Windows 95's DOS driver layer, ME's removal | R74, R75; N3's V86 upgrade | partly |
| Drivers and services as processes that can be restarted | QNX | HAL drivers without `unsafe`; services in APP-MODEL | not yet |
| Priority inheritance through messages for services | QNX | Decision 6 (with the file service); APP-MODEL decision 9 covers locks only | not yet |
| No mandatory thread per window; one lock-order rule | BeOS | APP-MODEL decision 7 (one main loop, workers by choice) | partly |
| Every blocking call a yield point on the cooperative track | Classic Mac OS (`WaitNextEvent`) | APP-MODEL hosted scheduling | applied |
| A cooperative runaway loop needs a watchdog | Classic Mac OS, Windows 3.x | APP-MODEL hosted watchdog idea | not yet |
| Static linking by default; versions refused at load, never overwritten | Windows DLL hell | R145, [`LOADER.md`](LOADER.md) | applied |
| No per-program special cases in the system | Windows 95 (SimCity) | Decision 7 | applied |
| Per-task namespace possible later; path resolution per task now | Plan 9 | Decision 8 (R125, R126, R77 task handles) | applied |
| Protection designed in from the start, compatibility on top | Windows NT against 9x (as usually told) | APP-MODEL decision 10 (paging and ring 3 before system calls) | applied |

## The owner's decisions

The owner decided the eight questions this note raised, each with its reason in a line. The design detail is applied to [`APP-MODEL.md`](APP-MODEL.md#decisions) and is not restated here.

1. **Hung apps.** An app that has not handled an event for 5 seconds is marked hung, its window shows that it is, and Close terminates its task. The frozen-desktop failure of a single input queue is the one the owner has seen, and Windows and OS/2 both arrived at a timer.
2. **The window system only posts messages to apps and never waits on them.** OS/2's lesson: a server that waits on a client is frozen by it, and the hang spreads to everything that waits on the server.
3. **Small messages are copied; large data (window pixel buffers and similar) lives in shared memory, as in Wayland.** Chosen over copy-only, so a window's pixels are not copied through a queue on every frame (a cost not measured on a 486), in the shape Wayland uses. What it asks of the design, as requirements: who owns a shared buffer and when ownership passes between the app and the compositor; how long a buffer lives and who frees it; how a task is stopped or killed while a buffer is shared, so the compositor never reads freed memory and the task's memory ledger stays right; and what the compositor may assume about a buffer's contents while the app may still be writing it (it must tolerate a torn frame or the protocol must say when the app may not write).
4. **`cli` is allowed in stage 1 and never offered by the SDK.** Ring 0 cannot prevent it; a lint or review rule bans the instruction in apps, and ring 3 in stage 2 makes it impossible, so nothing comes to depend on it.
5. **One task per DOS box (R49),** scheduled like an app, its monitor holding no kernel lock across V86 execution. NT's NTVDM shows that a cooperative world kept in one schedulable unit keeps its hangs to itself.
6. **Priority inheritance through messages is decided together with the file service.** The window system stays in its fixed band, and the question only matters once a service serves clients of different priorities.
7. **No per-program special cases.** The system never recognises a program by name to change its behaviour; compatibility is the SDK version and the service protocols' versions, so no special case has to be carried forever.
8. **Path resolution per task (R125).** One system namespace first, with path resolution and the handle table per task, so a Plan 9-style private view can be added later without new system calls.

## Sources

Fetched on 2026-11-11.

**Windows 3.x and 9x**

- Microsoft Knowledge Base 125867, "Understanding Win16Mutex": <https://ftp.zx.net.nz/pub/archive/ftp.microsoft.com/MISC/KB/en-us/125/867.HTM>
- Microsoft Knowledge Base Q117567, "How 16-Bit and 32-Bit Programs Multitask in Windows 95": <https://www.infania.net/misc/kbarchive/kb/117/Q117567/index.html>
- Microsoft Knowledge Base 125691, "Overview of the Windows 95 Virtual Address Space Layout": <https://ftp.zx.net.nz/pub/Patches/ftp.microsoft.com/MISC/KB/en-us/125/691.HTM>
- Microsoft Knowledge Base 125710, "Types of Thunking Available in Win32 Platforms": <https://ftp.zx.net.nz/pub/archive/ftp.microsoft.com/MISC/KB/en-us/125/710.HTM> and <https://techshelps.github.io/MSDN/KB/Source/win32sdk/q125710.htm>
- Windows 95 DDK, "Virtual Machine Manager": <https://techshelps.github.io/MSDN/WIN95DDK/HTML/S22E9.HTM>
- Win32 SDK, "MS-DOS-Based Applications": <https://techshelps.github.io/MSDN/WINBASE/devdoc/live/pdwbase/win95/chiarc_7p4j.htm>
- Windows 95 technical notes, "User": <https://techshelps.github.io/MSDN/DNWIN95/HTML/S72C4.HTM>
- Raymond Chen, "What was the role of MS-DOS in Windows 95?" (2007): <https://devblogs.microsoft.com/oldnewthing/20071224-00/?p=24063>
- Raymond Chen, "Windows 95's ticking death" (2009): <https://devblogs.microsoft.com/oldnewthing/20090918-00/?p=16663>
- Raymond Chen, "Windows Confidential: Windows 95 Unplugged", TechNet Magazine (2008): <https://learn.microsoft.com/en-us/previous-versions/technet-magazine/cc742531(v=msdn.10)>
- Raymond Chen, "The case of the recursively hung WM_DRAWCLIPBOARD" (2022): <https://devblogs.microsoft.com/oldnewthing/20221223-00/?p=107612>
- Joel Spolsky, "How Microsoft Lost the API War" (2004): <https://www.joelonsoftware.com/2004/06/13/how-microsoft-lost-the-api-war/>
- Wikipedia, "Windows 3.0": <https://en.wikipedia.org/wiki/Windows_3.0>; "Windows 9x": <https://en.wikipedia.org/wiki/Windows_9x>; "Windows Me": <https://en.wikipedia.org/wiki/Windows_Me>; "VxD": <https://en.wikipedia.org/wiki/VxD>; "DLL Hell": <https://en.wikipedia.org/wiki/DLL_Hell>

**Windows NT**

- Windows NT 4.0 Resource Kit, "Windows 16-bit Environment": <https://techshelps.github.io/MSDN/NT40RES/S5903.HTM>
- Windows 2000 Server documentation, "Running 16-bit Windows Applications in a Separate Process": <https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-2000-server/cc938548(v=technet.10)>
- Windows NT documentation, "Input Queues": <https://techshelps.github.io/MSDN/DNWINNT/HTML/D1E/S856F.HTM>
- Microsoft, "Preventing Hangs in Windows Applications": <https://learn.microsoft.com/en-us/windows/win32/win7appqual/preventing-hangs-in-windows-applications>
- Wikipedia, "Windows NT": <https://en.wikipedia.org/wiki/Windows_NT>

**Other systems**

- Wikipedia, "Exec (Amiga)": <https://en.wikipedia.org/wiki/Exec_(Amiga)>; "Guru Meditation": <https://en.wikipedia.org/wiki/Guru_Meditation>
- AmigaOS documentation wiki, "Exec Messages and Ports": <https://wiki.amigaos.net/wiki/Exec_Messages_and_Ports>
- Peter Potrebic, "Be Engineering Insights: Summer Vacations and Semaphores", Be Newsletter 1-4 (1996): <https://haiku-os.org/legacy-docs/benewsletter/Issue1-4.html>
- The Be Book, "The Interface Kit": <https://www.haiku-os.org/legacy-docs/BeBook/TheInterfaceKit_Overview_Introduction.html>
- Haiku, "app_server: graphics": <https://haiku-os.org/docs/develop/servers/app_server/graphics.html>
- Wikipedia, "Presentation Manager": <https://en.wikipedia.org/wiki/Presentation_Manager>
- Apple, "Inside Macintosh: Processes", the cooperative multitasking environment: <https://developer.apple.com/library/archive/documentation/mac/Processes/Processes-13.html>
- Wikipedia, "MultiFinder": <https://en.wikipedia.org/wiki/MultiFinder>; "Classic Mac OS": <https://en.wikipedia.org/wiki/Classic_Mac_OS>
- QNX Neutrino 6.5 system architecture, introduction: <https://www.qnx.com/developers/docs/6.5.0SP1/neutrino/sys_arch/intro.html>; interprocess communication: <https://www.qnx.com/developers/docs/6.5.0SP1/neutrino/sys_arch/ipc.html>
- QNX Neutrino 6.4.1 system architecture, "Photon microGUI": <https://www.qnx.com/developers/docs/6.4.1/neutrino/sys_arch/photon.html>
- Wikipedia, "X Window System": <https://en.wikipedia.org/wiki/X_Window_System>
- Wayland documentation, "Introduction": <https://wayland.freedesktop.org/docs/book/Introduction.html>
- Wikipedia, "RISC OS": <https://en.wikipedia.org/wiki/RISC_OS>
- Rob Pike, Dave Presotto, Ken Thompson, Howard Trickey and Phil Winterbottom, "The Use of Name Spaces in Plan 9" (1993): <https://9p.io/sys/doc/names.html>
