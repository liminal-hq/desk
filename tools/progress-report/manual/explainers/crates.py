# The crate graph: every crate of `hal/` and `shell/` and the three programs that use them, with their path dependencies, by kind.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by the `Cargo.toml` files themselves. Each crate is a node of one kind (the HAL contract, a driver, the core, a presenter, a
# platform), each path dependency in `[dependencies]` an edge (those in `[dev-dependencies]` are drawn dashed and never count). The
# explainer refuses a cycle, a core crate that depends on a driver other than the event decoder `desk-kbd` or on `desk-present`, and a
# crate that no kind accounts for, so a new crate cannot appear in the picture unclassified.
import glob
import os
import tomllib

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-crates"

KINDS = [("platform", "Platforms"), ("core", "Shared core"), ("presenter", "Presenter"), ("driver", "Drivers"), ("hal", "HAL contract")]
CHAPTER = {"desk-hal": "m-hal", "desk-serial": "m-sysdrivers", "desk-pic": "m-sysdrivers", "desk-pit": "m-sysdrivers", "desk-i8042": "m-input", "desk-kbd": "m-input", "desk-int33": "m-input",
           "desk-vga": "m-graphics", "desk-hal-mock": "m-hal", "desk-gfx": "m-pixels", "desk-present": "m-pixels", "desk-font": "m-text", "desk-ui": "m-text", "desk-wm": "m-shell",
           "desk-shell": "m-shell", "desk-kernel": "m-platforms", "desk-shell-dos": "m-platforms", "rt": "m-rt"}
# What each directory holds, for the kind: a glob of manifests and the kind its crates have
SOURCES = [("hal/hal/Cargo.toml", "hal"), ("hal/drivers/*/Cargo.toml", "driver"), ("hal/platform/*/Cargo.toml", "platform"), ("shell/gfx/Cargo.toml", "core"), ("shell/font/Cargo.toml", "core"),
           ("shell/ui/Cargo.toml", "core"), ("shell/wm/Cargo.toml", "core"), ("shell/shell/Cargo.toml", "core"), ("shell/present/Cargo.toml", "presenter"), ("shell/dos/Cargo.toml", "platform"),
           ("kernel/Cargo.toml", "platform"), ("spikes/d0-rust-dos/rt/Cargo.toml", "platform")]
# Drivers a core crate may depend on: the decoder of key events has no registers in it.
CORE_MAY_USE = {"desk-kbd"}


def load(root=None, sources=SOURCES):
    root = root or gather.ROOT
    nodes, order = {}, []
    for pattern, kind in sources:
        for path in sorted(glob.glob(os.path.join(root, pattern))):
            with open(path, "rb") as f:
                data = tomllib.load(f)
            name = data.get("package", {}).get("name")
            if not name:
                continue
            nodes[name] = {"name": name, "kind": kind, "path": os.path.relpath(os.path.dirname(path), root), "deps": sorted(data.get("dependencies", {})), "dev": sorted(data.get("dev-dependencies", {}))}
            order.append(name)
    for n in nodes.values():
        n["deps"] = [d for d in n["deps"] if d in nodes]
        n["dev"] = [d for d in n["dev"] if d in nodes and d not in n["deps"]]
    return nodes, order


def check(nodes):
    problems = []
    for n in nodes.values():
        if n["name"] not in CHAPTER:
            problems.append("%s is a crate no chapter accounts for" % n["name"])
        if n["kind"] == "core":
            for d in n["deps"]:
                if nodes[d]["kind"] == "presenter":
                    problems.append("the core crate %s depends on the presenter %s" % (n["name"], d))
                if nodes[d]["kind"] in ("driver", "platform") and d not in CORE_MAY_USE:
                    problems.append("the core crate %s depends on %s %s, which is not the event decoder" % (n["name"], nodes[d]["kind"], d))
        if n["kind"] == "hal" and n["deps"]:
            problems.append("the HAL contract depends on %s" % ", ".join(n["deps"]))
        if n["kind"] == "driver":
            for d in n["deps"]:
                if nodes[d]["kind"] not in ("hal", "driver"):
                    problems.append("the driver %s depends on %s %s" % (n["name"], nodes[d]["kind"], d))
    state = {}

    def visit(x, stack):
        if state.get(x) == 2:
            return
        if state.get(x) == 1:
            problems.append("a dependency cycle: " + " -> ".join(stack + [x]))
            return
        state[x] = 1
        for d in nodes[x]["deps"]:
            visit(d, stack + [x])
        state[x] = 2
    for x in nodes:
        visit(x, [])
    return problems


def render(D, ctx):
    nodes, order = load()
    problems = check(nodes)
    if problems:
        raise ValueError("the crate graph refuses the manifests: " + "; ".join(problems))
    rows = {k: [n for n in order if nodes[n]["kind"] == k] for k, _ in KINDS}
    W, ROWH = 820, 76
    pos = {}
    for r, (k, _) in enumerate(KINDS):
        row = rows[k]
        gap = (W - 150) / max(1, len(row))
        for i, n in enumerate(row):
            pos[n] = (140 + gap * i + gap / 2, 30 + r * ROWH)
    step_of = {"hal": 0, "driver": 1, "core": 2, "presenter": 3, "platform": 4}  # the HAL first, platforms last; the tests-only edges at step 5
    o = ['<svg viewBox="0 0 %d %d" class="diagram m-cratefig" role="img" aria-label="The crates of the HAL, the drivers, the shared core, the presenter and the platforms, with the dependencies between them">' % (W, 30 + len(KINDS) * ROWH)]
    for r, (k, label) in enumerate(KINDS):
        o.append('<text x="4" y="%d" font-size="13" class="d-muted" data-from="%d">%s</text>' % (35 + r * ROWH, step_of[k], esc(label)))
    for n in order:
        for d in nodes[n]["deps"]:
            (x1, y1), (x2, y2) = pos[n], pos[d]
            o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="cg-edge cg-from-%s" data-from="%d"/>' % (x1, y1 + 14, x2, y2 - 14, nodes[n]["kind"], max(step_of[nodes[n]["kind"]], step_of[nodes[d]["kind"]])))
        for d in nodes[n]["dev"]:
            (x1, y1), (x2, y2) = pos[n], pos[d]
            o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="cg-dev" data-from="%d"/>' % (x1, y1 + 14, x2, y2 - 14, 5))
    for n in order:
        x, y = pos[n]
        k = nodes[n]["kind"]
        label = n.replace("desk-", "")
        o.append('<g data-from="%d"><a href="%s">%s<rect x="%g" y="%g" width="84" height="28" rx="6" class="cg-%s"/><text x="%g" y="%g" text-anchor="middle" font-size="12" class="cg-t">%s</text></a></g>' % (
            step_of[k], esc(ctx.href("chapter:" + CHAPTER[n])), "<title>%s (%s), in %s</title>" % (esc(n), esc(k), esc(nodes[n]["path"])), x - 42, y - 14, k, x, y + 4, esc(label)))
    o.append("</svg>")
    key = ('<ul class="m-key"><li><span class="sw cg-hal"></span> HAL</li><li><span class="sw cg-driver"></span> driver</li><li><span class="sw cg-core"></span> core</li><li><span class="sw cg-presenter"></span> presenter</li><li><span class="sw cg-platform"></span> platform</li>'
           '<li>solid line: a dependency; dashed: a dev-dependency (tests only)</li></ul>')
    text = "".join("<tr><th scope=\"row\"><code>%s</code></th><td>%s</td><td>%s</td><td>%s</td></tr>" % (esc(n), esc(nodes[n]["kind"]), esc(", ".join(nodes[n]["deps"]) or "nothing"), esc(", ".join(nodes[n]["dev"]) or "nothing")) for n in order)
    table = ('<details class="an-text"><summary>The same graph as text: every crate, its kind and what it depends on</summary><div class="tbl"><table><caption class="sr">Crates, kinds and dependencies</caption>'
             '<thead><tr><th scope="col">Crate</th><th scope="col">Kind</th><th scope="col">Depends on</th><th scope="col">Tests also use</th></tr></thead><tbody>%s</tbody></table></div></details>') % text
    names = lambda k: ", ".join(n.replace("desk-", "") for n in rows[k])
    steps = [
        ("The HAL contract", Raw(esc("%s: the traits and types every driver is written against. It depends on nothing in this repository." % names("hal")))),
        ("The drivers", Raw(esc("%s. Each depends on the HAL, and desk-i8042 and desk-int33 on the event decoder desk-kbd: no driver depends on the core or on a platform, so the same driver code runs under every platform." % names("driver")))),
        ("The shared core", Raw(esc("%s. The core draws and decides, and reaches the machine only through the HAL's types; desk-ui and desk-shell use the event decoder desk-kbd, which holds no registers. No core crate depends on a driver's register-level API or on the presenter, and the build refuses one that does." % names("core")))),
        ("The presenter", Raw(esc("%s turns the core's back buffer into pixels on a screen through desk-vga. The core never depends on it: the platform wires the two together." % names("presenter")))),
        ("The platforms", Raw(esc("%s. A platform depends on everything: the kernel and the DOS program each name the HAL, the drivers they use, the core and the presenter, and put them together (M16). desk-hal-mock is the test platform that the drivers' host tests run against." % names("platform")))),
        ("Edges that only tests use", Raw(esc("The dashed lines are dev-dependencies. desk-gfx lists desk-vga only for a test that compares its cursor sprite with the driver's arrow, and desk-shell lists desk-present only to run scenarios in its tests. They are not dependencies of the code that ships."))),
    ]
    cap = ('Read from the <code>Cargo.toml</code> files of <code>hal/</code>, <code>shell/</code>, <code>kernel/</code> and <code>spikes/d0-rust-dos/rt</code>. The build refuses a cycle, a core crate that depends on a driver other than the event decoder or on the presenter, and a crate that no chapter accounts for. Each node links to the chapter that teaches it.')
    return stepper(ID, "The crates and their dependencies, layer by layer", "".join(o) + key + table, steps, cap, ctx, "x-wide")
