---
id: mergeday
n: 9
title: Merge day
date: 2026-10-22
summary: Seven pull requests merged into main in thirty minutes, each after merging main into its own branch, and the conflicts were nearly all in the same few shared files.
prs: [3, 1, 2, 6, 5, 4, 7]
---
By late evening there were seven open pull requests and every one of them warned in its description that it would conflict with the others. Three were stacked on other branches (#2 on `feat/fonts`, and #4 and #7 on `feat/walking-skeleton`) and four were based on the morning's `main`. They touched the same shared files: `tools/check.sh`, `.github/workflows/ci.yml`, `AGENTS.md`, the READMEs and the docs index. The order in which they were merged is the order of the timestamps, and it is **not** the order of their numbers: #3, #1, #2, #6, #5, #4, #7.

For each pull request the recipe was the same, and the history keeps it. Merge `main` into the branch (a commit titled "Merge main into …" whose body lists the conflicts), then merge the pull request into `main` with a merge commit. The first merge, #3 at 22:41, had nothing to resolve. Every later one did, and the six resolution notes are the real record of what was kept: below, the merge-day replay, drawn from `git log` and the commits' own messages.

<!-- explainer mergeday 3 1 2 6 5 4 7 -->

The conflicts, all of them, in one table. They are the message bodies of the six "Merge main into" commits.

<!-- figure conflicts -->

Read down the right-hand column and you will see the pattern. `tools/check.sh` is named in four of the six notes (a fifth says it merged cleanly), and every time the resolution is the same decision: **keep every step**, each as its own `step`, in a sensible order (fonts, then Rust lints, the DOS build and its headless run, the s1-dxe3 steps, the d0 steps, and the native boot test last). The workflow file was resolved the same way: both the "Install the font tools" step and the "Set up the DOS toolchain" step stay, and so does the QEMU and NASM install, in that order before `tools/check.sh`. The job timeout stayed at 15 minutes throughout, and CI passed on every merge commit. The README, `AGENTS.md` and docs index conflicts were resolved by keeping both sides' lines. One of them had a twist: when #5 (the direction) went in, its notes say it replaced stale statements such as "nothing boots yet", because by then something did.

## What the first-parent view shows

`git log --first-parent main` shows eight commits: the Initial Commit and one merge per pull request. `git rev-list --count main` shows {{m.commitsMergeDay}} at the end of that night, because each merge brings its branch with it. Both are right, and the difference is the point of merge-commit-only: the story of a change stays on its branch and the story of the project stays readable.

<!-- facts prsMergeDay commitsFirstMergeDay commitsMergeDay ciGreenMergeDay -->

## The shelves

### Catalogued
- All {{m.prsMergeDay}} merge-day pull requests are merged into `main` as merge commits, and CI concluded "success" on each merge commit (`gh run list --branch main`, {{m.ciGreenMergeDay}}).
- The conflict resolutions are recorded in the six merge commits' own messages, and the table above prints them as written.
- The merge order, the times and the commit hashes in the replay come from `git log`; nothing in it is reconstructed.

### Uncatalogued
- The headroom of the 15-minute CI timeout after the combined run: CI passed each time, but I did not measure how close it came.
- Whether the merged `main` builds on a machine other than CI and the author's: I rebuilt only the native image on it, not the whole of `tools/check.sh`.

### Wish list
- Moving QEMU and NASM into the `ci-dos` image, which would remove the `apt-get` step and its time (recommended in PR #6, not done).
- A shared drivers crate and the first shared HAL, which the roadmap marks as the next convergence step.

> **From the stacks — First parent.** `git log --first-parent` follows only the first parent of each merge commit, which, in the manual's words, "can give a better overview when viewing the evolution of a particular topic branch" because the merges into a branch are usually only about keeping up with upstream. The Desk's rule of merge commits only, and never squash, is what makes that overview work: a squash would hide the branch, and a rebase would erase the merge.
> Source: [git-log documentation](https://git-scm.com/docs/git-log)

```figures
id: merge-day
kind: explainer
track: shared
title: Merge day, replayed
alt: A step-through of seven pull requests merging into main in thirty minutes, each after merging main into its branch, drawn from the git history.
caption: Seven merges in thirty minutes, from git log.
at: explainer mergeday 3 1 2 6 5 4 7
thumb: steps
```
