---
id: m-one
n: M1
part: I
title: The first fixture chapter
summary: A chapter used only by the generator's test; it points at [chapter two](chapter:m-two).
date: 2026-10-23
reviewed: 44f0265
watches: [tools/progress-report/manual.py]
log: [one]
anatomy: [desk:kernel]
---
## What it is

A fixture chapter that has every section of the template. The checker is [`check_chapter`](sym:tools/progress-report/manual.py::check_chapter). A [segment](term:segment) is a made-up term here. The heap is {{state.fix.heap}}.

## How it works

1. The first step.
2. The second step.

## Why it is this way

Because the test needs one.

## The code

| File | What it holds |
|---|---|
| [`manual.py`](repo:tools/progress-report/manual.py) | the checks |
| [`check_chapter`](sym:tools/progress-report/manual.py::check_chapter) | one chapter's problems |

## Worked example

Two plus two is four.

## State of this layer

```status
id: fix.heap
label: The fixture's first mechanism
state: built-verified
evidence: pr:1, test:tools/progress-report/tests/test_report.py::test_fixture_manual_site
note: A real test name.

id: fix.unrun
label: A mechanism whose pull request is not merged
state: built-verified
evidence: pr:1, repo:AGENTS.md

id: fix.later
label: A planned mechanism
state: planned
evidence: doc:docs/MANUAL-PLAN.md#the-chapter-template
```

> **From the stacks — A fixture.** A sidebar of two sentences. It is only here to be rendered.
> Source: [example](https://example.com)

## Try it yourself

```try
needs: host: python3
ran: 2026-10-23
$ python3 -I tools/progress-report/tests/test_report.py
OK
```

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Segment
def: A made-up term for the fixture.
```

## Revisions

- 2026-10-23: written for the generator's tests, against `44f0265`.
