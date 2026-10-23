// The scenario runner: the steps applied to a shell, a frame presented and captured at each `frame` step.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// One runner serves the host tests and both platforms, so the same steps give the same frames everywhere. A frame is presented at
// each `frame` step (not after every step), so what an `expect-damage` step checks is all the damage since the previous frame.

use crate::scenario::{type_events, FrameName, Scenario, Step};
use crate::Shell;
use alloc::string::String;
use core::fmt;
use desk_gfx::{FrameStats, Present};
use desk_hal::HalError;
use desk_ui::Depth;

/// Why a run stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// A step names a window that is not open.
    UnknownWindow(String),
    /// A `type` step has a character that no key types.
    Untypable(char),
    /// The presenter failed.
    Present(HalError),
    /// An `expect-damage` step saw a frame that wrote more than it allows.
    DamageExceeded {
        depth: Depth,
        pixels: u32,
        max_pixels: u32,
        rects: u8,
        max_rects: Option<u8>,
    },
    /// An `expect-damage` step came before any frame was presented.
    NoFrame,
    /// The capture callback refused a frame.
    Capture(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::UnknownWindow(n) => write!(f, "no window named {n}"),
            RunError::Untypable(c) => write!(f, "no key types {c:?}"),
            RunError::Present(e) => write!(f, "presenting failed: {e}"),
            RunError::DamageExceeded {
                depth,
                pixels,
                max_pixels,
                rects,
                max_rects,
            } => {
                write!(
                    f,
                    "a frame at depth {depth:?} wrote {pixels} pixels in {rects} rectangles, over the {max_pixels}"
                )?;
                match max_rects {
                    Some(m) => write!(f, " and {m} allowed"),
                    None => write!(f, " allowed"),
                }
            }
            RunError::NoFrame => f.write_str("expect-damage before any frame"),
            RunError::Capture(m) => write!(f, "capture failed: {m}"),
        }
    }
}

impl From<HalError> for RunError {
    fn from(e: HalError) -> RunError {
        RunError::Present(e)
    }
}

/// Applies one step's effect on `shell`: everything except presenting, so `Frame` and `ExpectDamage` do nothing here.
pub fn apply(shell: &mut Shell, step: &Step) -> Result<(), RunError> {
    let unknown = |ok: bool, name: &str| {
        if ok {
            Ok(())
        } else {
            Err(RunError::UnknownWindow(String::from(name)))
        }
    };
    match step {
        Step::Open { name, outer, title } => {
            shell.open(name, *outer, title);
            Ok(())
        }
        Step::Label { window, x, y, text } => unknown(shell.label(window, *x, *y, text), window),
        Step::Field { window, x, y, width } => unknown(shell.field(window, *x, *y, *width), window),
        Step::Close(name) => unknown(shell.close(name), name),
        Step::Focus(name) => unknown(shell.focus(name), name),
        Step::Event(e) => {
            shell.handle(*e);
            Ok(())
        }
        Step::Type(text) => {
            for e in type_events(text).map_err(RunError::Untypable)? {
                shell.handle(e);
            }
            Ok(())
        }
        Step::Frame(_) | Step::ExpectDamage { .. } => Ok(()),
    }
}

/// Runs every step of `sc` at `depth`. At each `Frame` step it presents a frame and then calls `capture(name, shell, stats)`; an
/// `ExpectDamage` step for this depth fails the run when the last frame wrote more pixels or rectangles than it allows (a step for
/// another depth is skipped). The first error stops the run.
pub fn run<P: Present>(
    sc: &Scenario,
    depth: Depth,
    shell: &mut Shell,
    out: &mut P,
    mut capture: impl FnMut(&FrameName, &Shell, FrameStats) -> Result<(), RunError>,
) -> Result<(), RunError> {
    let mut last: Option<FrameStats> = None;
    for step in &sc.steps {
        match step {
            Step::Frame(name) => {
                let stats = shell.frame(out)?;
                last = Some(stats);
                capture(name, shell, stats)?;
            }
            Step::ExpectDamage {
                depth: d,
                max_pixels,
                max_rects,
            } if *d == depth => {
                let stats = last.ok_or(RunError::NoFrame)?;
                if stats.presented > *max_pixels || max_rects.is_some_and(|m| stats.rects > m) {
                    return Err(RunError::DamageExceeded {
                        depth,
                        pixels: stats.presented,
                        max_pixels: *max_pixels,
                        rects: stats.rects,
                        max_rects: *max_rects,
                    });
                }
            }
            other => apply(shell, other)?,
        }
    }
    Ok(())
}
