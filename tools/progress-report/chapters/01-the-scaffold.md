---
id: scaffold
n: 1
title: The scaffold
date: 2026-10-22
summary: The Initial Commit, AGENTS.md, the CI image and the checks: the conventions every later chapter had to obey.
prs: []
---
A good archive starts with its rules of accession, and the Desk wrote its rules before it wrote any code. The Initial Commit, [`a4b0d86`](commit:a4b0d86), added 84 files and 19,589 lines and, by its own account, **no code at all**: only the plumbing and the organisation. `AGENTS.md` and `CLAUDE.md` set the house style (Canadian English, unwrapped markdown, Conventional Commits after that first commit, merge commits only, a licence header on every source file, and no pushes unasked). The `.github/` folder held the CI workflow, the issue and pull request templates, a Dependabot file with version updates switched off (`open-pull-requests-limit: 0`, so nothing fires yet), and the label-grouped release notes.

Three small checks went in with it, and they are the reason I can trust the other chapters. `tools/check-headers.sh` insists on the licence header, `tools/check-docs.py` insists on unwrapped paragraphs, real em dashes, Canadian spellings and working relative links, and `tools/check.sh` runs both. The commit message adds that the checks "were also run against deliberately bad input to confirm they fail", which is the right way to trust a checker: you must see it say no.

The hero banner, an animated SVG in `assets/hero.svg`, is built by `tools/build-hero.py` and honours `prefers-reduced-motion`. It is the banner at the top of the front page of this archive.

<!-- figure hero -->

<!-- facts docs files -->

One field note for the cataloguer. Every merged change since has had to pass the same `tools/check.sh` in the shared `ci-dos` container image, which the workflow names **by digest**. The reason is in `AGENTS.md`: a tool or Rust version change is made in the image and arrives here as a pull request that changes the digest, so the new image is checked before anyone relies on it.

## The shelves

### Catalogued
- The licence header and markdown style checks exist and run in CI: the CI conclusion on each of the {{m.prs}} merge commits is "success" (`gh run list --branch main`).
- The checks were tried against bad input before being trusted (stated in the Initial Commit message; I have not re-run that experiment).
- The banner rebuilds from `tools/build-hero.py`; the Initial Commit says it was rendered in headless Chromium and rebuilt byte for byte.

### Uncatalogued
- The banner in any renderer other than headless Chromium.
- The Dependabot configuration: it is switched off, so it has never opened a pull request and has never been seen working.

### Wish list
- Release notes and a first release: no tag exists on the repository yet (`git tag` is empty), and `AGENTS.md` describes a `CHANGELOG.md` that has not been written.
- Moving the QEMU and NASM install out of the CI workflow and into the `ci-dos` image, which PR #6 recommends and does not do.

> **From the stacks — The 8.3 rule.** The Desk's launcher is called `DESK.COM` and its core `DESKCORE.EXE` because DOS file names have a name of at most eight characters and an extension of at most three. As the story is usually told, the limit comes from the FAT file system's directory entries, and Windows 95 was where long file names reached ordinary users in 1995. For the Desk this is a hosted-track rule: `docs/DIRECTION.md` notes that the native track is not bound by it.
> Source: [8.3 filename, Wikipedia](https://en.wikipedia.org/wiki/8.3_filename)
