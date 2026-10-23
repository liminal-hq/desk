// The scenarios in `tests/shell/`: every one at every depth it lists, through `MemoryPresent`, `LinearPresent` over a Vec-backed
// region and `PlanarPresent` over a four-plane model, compared with the committed `.sums`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `DESK_FRAMES_RECORD=1` rewrites each `.sums` and writes every frame as `NAME-DEPTH.RAW` and `NAME-DEPTH.PAL` in
// `shell/target/frames/`; `tools/shell-goldens.sh` turns those into the golden PNGs. Without it, a mismatch writes the same dumps
// and prints the `tools/frames.py` commands that show the difference.

mod common;

use common::*;
use desk_gfx::{FrameDigest, Present};
use desk_hal::{HalError, MmioRegion};
use desk_present::{read_back_linear, LinearPresent, PlanarPresent, PlaneSink};
use desk_shell::scenario::{apply, parse, run, RunError, Scenario, Step};
use desk_ui::Depth;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn bits(depth: Depth) -> u32 {
    if depth == Depth::Bits8 {
        8
    } else {
        4
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// `p` as the repository-relative path to type in a shell started at the repository root.
fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
}

/// Where a scenario's dumps go: a directory of its own, so two scenarios may share a frame name.
fn frames_dir(scenario: &Path) -> PathBuf {
    root().join("shell/target/frames").join(scenario.file_stem().unwrap())
}

/// One frame of a scenario: its name, the depth, the indices on screen and their digests.
struct Frame {
    name: String,
    depth: Depth,
    pixels: Vec<u8>,
    digest: FrameDigest,
}

impl Frame {
    fn sums_line(&self) -> String {
        let d = &self.digest;
        format!(
            "{} {} {}x{} {:08x} {:08x}",
            self.name,
            bits(self.depth),
            d.width,
            d.height,
            d.index,
            d.colour
        )
    }

    fn file_stem(&self) -> String {
        format!("{}-{}", self.name, bits(self.depth))
    }
}

fn digest_pixels(pixels: &[u8], depth: Depth) -> FrameDigest {
    FrameDigest::from_rows(W, H, &pal(depth), |y, buf| {
        let start = y as usize * W as usize;
        buf.copy_from_slice(&pixels[start..start + W as usize]);
    })
}

/// Four planes in memory, one byte per plane byte, as in `desk-present`'s own tests.
struct Planes {
    planes: [Vec<u8>; 4],
    selected: u8,
    pitch: usize,
}

impl Planes {
    fn new(pitch: usize, height: usize) -> Planes {
        Planes {
            planes: std::array::from_fn(|_| vec![0; pitch * height]),
            selected: 0,
            pitch,
        }
    }

    /// Recombines the planes into indices.
    fn indices(&self) -> Vec<u8> {
        let mut out = vec![0u8; (W * H) as usize];
        for y in 0..H as usize {
            for x in 0..W as usize {
                for (p, plane) in self.planes.iter().enumerate() {
                    out[y * W as usize + x] |= (plane[y * self.pitch + x / 8] >> (7 - x % 8) & 1) << p;
                }
            }
        }
        out
    }
}

impl PlaneSink for Planes {
    fn select(&mut self, plane: u8) -> Result<(), HalError> {
        self.selected = plane;
        Ok(())
    }

    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), HalError> {
        self.planes[self.selected as usize][offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

/// Steps `sc` at `depth` presenting into `out`, and calls `read` for the pixels on screen after each frame.
fn through<P: Present>(sc: &Scenario, depth: Depth, out: &mut P, mut read: impl FnMut(&mut P) -> Vec<u8>) -> Vec<Frame> {
    let mut shell = shell(depth);
    let mut frames = Vec::new();
    for step in &sc.steps {
        match step {
            Step::Frame(name) => {
                shell.frame(out).unwrap();
                let pixels = read(out);
                let digest = digest_pixels(&pixels, depth);
                assert_eq!(
                    digest,
                    shell.digest(),
                    "{depth:?} frame {}: the presented pixels are not the shell's",
                    name.as_str()
                );
                frames.push(Frame {
                    name: name.as_str().to_string(),
                    depth,
                    pixels,
                    digest,
                });
            }
            other => apply(&mut shell, other).unwrap(),
        }
    }
    frames
}

/// Through `MemoryPresent`: the runner itself first, so the `expect-damage` steps are checked, then the stepped run that keeps the
/// pixels.
fn through_memory(sc: &Scenario, depth: Depth) -> Vec<Frame> {
    let mut digests = Vec::new();
    run(sc, depth, &mut shell(depth), &mut present(depth), |name, s, _| {
        digests.push((name.as_str().to_string(), s.digest()));
        Ok::<(), RunError>(())
    })
    .unwrap_or_else(|e| panic!("{depth:?}: {e}"));
    let frames = through(sc, depth, &mut present(depth), |p| p.pixels.clone());
    assert_eq!(
        frames.iter().map(|f| (f.name.clone(), f.digest)).collect::<Vec<_>>(),
        digests,
        "{depth:?}"
    );
    frames
}

fn through_linear(sc: &Scenario) -> Vec<Frame> {
    // A pitch wider than the screen, as a VBE mode may have.
    let (pitch, mut buf) = (W + 64, vec![0u8; (W + 64) as usize * H as usize]);
    // SAFETY: the buffer outlives the region and is only reached through it while the presenter and the read-back use it.
    let mut region = unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) };
    let mut p = LinearPresent::new(&mut region, W, H, pitch).unwrap();
    // `read_back_linear` takes the region the presenter holds, so the read goes round it: the same Vec through a second region.
    // SAFETY: as above; the second region only reads, between frames.
    let mut reader = unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) };
    through(sc, Depth::Bits8, &mut p, |_| {
        let mut px = vec![0u8; (W * H) as usize];
        read_back_linear(&mut reader, W, H, pitch, &mut px).unwrap();
        px
    })
}

fn through_planar(sc: &Scenario) -> Vec<Frame> {
    let pitch = (W / 8) as usize;
    let mut p = PlanarPresent::with_sink(Planes::new(pitch, H as usize), W, H, pitch as u32).unwrap();
    through(sc, Depth::Bits4, &mut p, |p| p.sink().indices())
}

/// The scenario's frames in file order, depth by depth, from every presenter that supports the depth; the presenters must agree.
fn frames_of(sc: &Scenario) -> Vec<Frame> {
    let mut all = Vec::new();
    for &depth in &sc.depths {
        let memory = through_memory(sc, depth);
        let other = match depth {
            Depth::Bits8 => through_linear(sc),
            Depth::Bits4 => through_planar(sc),
        };
        assert_eq!(memory.len(), other.len());
        for (m, o) in memory.iter().zip(&other) {
            assert_eq!(m.sums_line(), o.sums_line(), "{depth:?}: the presenters disagree");
            assert!(
                m.pixels == o.pixels,
                "{depth:?} frame {}: the presenters' pixels differ",
                m.name
            );
        }
        if depth == Depth::Bits4 {
            assert!(
                memory.iter().all(|f| f.pixels.iter().all(|&p| p < 16)),
                "an index above 15 at depth 4"
            );
        }
        all.extend(memory);
    }
    all
}

fn write_dumps(scenario: &Path, frames: &[Frame]) {
    let dir = frames_dir(scenario);
    std::fs::create_dir_all(&dir).unwrap();
    for f in frames {
        let mut dac = Vec::with_capacity(768);
        for c in pal(f.depth) {
            dac.extend_from_slice(&[c.r, c.g, c.b]);
        }
        std::fs::write(dir.join(format!("{}.RAW", f.file_stem())), &f.pixels).unwrap();
        std::fs::write(dir.join(format!("{}.PAL", f.file_stem())), dac).unwrap();
    }
}

#[test]
fn every_scenario_matches_its_sums_through_every_presenter() {
    let dir = root().join("tests/shell");
    let mut scenarios: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "scn"))
        .collect();
    scenarios.sort();
    assert!(!scenarios.is_empty(), "no scenario in {}", dir.display());
    let record = std::env::var_os("DESK_FRAMES_RECORD").is_some_and(|v| v == "1");
    let mut failures = String::new();
    for path in scenarios {
        let sc = parse(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(sc.screen, (W, H), "{}: the host tests run at {W}x{H}", path.display());
        let frames = frames_of(&sc);
        let sums = path.with_extension("sums");
        let now: String = frames.iter().map(|f| f.sums_line() + "\n").collect();
        if record {
            write_dumps(&path, &frames);
            std::fs::write(&sums, &now).unwrap();
            continue;
        }
        let Ok(committed) = std::fs::read_to_string(&sums) else {
            writeln!(
                failures,
                "{} has no {}; record it with tools/shell-goldens.sh",
                rel(&path),
                rel(&sums)
            )
            .unwrap();
            write_dumps(&path, &frames);
            continue;
        };
        if committed == now {
            continue;
        }
        write_dumps(&path, &frames);
        let want: Vec<&str> = committed.lines().collect();
        writeln!(failures, "{} does not match {}:", rel(&path), rel(&sums)).unwrap();
        for (i, f) in frames.iter().enumerate() {
            let line = f.sums_line();
            if want.get(i) != Some(&line.as_str()) {
                writeln!(
                    failures,
                    "  frame {}:\n    committed {}\n    now       {}",
                    f.file_stem(),
                    want.get(i).unwrap_or(&"(none)"),
                    line
                )
                .unwrap();
                let stem = f.file_stem();
                let out = frames_dir(&path);
                let golden = dir.join("golden").join(path.file_stem().unwrap()).join(format!("{stem}.png"));
                writeln!(
                    failures,
                    "    python3 -I tools/frames.py png {0}/{1}.RAW {0}/{1}.PAL {2} {3} {0}/{1}.png && \\\n    python3 -I tools/frames.py diff {4} {0}/{1}.png {0}/{1}.diff.png",
                    rel(&out),
                    stem,
                    W,
                    H,
                    rel(&golden)
                )
                .unwrap();
            }
        }
        if want.len() > frames.len() {
            writeln!(
                failures,
                "  {} more line(s) in the .sums than frames",
                want.len() - frames.len()
            )
            .unwrap();
        }
    }
    assert!(
        failures.is_empty(),
        "\n{failures}\nIf the change is deliberate, re-record with tools/shell-goldens.sh and look at the PNGs.\n"
    );
}
