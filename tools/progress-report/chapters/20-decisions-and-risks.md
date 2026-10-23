---
id: decisions
n:
kicker: Decisions and risks
title: Decisions and risks
date: 2026-10-23
summary: What has been decided, what is waiting for the owner, what could go wrong, and, since merge day, the order in which eight open pull requests could be merged and what the work taught about how it is being done.
prs: [5, 8, 10, 11, 12, 13, 14, 15, 16, 17]
section: reference
---
An archive that only lists successes is a brochure. Here are the decisions on record, the questions still open, and the risks that `docs/DIRECTION.md` names for itself. The first three tables are lifted from `docs/OPEN-QUESTIONS.md`, `docs/DIRECTION.md` and the roadmap, and say where. The sections after them are mine: they are about the open pull requests and the way the work is being done, and each says what it rests on.

## Decided

| Decision | What was chosen | Where |
|---|---|---|
| What the Desk is | An operating system in Rust for old hardware; the shell is its product layer | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) |
| Hosted or native | Both, in parallel, sharing a core behind a HAL | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| DOS programs on the native OS | Bundle FreeDOS, designed later | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Rust apps as modules, hosted | DXE3 modules importing `desk_*` by name (route a), with a function-pointer table as the fallback (route b) | [`docs/S1-DXE3.md`](repo:docs/S1-DXE3.md) |
| Licence | Apache-2.0 OR MIT | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) |
| Merging | Merge commits only, never squash, never rebase-merge | [`AGENTS.md`](repo:AGENTS.md) |
| The CI image | `ci-dos`, pinned by digest | [`AGENTS.md`](repo:AGENTS.md) |
| Browser build | Ship DOSBox-only until the linear frame buffer works in js-dos. The condition is now met by spike s5, in a pull request that is open, not merged (chapter 14) | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md), [`docs/S5-JSDOS.md`](repo:docs/S5-JSDOS.md) on its branch |

## Waiting for a decision

| Question | Why it matters | Where |
|---|---|---|
| Retire the C layer and `coff2go32.py`? | One toolchain; spike d0 recommends it | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| Graphics strategy on the native track: VBE before the switch, V86 calls or native drivers | Decides how a frame reaches the screen. Spike D shows VBE from the loader works in QEMU and that leaving it without the BIOS fails on one card (chapter 12); the real BIOS path is spike n3 | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), [`docs/D-NATIVE-VGA.md`](repo:docs/D-NATIVE-VGA.md) on its branch |
| Scheduling: cooperative or preemptive | Shapes the app ABI and the core's event loop | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Memory protection and app isolation | Decides what a crashed app can damage | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Filesystem, boot media, early disk access | What the kernel can read and how it is started | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| The shape of the HAL | Without it the two tracks drift apart. A design is proposed in pull request #8 and is waiting for review (chapter 10) | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), [`docs/HAL.md`](repo:docs/HAL.md) on its branch |
| Licensing of FreeDOS components | Must be checked before a release carries them | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| The product name | Names the image, the disk label and the repository; blocks nothing | [`docs/NAMING.md`](repo:docs/NAMING.md) |
| The 4 bitmap faces | They have not been seen on a DOS screen, and the 6 by 8 face is the one to check first | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) |

## The open pull requests: order and stacking

Since merge day only {{merged.since}} has merged, which is the generator of this report; {{m.prsOpen}} other pull requests are open ({{open.numbers}}), with CI green on the head of {{m.openCi}} of them, and {{m.prsMergeDay}} merged on merge day. The generator reads how they sit on each other, drawn here, and the order below is my reading of that, not a decision.

<!-- figure pr-stack -->

And the pairs that would conflict if both were merged, found by trial merges of every pair of open branches where neither contains the other:

<!-- figure open-conflicts -->


- **#8, the HAL, goes first.** It is based on `main`, and #10 and #11 are based on its branch. Every other hardware pull request contains it.
- **#10 and #11 come next, in either order.** The keyboard and mouse spike (#10) and the VGA and VBE spike (#11) are each based on #8 and do not depend on each other.
- **#14 and #15 come after their parents.** #14 (native graphics) is based on #10 and contains #11 through a merge commit; #15 (DOS input) is based on #11 and contains #10 the same way. Merging #10 and #11 first leaves each of them with only its own work. The pull request pages show the lines of their base branches, which is why their numbers are large.
- **#17 is based on #14.** Spike n3, the BIOS path, is stacked on the native graphics spike, so it waits for #14 and its parents.
- **#12, #13 and #16 are independent of the stack.** The `std` experiment (#12), js-dos (#13) and spike G (#16) are based on `main`. #12 does change the runtime that #11, #14 and #15 also edit (`spikes/d0-rust-dos/rt`), so it should expect to be reconciled with them.
- **Spike G's loader fix and the loader changes of #14 and #17 will conflict.** #16 adds the INT 15h AH=88h fallback to `boot/stage2.asm`, and #14 (and so #17) add the VBE step and the boot information block to the same file. A trial merge reports a conflict in `boot/stage2.asm` for {{conflicts.boot}}, and in the native-boot document and the shared readmes. Whichever is merged second has to carry the other's lines, and the 86Box result of chapter 16 and the BIOS path of chapter 17 are then needed together.
- **The shared files will conflict again.** Of the nine pull requests open when this was written, `tools/check.sh` and `tools/README.md` are edited by all nine, `docs/README.md` and `spikes/README.md` by eight, and `AGENTS.md` by six. Merge day had the same pattern, and its remedy was to keep every step: merge `main` into the branch, resolve, then merge the pull request with a merge commit (chapter 9).
- **This report's chapters and gallery** are in a pull request of their own, based on `main` after #9 (the generator) merged. It touches only `tools/progress-report` and can go in at any point, but the report describes the others as open, so it will read differently once they merge.

## What the work taught about how it is done

- **The code-review tool reviewed the wrong target, more than once.** The comments on #8, #9 and #12 each record that a first review run looked at the primary checkout (the already-merged d0 code on `main`) instead of the branch, reported findings that were not part of the pull request, and edited files in the main checkout, which were reverted. The repair that worked was to run the review from the branch's own worktree with an explicit pull request number and to check afterwards that the main checkout was untouched. It is a process finding, not a defect of any one pull request.
- **The review still paid for itself.** Each of those comments lists real defects found and fixed in separate commits, such as the timer's wrong mode in #8, the clock that froze at midnight in #12 and the Backspace key that did nothing in the first window of #14, and a few findings declined with reasons. The archive counts the fixes by reading the comments, not by trusting the summary.
- **Spikes stack, and the stacks are the risk.** A stacked pull request shows its parents' lines, so the sizes in the evidence tables carry a footnote, and a base branch that is merged first changes what a later pull request contains.
- **Disk and tooling.** When this chapter was written the development machine's `/home` was 99% full with 13 GB free (`df -h /home`), and `/tmp` is memory-backed, so the generator's scratch builds are kept small and deleted. Cargo target directories, a DOS toolchain and Docker images each cost real space, and the repository's own build scripts say so. This is a note about the machine, not about the repository.

## Risks

| Risk | In the document's words, in short |
|---|---|
| Hardware diversity | Video cards, sound cards and chipsets differ in ways the BIOS used to hide; a native OS carries that cost itself |
| No real hardware in the loop | Emulators will pass things real machines reject, and the reverse |
| Emulator accuracy | QEMU, 86Box and Bochs disagree with each other and with real parts; a result in one is not a claim about hardware |
| Scope | An operating system is much more than a shell; two tracks split the effort |
| Divergence | If the HAL is wrong, the two tracks drift apart. A first design now exists and has two consumers in open pull requests, but none is merged |

Source for the risks: [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), section "Risks".

## The shelves

### Catalogued
- The "Decided" rows are recorded in the cited documents on `main`, each merged with CI green, except the browser row, whose spike is in an open pull request.
- The stacking in the open-pull-request section is read from `git` and `gh`: base branches, which open branches contain which, and the files each touches.
- The review-tool finding is quoted from the comments on pull requests #8, #9 and #12.

### Uncatalogued
- The "Decided" rows are decisions, not measurements: none has been tested beyond the spikes in the findings table, and `docs/DIRECTION.md` says its numbers are targets.
- The risks have not been assessed: I have copied them, and nothing here measures how likely they are.
- The merge order is my reading of the stacking. The conflicts below were found with trial merges (`git merge-tree`, which writes no working tree) of every independent pair, and no real merge was made.
- The disk figure is a reading of one machine on one day.

### Wish list
- An answer to every row under "Waiting for a decision", in the owner's time.
- A merge, in the order above, of the pull requests that earn it.
- A way for the review tool to take its target as an argument it cannot ignore.

```figures
id: pr-stack
kind: diagram
track: shared
title: The stack of open pull requests
alt: A graph of the open pull requests, each box pointing to the branch it is based on, with dashed lines to other open branches it contains.
caption: What is based on what, from gh and git.
at: figure pr-stack
thumb: svg
state: none
```
