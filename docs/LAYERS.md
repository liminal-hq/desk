# The layers of the repository

Desk stays in one repository. This page says how the repository is divided inside, so that the parts that are meant to stand alone can be seen to stand alone, and so that a later extraction is a matter of copying directories and not of untangling them. The owner decided on one repository on 2026-11-19, after [`REPOSITORY-BREAKUP-PROPOSAL.md`](REPOSITORY-BREAKUP-PROPOSAL.md) and a review of the tree; the work is R148 in [`ROADMAP.md`](ROADMAP.md).

The rules are in [`tools/layers/layers.txt`](../tools/layers/layers.txt), which is the authority; this page explains them. `tools/layers` reads them on every run of `tools/check.sh` (the `rust:tools` part) and fails when a file names one in a layer it may not use.

## The layers

A layer may name itself and the layers listed after its arrow, and nothing else. Dependencies point down.

| Layer | Where | May name | What it is |
|---|---|---|---|
| `hal` | `hal/` | nothing | The hardware abstraction layer: the traits, the drivers written against them, and the mock platform for host tests |
| `dos` | `dos/` | `hal` | The Rust-on-DOS foundation: the runtime (`rt`), the `dos-elf.json` target specification and the `link.ld` linker script |
| `shared` | `shell/` (except `shell/dos/`), `module/`, `fonts/`, `tests/shell/`, `tests/module/` | `hal` | The core that both tracks share: the shell crates, the module format and loader, the fonts and the goldens |
| `adapter-dos` | `shell/dos/` | `shared`, `dos`, `hal` | The shell on DOS: the hosted adapter |
| `system-native` | `kernel/`, `boot/`, `tests/native/` | `shared`, `hal` | The native track: the boot sector and the kernel, which composes the shell for now |
| `hosted-skeleton` | `core/`, `c/`, `launcher/`, `sdk/`, `tests/dos/` | `hal` | The first hosted skeleton, `DESK.EXE` in C over a Rust core |
| `spikes` | `spikes/` | everything | Experiments. A spike may name anything; nothing outside `spikes/` may name a spike |
| `outside` | `tools/`, `docs/`, `design/`, `assets/`, `.github/`, `.config/`, `vendor/`, `dist/` | everything | Not code in a layer. Nothing names it, except that build outputs in `dist/` and tools in `vendor/` are not edges |

`dos` and `system-native` never name each other: the foundation knows nothing of the native track, and the kernel does not use DOS.

**Where `dos/` is today.** Until step 5 of R148 lands, the runtime, target specification and linker script are in `spikes/d0-rust-dos/` (`rt/`, `dos-elf.json`, `link.ld`), and `layers.txt` records the shell on DOS naming them there as an exception. After step 5 the exception is deleted and the files are in `dos/`.

## What counts as an edge

The same things `tools/ci-scope.sh` follows when it works out what a change affects (its `closure()`), and the check keeps the two in step:

- a Cargo path dependency (`path = "..."` in a dependencies section);
- the target specification or linker script a `.cargo/config.toml` names (`target = "..."`, `-T...`);
- a file compiled in with `include_bytes!`, `include_str!` or `include!`;
- a module named by `#[path = "..."]`.

It does not see a path in a shell script, a string such as the one in `module/pack/tests/real.rs`, or what a Cargo manifest says in a comment. Those are for review.

## Recorded exceptions

An `allow` line in `layers.txt` records one exception and says why. A line that no edge needs is itself an error, so an exception is deleted in the change that removes its edge.

- Until step 5 of R148, the shell on DOS names the DOS runtime, target specification and linker script in a spike. The scene drawing (now `kernel/src/scene.rs`) and the key fixture (now in `hal/drivers/int33/testdata/`) were the other two; each moved down to the layer that owns it, and its exception was deleted.
- `module/mods` is built for the kernel's target specification, `kernel/i486-desk-none.json`, because the loader's test modules run in the kernel. `module/pack/tests/real.rs` names the same file in a string the check cannot see. This one stays until the module target stops being the kernel's.

## Seams that are known and left alone

- **The native composition.** `kernel/src/shell.rs` composes the shell inside the kernel, so `system-native` is both the platform and the composition. Splitting them is the "native composition seam" of the breakup proposal; nothing else needs it yet.
- **The legacy C route.** `core/`, `c/` and `tools/build.sh` still build `DESK.EXE`. They stay until the loader replaces them.

## Extracting the foundation later

`hal/` and `dos/` name nothing above themselves, so the pair could be copied into a repository of its own with their history, for example by a path filter on `hal` and `dos`. Such a copy would also need the root `rust-toolchain.toml` and the licence files (`LICENSE-APACHE`, `LICENSE-MIT`, `NOTICE`), which are not in either directory, and the DOS packer and harness in `tools/`, which move into `dos/` in the deferred steps of R148. Nothing in this repository depends on that happening, and the owner has not decided it.
