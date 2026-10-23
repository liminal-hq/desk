# AGENTS.md

## Table of Contents

- [Project Status](#project-status)
- [Source of Truth](#source-of-truth)
- [Localization and Spelling](#localization-and-spelling)
- [Markdown Formatting](#markdown-formatting)
- [Authoring Voice](#authoring-voice)
- [Commit Messages](#commit-messages)
- [Pull Request Titles](#pull-request-titles)
- [Pull Request Content](#pull-request-content)
- [Pull Request Labels](#pull-request-labels)
- [Git Workflow](#git-workflow)
- [Local Tooling](#local-tooling)
- [Code Conventions](#code-conventions)
- [Testing](#testing)
- [Documentation](#documentation)
- [Repository Layout](#repository-layout)
- [Licence and Copyright](#licence-and-copyright)
- [Agent Behaviour](#agent-behaviour)

## Project Status

Desk (`liminal-hq/desk`, a working name; see `docs/NAMING.md`) is an operating system, written in Rust, for old hardware: a calm graphical system for 486-class PCs, with windows, files and small apps. It is built on two tracks in parallel (decided 2026-10-22; see `docs/DIRECTION.md`): a **hosted track**, Desk running on top of DOS (DJGPP, CWSDPMI, DOSBox), which is what the repository has built so far, and a **native track**, a bootable Rust kernel tested in QEMU, 86Box and Bochs. Both share an OS-independent core behind a small hardware abstraction layer (HAL). DOS programs on the native OS are planned to run by bundling FreeDOS. It is the spiritual successor to Lieutenant Fizz for DOS (`ScottMorris/lieutenant-fizz-dos`) and applies what that port taught about Rust on DOS. The project is at the design-and-spikes stage: the code is the hosted walking skeleton (a small Rust and C program that proves the DOS toolchain), the native boot skeleton (a NASM boot sector and a Rust kernel that boots in QEMU; `docs/NATIVE-BOOT.md`), and the fonts and emoji work. The documents in `docs/` are targets, not measurements.

## Source of Truth

- `docs/DIRECTION.md` is the direction: the two tracks, the shared core, the native milestones and the open questions of the native track. `docs/SPEC.md` is the detailed spec of the hosted track and the OS-independent parts until a native spec exists; `docs/SDK.md`, `docs/FORMATS.md`, `docs/HANDOFF.md`, `docs/PALETTE.md` and `docs/HIG.md` hold the detail, and `docs/OPEN-QUESTIONS.md` says what is decided and what is open.
- The documents came from a design brainstorm written without access to code, so they may be wrong. Once code exists, the code is the source of truth: if code and docs disagree, resolve it deliberately by changing the code to match, or by updating the document in the same branch with the reasoning.
- A spike ends with a written result in `docs/` (what worked, what did not, what it changes) so the design follows what was learned.
- `design/` is reference only: boards and prototypes that are not built, tested or licence-headered. Do not port or mirror its code.
- Lieutenant Fizz for DOS is a reference for lessons, not a source to copy. Tooling here is written for this project.

## Localization and Spelling

**REQUIREMENT:** All UI strings, code identifiers, comments, commit messages, pull request descriptions and documentation MUST use **Canadian English** spelling, unless exact external spelling is required by tooling, APIs, platform interfaces or published identifiers (for example the CSS `color` property or a DOS `COLOR` constant).

Examples:

- `colour` instead of `color`
- `centre` instead of `center`
- `neighbour` instead of `neighbor`
- `behaviour` instead of `behavior`
- `cancelled` instead of `canceled`
- `licence` (noun) vs `license` (verb)

## Markdown Formatting

**REQUIREMENT:** Do not hard-wrap markdown prose. Write each paragraph or bullet as a single unwrapped line in the source, no matter how long, and let the renderer reflow it. This applies to PR descriptions, docs under `docs/`, README files and any other markdown. Commit message bodies are the one exception: hard-wrap those (see [Commit Messages](#commit-messages)).

- This does not apply to genuinely separate list items, headings, or intentional line breaks.
- Code comments are not markdown-rendered and may wrap normally at a reasonable line length.

**Em dashes:** use a real em dash (`—`) in prose, never `--` as a substitute. This does not apply to a double-hyphen with another meaning (a CLI flag such as `--check`, a numeric range).

`tools/check-docs.py` checks both rules and that relative links resolve.

## Authoring Voice

**REQUIREMENT:** Ship the result, not how the conversation arrived at it. Write every outward-facing line (code comments, identifier names, docs, PR descriptions) as the author of the artefact, for the reader who will encounter it later.

- Don't reference "this PR", "the review", a reviewer's name, or a commit SHA inside code comments or PR prose. State the fact or reasoning directly.
- When a comment is edited more than once, rewrite it as one clean explanation rather than stacking fragments.
- Commit messages are the exception: they are the right place to record _why_ a change happened, including debugging context and measurements. The `## Test plan` section of a PR description is a similar exception.

## Commit Messages

**Format:** Use Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `ci:`, `build:`, `chore:`, `refactor:`, `perf:`). The one exception is the repository's first commit, titled `Initial Commit`, which records the plumbing and organisation the project started with.

- Use `test:` for test-related changes, including fixes to tests themselves. Use `fix:` only when it fixes application code.
- Use `ci:` or `build:` when the primary change is workflow or build behaviour.
- Keep each commit focused on the specific change made in that commit; describe the now, not the whole project history.

**Body requirements:**

- Explain what and why (not how).
- Use markdown: **bold**, _italics_, `code`, flat bullet lists. No markdown headings; use **bold labels** if sections are needed.
- **Backtick every code-level reference**: function, type and variable names, file and directory paths, crate names, config keys and CLI flags (for example `core/src/video.rs`, `DESK.INI`, `--check`). Plain-English descriptions and in-app text use quotes, not backticks.
- Hard-wrap paragraphs at roughly 72-100 characters.
- End with the agent co-author trailer when an agent authored the commit.

**Shell interpolation safety:**

- Do not pass markdown-heavy bodies through `git commit -m "..."` when they contain backticks, `$()` or other shell-sensitive characters.
- Write the message to a file with a single-quoted heredoc and commit with `git commit -F <file>`.
- Verify with `git log -1 --pretty=fuller` and amend immediately if interpolation altered the text.

## Pull Request Titles

**REQUIREMENT:** PR titles are human-readable summaries of the change.

- Start with a capital letter, use the imperative mood, and keep to roughly 70 characters.
- Do not use Conventional Commit prefixes in PR titles.
- Describe the outcome or behaviour change, not internal process.
- Do not rename merged PRs unless explicitly requested.

## Pull Request Content

- Open pull requests ready for review by default. Use a draft only when asked or when there is a clearly communicated blocker.
- Default structure: `## Summary` (flat bullets with bold lead-ins) followed by `## Test plan` (checklist bullets with concrete commands, and an explicit statement of anything that could not be verified).
- Keep the summary focused on outcomes and behaviour changes, not commit chronology.

## Pull Request Labels

Add labels to every PR when it is created and keep them accurate as scope changes.

- **Primary category (at least one):** `enhancement`, `bug`, `documentation`, `testing`, `ci`, `build`, `chore`.
- **Scope:** `dos`, `shell`, `compositor`, `graphics`, `input`, `sound`, `sdk`, `apps`, `handoff`, `launcher`, `installer`, `rust`, `performance`, `design-docs`, `design-system`, `spike`, `experimental`, `ui`, `pages`, `deploy`.
- Prefer GitHub category labels (`enhancement`, `bug`) over Conventional Commit terms (`feat`, `fix`).
- `skip-changelog` and `internal` leave a PR out of the release notes (`.github/release.yml`).

## Git Workflow

**REQUIREMENT:** Do not push (especially force-push) unless explicitly requested by the user.

- **Merge commits only.** Pull requests are merged with a merge commit: never squash and never rebase-merge. Do not rewrite history that has been pushed. Never merge without the user's explicit go-ahead.
- **Branch naming:** `feat/<short-description>`, `docs/<short-description>`, `chore/<short-description>`, `spike/<short-description>`; for fixes, `fix/issue-<number>-<short-description>`.
- **Focused commits:** when work splits naturally into implementation, validation and docs, use separate contextual commits.
- **Stage explicitly:** use `git add <specific paths>`, never `git add -A` or `git commit -a`. Retry on an `index.lock` collision rather than deleting the lock.
- Do not commit scratch files, build outputs (`*.exe`, `dist/`, `target/`, `vendor/`) or large source archives.
- **Releases:** add the version's section to `CHANGELOG.md` in a PR and merge it; the tag `vX.Y.Z` is pushed on the merge commit only when the user asks. Release notes group the pull requests since the last release by label (`.github/release.yml`), so every PR needs its category label.
- **The build image:** CI runs in `ghcr.io/liminal-hq/ci-dos`, the DOS tier of the shared images defined in `liminal-hq/.github`, named by digest in the workflows. A tool or Rust version change is made there; moving to a newer image is a pull request here that changes the digest, so its checks run in the new image.
- **GitHub tooling:** prefer the `gh` CLI for repository, pull request, label, review and Actions work.

## Local Tooling

The stack is **Rust** (pinned in `rust-toolchain.toml`; the DOS target needs `-Zbuild-std`, with `RUSTC_BOOTSTRAP=1` set by `tools/build.sh`), **C** (DJGPP's cross-compiler) and **DOSBox Staging** for headless runs of the hosted track; the native track adds **NASM** and **QEMU** (`-cpu 486`), with Bochs and 86Box as opt-in Docker runs (`docs/NATIVE-EMULATORS.md`). `docs/TOOLCHAIN.md` describes the hosted pipeline. The DOS tools live in `vendor/` (not committed): `tools/setup.sh` links it to `DOS_TOOLCHAIN` in the CI image or downloads the tools. Do not install system packages to get a build working without asking.

```bash
tools/setup.sh                           # once: Rust pieces, then DJGPP, DOSBox Staging and CWSDPMI in vendor/
tools/check.sh                           # everything the CI runs: checks, build, headless run
tools/build.sh                           # dist/DESK.EXE and dist/CWSDPMI.EXE
tools/dosrun.sh dist DESK.EXE            # run a DOS program headless; prints its output, returns its exit status
tools/check-headers.sh                   # licence headers on source, script and workflow files
python3 -I tools/check-docs.py           # markdown style and links
python3 -I tools/build-hero.py [name] > assets/hero.svg    # rebuild the README banner
```

Treat the scripts as the source of truth for exact commands. Use scratch space under `$TMPDIR` (not `/tmp`) when several jobs share a machine.

## Code Conventions

- **Rust (`core/`, `sdk/`):** `no_std` with `alloc`; no floating-point maths from `std`; no `unsafe` without a one-line justification of what makes it sound. Only integers, pointers and lengths cross the Rust and C boundary, never structs passed by value.
- **C (`c/`, `launcher/`):** C99 for DJGPP, one concern per file, `volatile` for anything an interrupt handler touches, no allocation in an interrupt handler.
- **DOS limits (hosted track):** file names are 8.3; every hooked interrupt vector (IRQ 0, IRQ 1, the mouse callback) is restored on exit and before a handoff; the screen goes back to text mode before a handoff.
- **Rendering:** the Desk owns the palette. Fades and dims go through the DAC, never through the frame buffer; per-pixel work stays free of floating point.
- **The HAL:** the OS-independent core must not call DOS or DJGPP APIs directly. It reaches the machine only through the hardware abstraction layer, and each track implements that layer. DOS, BIOS and DJGPP calls belong to the hosted track's code (`c/`, `launcher/`).
- No speculative scaffolding: a directory, crate or abstraction arrives with the code that needs it.

## Testing

- Verify relevant changes before considering work complete, and state exactly what was and was not run.
- `tools/check.sh` is the pre-PR gate and what the CI runs. It already builds `DESK.EXE` and runs it headless; frame checksums join it as the code that needs them arrives.
- Fix a bug with a check that fails before the fix.
- Say plainly what could not be verified: nothing here has run on real hardware, and audio cannot be checked headless.

## Documentation

- Update the relevant document in the same branch when behaviour changes: `docs/SPEC.md` for what the Desk is, `docs/SDK.md` and `docs/FORMATS.md` for interfaces and file formats, `docs/HANDOFF.md` for the handoff, `README.md` for how to build and use it.
- Prefer stable reference docs over burying decisions in code comments or PR threads.
- No hard wrapping: see [Markdown Formatting](#markdown-formatting).

## Repository Layout

- `docs/` — the working design (direction, spec, SDK, formats, handoff, palette, app guidelines, open questions, naming)
- `design/` — boards and prototypes from the design brainstorm, reference only
- `core/` — the OS-independent shell in Rust (`no_std`), with the DOS target specification `dos.json`; today the skeleton's one function
- `c/` — the hosted track's hardware layer in C for DJGPP; today `main`, result output and the panic exit
- `launcher/` — `DESK.COM`, the hosted track's real-mode launcher
- `sdk/` — the app interface: the C header and a Rust wrapper
- `spikes/` — experiments that decide the architecture
- `tests/dos/` — test programs of our own for headless DOS runs
- `tools/` — repository checks, the DOS build pipeline (`setup`, `fetch-tools`, `build`, `coff2go32`, `dosrun`) and scripts
- `fonts/` — the four bitmap faces as pixel grids, the modern outline faces (Desk Sans, Mono and Serif), the emoji (`fonts/src/emoji/`, `docs/EMOJI.md`), and the Bun build that writes them as OTF, TTF, `.DFN` and `.DEM` files (`docs/FONTS.md`)
- `vendor/` and `dist/` — the downloaded DOS tools and the build output; neither is committed
- `assets/` — the README banner
- `.github/` — workflows, templates, dependency updates and release-note grouping

Directories for the native HAL, apps, the installer and the icons are created when their milestone starts. Spike 1 settled the layout of `apps/`: one Cargo workspace for the DOS target with a crate per app, built into `.DXE` modules by a script in `tools/` that is still to be written (`docs/S1-DXE3.md`).

## Licence and Copyright

This project is dual-licensed under Apache-2.0 OR MIT (`LICENSE-APACHE`, `LICENSE-MIT`).

**REQUIREMENT:** New and substantially rewritten source files (`.rs`, `.c`, `.h`, `.ts`, `.js`, `.py`, `.sh`) and workflow YAML MUST begin with a licence and copyright header:

```
// Brief one-line summary of what this file does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
```

Use `#` comments for shell scripts, Python and workflow YAML, and `/* */` for C. Do not add headers to markdown, JSON, lockfiles or other generated or config-only files, or to `design/`. Third-party material is listed in `NOTICE`. `tools/check-headers.sh` checks the headers.

## Agent Behaviour

- Read the relevant `docs/` section before implementing, and follow it.
- Make the change that was asked for; avoid speculative scaffolding and unrelated refactors.
- Do not push, merge, force-push, or alter shared settings without explicit instruction.
- State uncertainty and unverified items plainly rather than implying success.
- Stay within your assigned files when several agents work in the same checkout.
