---
id: decisions
n:
kicker: Decisions and risks
title: Decisions and risks
date: 2026-10-22
summary: What has been decided, what is waiting for the owner, and what could go wrong, as the repository's own documents state them.
prs: [5]
section: reference
---
An archive that only lists successes is a brochure. Here are the decisions on record, the questions still open, and the risks that `docs/DIRECTION.md` names for itself. I have not added any of my own: every row is lifted from `docs/OPEN-QUESTIONS.md`, `docs/DIRECTION.md` or the roadmap, and says where.

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
| Browser build | Ship DOSBox-only until the linear frame buffer works in js-dos | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) |

## Waiting for a decision

| Question | Why it matters | Where |
|---|---|---|
| Retire the C layer and `coff2go32.py`? | One toolchain; spike d0 recommends it | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| Graphics strategy on the native track: VBE before the switch, V86 calls or native drivers | Decides how a frame reaches the screen | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Scheduling: cooperative or preemptive | Shapes the app ABI and the core's event loop | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Memory protection and app isolation | Decides what a crashed app can damage | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Filesystem, boot media, early disk access | What the kernel can read and how it is started | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| The shape of the HAL | Without it the two tracks drift apart | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| Licensing of FreeDOS components | Must be checked before a release carries them | [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) |
| The product name | Names the image, the disk label and the repository; blocks nothing | [`docs/NAMING.md`](repo:docs/NAMING.md) |
| The 4 bitmap faces | They have not been seen on a DOS screen, and the 6 by 8 face is the one to check first | [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) |

## Risks

| Risk | In the document's words, in short |
|---|---|
| Hardware diversity | Video cards, sound cards and chipsets differ in ways the BIOS used to hide; a native OS carries that cost itself |
| No real hardware in the loop | Emulators will pass things real machines reject, and the reverse |
| Emulator accuracy | QEMU, 86Box and Bochs disagree with each other and with real parts; a result in one is not a claim about hardware |
| Scope | An operating system is much more than a shell; two tracks split the effort |
| Divergence | If the HAL is wrong, the two tracks drift apart |

Source for the risks: [`docs/DIRECTION.md`](repo:docs/DIRECTION.md), section "Risks".

## The shelves

### Catalogued
- The "Decided" rows are recorded in the cited documents, each merged into `main` with CI green.

### Uncatalogued
- The "Decided" rows are decisions, not measurements: none has been tested beyond the spikes in the findings table, and `docs/DIRECTION.md` says its numbers are targets.
- The risks have not been assessed: I have copied them, and nothing here measures how likely they are.

### Wish list
- An answer to every row under "Waiting for a decision", in the owner's time.
