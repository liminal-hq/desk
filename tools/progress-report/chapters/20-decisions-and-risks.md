---
id: decisions
n:
kicker: Decisions and risks
title: Decisions and risks
date: 2026-10-23
summary: What has been decided, what is waiting for the owner, what could go wrong, and, since merge day, the order in which the spike pull requests were merged and what the work taught about how it is being done.
prs: [5, 8, 10, 11, 12, 13, 14, 15, 16, 17]
section: reference
---
An archive that only lists successes is a brochure. Here are the decisions on record, the questions still open, and the risks that `docs/DIRECTION.md` names for itself. The first three tables are lifted from `docs/OPEN-QUESTIONS.md`, `docs/DIRECTION.md` and the roadmap, and say where. The sections after them are mine: they are about how the spike pull requests were merged and the way the work is being done, and each says what it rests on.

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
| Browser build | Ship DOSBox-only until the linear frame buffer works in js-dos. The condition is now met by spike s5, in pull request #13, which is {{pr13.state}} (chapter 14) | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md), [`docs/S5-JSDOS.md`](repo:docs/S5-JSDOS.md) on its branch |

## Waiting for a decision

| Question | Why it matters | Where |
|---|---|---|
| Retire the C layer and `coff2go32.py`? | One toolchain; spike d0 recommends it | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| Graphics strategy on the native track: VBE before the switch, V86 calls or native drivers | Decides how a frame reaches the screen. Spike D shows VBE from the loader works in QEMU and that leaving it without the BIOS fails on one card (chapter 12); the real BIOS path is spike n3 | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), [`docs/D-NATIVE-VGA.md`](repo:docs/D-NATIVE-VGA.md) on its branch |
| Scheduling: cooperative or preemptive | Shapes the app ABI and the core's event loop | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Memory protection and app isolation | Decides what a crashed app can damage | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Filesystem, boot media, early disk access | What the kernel can read and how it is started | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| The shape of the HAL | Without it the two tracks drift apart. A first design came in pull request #8, which is {{pr8.state}}, and it has consumers in #10, #11, #14, #15 and #17, and it is still a design that only emulators and host tests have met (chapter 10) | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), [`docs/HAL.md`](repo:docs/HAL.md) on its branch |
| Licensing of FreeDOS components | Must be checked before a release carries them | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| The product name | Names the image, the disk label and the repository; blocks nothing | [`docs/NAMING.md`](repo:docs/NAMING.md) |
| The 4 bitmap faces | They have not been seen on a DOS screen, and the 6 by 8 face is the one to check first | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) |

## The spike pull requests: how they were merged

Nine spike pull requests were open at once, and all nine are merged now (#8, #10, #11, #12, #13, #14, #15, #16 and #17), each with a merge commit and, on the merge commit, CI passing. `git log --first-parent main` gives the order they landed in: #8, #10, #11, #14, #15, #17, #16, #12 and #13, in about an hour, then #18, the first edition of this report, after them. What follows is the shape they had while they waited, kept as history. The graph that drew it, and the trial-merge table of pairs that would conflict, needed open branches to draw, so they are retired with this edition.

- **#8, the HAL, went first.** It was based on `main`, and #10 and #11 were based on its branch.
- **#10 and #11 came next.** The keyboard and mouse spike (#10) and the VGA and VBE spike (#11) were each based on #8 and did not depend on each other.
- **#14 and #15 came after their parents.** #14 (native graphics) was based on #10 and contained #11 through a merge commit; #15 (DOS input) was based on #11 and contained #10 the same way, so their pull request pages showed the lines of their base branches.
- **#17 was based on #14.** Spike n3, the BIOS path, was stacked on the native graphics spike, and it landed after #14 and #15 and before #16.
- **#12, #13 and #16 were independent of the stack.** The `std` experiment (#12), js-dos (#13) and spike G (#16) were based on `main`; they merged last.
- **The shared files conflicted again, as on merge day.** The merge commits that the authors made on the branches before merging ("Merge main into ...", `git log --merges main`) list the conflicts: the index rows in `docs/README.md` and `spikes/README.md`, the `check.sh` row of `tools/README.md` and the member list of `hal/Cargo.toml`, each resolved by keeping every side's lines; and in `boot/stage2.asm` the AH=88h fallback of #16 was combined with the VBE step and boot information block that #14 had added. It is the merge-day remedy over again: keep every step (chapter 9).
- **This report's chapters and gallery** (#18) merged after the others. It touched only `tools/progress-report`, and its own merge into `main` before landing reported no conflicts.

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
| Divergence | If the HAL is wrong, the two tracks drift apart. A first design now exists and is merged with its consumers (the input, graphics and BIOS spikes), but every consumer ran only in an emulator, so whether the design is right is still open |

Source for the risks: [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), section "Risks".

## The shelves

### Catalogued
- The "Decided" rows are recorded in the cited documents on `main`, each merged with CI green; the browser row's spike, #13, is merged too.
- The merge order in the spike section is read from `git log --first-parent main`, and the conflicts from the bodies of the "Merge main into" commits.
- The review-tool finding is quoted from the comments on pull requests #8, #9 and #12.

### Uncatalogued
- The "Decided" rows are decisions, not measurements: none has been tested beyond the spikes in the findings table, and `docs/DIRECTION.md` says its numbers are targets.
- The risks have not been assessed: I have copied them, and nothing here measures how likely they are.
- The stacking described in the spike section is from the first edition's reading of the base branches and what each contained; the branches are gone from the open list and I did not re-derive it.
- The disk figure is a reading of one machine on one day.

### Wish list
- An answer to every row under "Waiting for a decision", in the owner's time.
- A way for the review tool to take its target as an argument it cannot ignore.
