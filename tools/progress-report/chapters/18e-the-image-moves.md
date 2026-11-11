---
id: ciimage
n: 22
title: The image moves
date: 2026-11-11
summary: CI went red on 11 November because a step asked a package mirror for QEMU and NASM and the runner could not reach it; the fix was to stop asking and move to a build image that already carries them.
prs: [70]
---
A short chapter, about plumbing, in the spirit of chapter 1. The repository's checks run in `ghcr.io/liminal-hq/ci-dos`, the DOS tier of the owner's shared build images, named in the workflow by digest. Until 11 November the pinned image carried the DOS tools and Rust but not QEMU or NASM, which the native track needs, so the workflow installed them with `apt-get` as a step of its own. That step is the one that broke.

**What happened.** From the pull request's description: since 15:27 UTC on 11 November the runner could not resolve `archive.ubuntu.com`, and every run, including the push builds on `main`, failed at the "Install QEMU and NASM" step. The strip below shows the day. The first red run is on `b928309`, the merge of #65, at 15:27; nothing in the code under test had changed. I take the cause from the pull request, not from a log I read myself: the mirror problem was outside the repository.

<!-- figure ci-runs -->

**The fix.** Pull request #70 ({{pr70.merge}}, merged 17:21 UTC) pins `ci-dos` to the image published on 10 November, which carries QEMU 8.2.2, NASM 2.16.01 and Rust 1.98.0 with `rust-src` (matching `rust-toolchain.toml`) as well as the DOS toolchain, and removes the `apt` step, so the job no longer depends on the Ubuntu mirrors at all. The run on {{pr70.merge}} passed. `AGENTS.md` already described the rule: a tool or Rust version change is made in the shared image, and moving to a newer image is a pull request here that changes the digest, so its own checks run in the new image. This was that rule being followed.

**Reading the strip honestly.** Green, red and cancelled mean different things. Three of the later runs, on #67, #69 and #72, show *cancelled*: the workflow cancels a run when a newer push arrives on the same ref, and these merged within minutes of each other. A cancelled run is not a failure and not a pass. The completed run on {{pr71.merge}} passed in the new image, and it contains all of them; the run on #73's merge commit, the last merge of the day, had not finished when the strip was gathered, so the strip does not claim it. Main's CI is green again, as the passes on {{pr70.merge}} and {{pr71.merge}} show. One more detail worth keeping: the image's QEMU is 8.2.2, while the live-input results of chapter 19 were measured with QEMU 11.1.2 on the host. The checks that passed in CI ran on the older QEMU, so the project now has the native checks passing on two QEMU versions, though the CI's pass is a build-and-check run, not a re-measurement of every live scenario.

> **From the stacks — Why a run is "cancelled".** GitHub's concurrency setting groups runs, and its documentation says that to also cancel a run already in progress in the same group you specify `cancel-in-progress: true`. The Desk's workflow does so (see `.github/workflows/ci.yml`), which is kind to the runners and unkind to an archivist: when pull requests merge a few minutes apart, every run but the last is cancelled, and the last one carries all of the work. For the Desk it means a cancelled run is never counted as a pass or a failure, and the next completed run on `main` is the evidence.
> Source: [GitHub Docs, using concurrency](https://docs.github.com/en/actions/using-jobs/using-concurrency)

**What was not run.** The pull request says `tools/check.sh` in the new image was not run locally; its own CI run was the first. The manual's recorded commands still name the old image digest, and were left as dated runs.

## The shelves

### Catalogued
- The workflow is pinned to the 20261110 image and has no `apt` step (#70, {{pr70.merge}}); CI passed on {{pr70.merge}} and on {{pr71.merge}}.
- The failure window and the conclusions of the day's runs are as `gh run list --branch main` reports them (source recorded under the figure).

### Uncatalogued
- The cause of the failed step, read from the pull request's description and not from the log, which I did not open.
- Whether the mirror failure will recur for any tool that is still installed at run time; nothing else is, as far as the workflow shows.
- The CI image's contents beyond the versions the pull request lists.

### Wish list
- Path-scoped pull request checks and a nightly full gate, as Lieutenant Fizz 2 has (R137).

```figures
id: ci-runs
kind: diagram
track: shared
title: CI on main, 11 November
alt: A list of the CI runs on main on 11 November in time order, each marked passed, failed or cancelled, showing the first failure at 15:27 UTC and the first pass in the new image at 17:21 UTC.
caption: The day's runs on main, from gh.
at: figure ci-runs
thumb: svg
date: 2026-11-11
```
