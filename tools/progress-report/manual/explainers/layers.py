# The layers of the repository: which layer may name which, from the rules the layering check reads.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by `tools/layers/layers.txt`, the file `tools/layers` reads on every run of `tools/check.sh`. Each `layer` line is a box (its
# paths are the label), each `may-use` line an arrow from a layer to the layers it may name, and each `allow` line an exception that the
# last step lists. A layer that may name `*` is drawn above the others and gets no arrows. The explainer refuses a rule file that has a
# layer with no colour here, a `may-use` of a layer that does not exist, a layer path that is not in the repository, or a dependency
# cycle among the named layers, so the picture cannot drift from the file.
import os

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-layers"
# Paths a clean checkout does not have: the tools that `tools/setup.sh` fetches and the build outputs
NOT_TRACKED = {"vendor", "dist"}
RULES = "tools/layers/layers.txt"
# The colour class of each layer, shared with the crate graph's
KIND = {"hal": "hal", "dos": "hal", "shared": "core", "adapter-dos": "core", "system-native": "platform", "hosted-skeleton": "platform",
        "spikes": "presenter", "outside": "driver"}


def load(root=None):
    root = root or gather.ROOT
    layers, order, may, allow = {}, [], {}, []
    with open(os.path.join(root, RULES), encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            word, *rest = line.split()
            if word == "layer" and len(rest) >= 2:
                layers[rest[0]] = rest[1:]
                order.append(rest[0])
            elif word == "may-use" and len(rest) >= 2:
                may.setdefault(rest[0], []).extend(rest[1:])
            elif word == "allow" and len(rest) >= 3:
                allow.append((rest[0], rest[1], " ".join(rest[2:])))
            else:
                raise ValueError("%s:%d: cannot read %r" % (RULES, n, line))
    return {"layers": layers, "order": order, "may": may, "allow": allow}


def check(d, root=None):
    root = root or gather.ROOT
    problems = []
    for name, paths in d["layers"].items():
        if name not in KIND:
            problems.append("the layer %s has no colour in the explainer" % name)
        for p in paths:
            if p not in NOT_TRACKED and not os.path.exists(os.path.join(root, p)):
                problems.append("the layer %s names %s, which is not in the repository" % (name, p))
    for name, used in d["may"].items():
        if name not in d["layers"]:
            problems.append("may-use names the layer %s, which does not exist" % name)
        for u in used:
            if u != "*" and u not in d["layers"]:
                problems.append("%s may use %s, which is not a layer" % (name, u))
    named = {n: [u for u in d["may"].get(n, []) if u != "*" and u != n] for n in d["layers"]}
    state = {}

    def visit(x, stack):
        if state.get(x) == 2:
            return
        if state.get(x) == 1:
            problems.append("a cycle among the layers: " + " -> ".join(stack + [x]))
            return
        state[x] = 1
        for u in named.get(x, []):
            if u in d["layers"]:
                visit(u, stack + [x])
        state[x] = 2
    for x in d["layers"]:
        visit(x, [])
    return problems


def ranks(d):
    """Rank 0 is the layer that names nothing; a layer is one above the highest it names; layers that may name everything come last, in file order."""
    star = [n for n in d["order"] if "*" in d["may"].get(n, [])]
    rank = {}

    def of(n):
        if n not in rank:
            rank[n] = 0
            used = [u for u in d["may"].get(n, []) if u != "*" and u != n]
            rank[n] = 1 + max([of(u) for u in used], default=-1)
        return rank[n]
    for n in d["order"]:
        if n not in star:
            of(n)
    top = max(rank.values(), default=-1)
    for i, n in enumerate(star):
        rank[n] = top + 1 + i
    return rank


def render(D, ctx):
    d = load()
    problems = check(d)
    if problems:
        raise ValueError("the layers explainer refuses " + RULES + ": " + "; ".join(problems))
    rank = ranks(d)
    top = max(rank.values())
    rows = {r: [n for n in d["order"] if rank[n] == r] for r in range(top + 1)}
    W, ROWH, BW = 820, 78, 150
    pos = {}
    for r in range(top + 1):
        row = rows[r]
        gap = (W - 20) / max(1, len(row))
        for i, n in enumerate(row):
            pos[n] = (10 + gap * i + gap / 2, 34 + (top - r) * ROWH)
    o = ['<svg viewBox="0 0 %d %d" class="diagram m-cratefig" role="img" aria-label="The layers of the repository, with an arrow from each layer to the layers it may name">' % (W, 34 + top * ROWH + 40)]
    for n in d["order"]:
        for u in d["may"].get(n, []):
            if u in pos and u != n:
                (x1, y1), (x2, y2) = pos[n], pos[u]
                o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="cg-edge" data-from="%d"/>' % (x1, y1 + 20, x2, y2 - 20, rank[n]))
    for n in d["order"]:
        x, y = pos[n]
        paths = " ".join(p + "/" for p in d["layers"][n])
        shown = paths if len(paths) <= 24 else paths[:22] + "…"
        o.append('<g data-from="%d"><rect x="%g" y="%g" width="%d" height="40" rx="6" class="cg-%s"/><title>%s: %s</title>'
                 '<text x="%g" y="%g" text-anchor="middle" font-size="13" class="cg-t">%s</text><text x="%g" y="%g" text-anchor="middle" font-size="10.5" class="cg-t">%s</text></g>' % (
                     rank[n], x - BW / 2, y - 20, BW, KIND[n], esc(n), esc(paths), x, y - 3, esc(n), x, y + 12, esc(shown)))
    o.append("</svg>")
    key = '<ul class="m-key"><li>an arrow runs from a layer to a layer it may name; nothing else may be named</li><li>a layer drawn without arrows may name everything</li></ul>'
    trs = "".join('<tr><th scope="row"><code>%s</code></th><td>%s</td><td>%s</td></tr>' % (
        esc(n), esc(", ".join(p + "/" for p in d["layers"][n])), esc("everything" if "*" in d["may"].get(n, []) else ", ".join(d["may"].get(n, [])) or "nothing")) for n in d["order"])
    table = ('<details class="an-text"><summary>The same layers as text: where each is and what it may name</summary><div class="tbl"><table><caption class="sr">Layers, paths and what each may name</caption>'
             '<thead><tr><th scope="col">Layer</th><th scope="col">Paths</th><th scope="col">May name</th></tr></thead><tbody>%s</tbody></table></div></details>') % trs
    steps = []
    for r in range(top + 1):
        bits = []
        for n in rows[r]:
            used = d["may"].get(n, [])
            who = "everything" if "*" in used else (", ".join(used) if used else "nothing else")
            bits.append("%s (%s) may name %s." % (n, ", ".join(p + "/" for p in d["layers"][n]), who))
        steps.append(("Rank %d: %s" % (r, ", ".join(rows[r])), Raw(esc(" ".join(bits)))))
    ex = d["allow"]
    steps.append(("Recorded exceptions", Raw(esc(
        ("%d exception is recorded in the rules: " if len(ex) == 1 else "%d exceptions are recorded in the rules: ") % len(ex)
        + " ".join("edges from %s to %s are accepted (%s)." % (a, b, why) for a, b, why in ex)
        + " A line that no edge needs is itself an error, so an exception goes in the change that removes its edge." if ex else "No exception is recorded in the rules."))))
    cap = ('Read from <code>%s</code>, the rules <code>tools/layers</code> checks on every run of <code>tools/check.sh</code>; the page <code>docs/LAYERS.md</code> explains them. '
           'The build refuses a layer with no colour, a rule that names a layer that does not exist, a path that is not in the repository and a cycle. Whether the files obey the rules is the check\'s job, not this picture\'s.') % RULES
    return stepper(ID, "The layers of the repository, and what each may name", "".join(o) + key + table, steps, cap, ctx, "x-wide")
