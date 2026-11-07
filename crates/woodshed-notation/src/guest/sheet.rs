// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! The leaf's paint list as a `vector-sheet`: fills, strokes and text over theme roles.
//!
//! The leaf is built with sentinel colours (ink opaque black, accent opaque red) so each
//! painted colour maps back to a role and an opacity; the host never receives a colour.
use crate::{ColorF, NotationLabel};
use paint_list_api::{PaintCmd, items::PathCommand};
use sprigging::{PathData, StrokeCap, StrokeJoin, StrokeStyle};
use vector_sheet::{Anchor, Cap, Face, FillRule, Join, Op, Paint, Path, Role, Seg, VectorSheet};

/// Opaque black stands for the ink role.
pub(super) const INK: ColorF = ColorF {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// Opaque red stands for the accent role (its translucent wash keeps the red).
pub(super) const ACCENT: ColorF = ColorF {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const LABEL_SIZE: f32 = 12.0;

fn paint(color: ColorF) -> Paint {
    let role = if color.r > 0.5 && color.g < 0.5 {
        Role::Accent
    } else {
        Role::Ink
    };
    Paint::with_alpha(role, color.a)
}

/// Every subpath of the path is kept: counters in clefs, note heads and digits are
/// subpaths of one compound path, and the leaf's fills are non-zero (netrender's rule).
fn path(data: &PathData) -> Path {
    let segs = data
        .commands
        .iter()
        .map(|c| match *c {
            PathCommand::MoveTo(p) => Seg::Move(p.x, p.y),
            PathCommand::LineTo(p) => Seg::Line(p.x, p.y),
            PathCommand::QuadTo { control, to } => Seg::Quad(control.x, control.y, to.x, to.y),
            PathCommand::CurveTo {
                control1,
                control2,
                to,
            } => Seg::Cubic(control1.x, control1.y, control2.x, control2.y, to.x, to.y),
            PathCommand::Close => Seg::Close,
        })
        .collect();
    Path { segs }
}

fn stroke(path: Path, style: &StrokeStyle) -> Op {
    Op::Stroke {
        path,
        width: style.width,
        cap: match style.cap {
            StrokeCap::Butt => Cap::Butt,
            StrokeCap::Round => Cap::Round,
            StrokeCap::Square => Cap::Square,
        },
        join: match style.join {
            StrokeJoin::Miter => Join::Miter,
            StrokeJoin::Round => Join::Round,
            StrokeJoin::Bevel => Join::Bevel,
        },
        paint: paint(style.color),
    }
}

/// The sheet for a painted leaf of `width` by `height`, with its text labels.
pub(super) fn from_paint(
    cmds: &[PaintCmd],
    labels: &[NotationLabel],
    width: f32,
    height: f32,
) -> VectorSheet {
    let mut ops = Vec::new();
    for cmd in cmds {
        match cmd {
            PaintCmd::DrawRect(r) => {
                let b = r.placement.bounds;
                ops.push(Op::Fill {
                    path: Path::rect(b.min.x, b.min.y, b.max.x - b.min.x, b.max.y - b.min.y),
                    rule: FillRule::NonZero,
                    paint: paint(r.color),
                });
            },
            PaintCmd::DrawPath(p) => {
                let shape = path(&p.path);
                if let Some(fill) = p.fill {
                    ops.push(Op::Fill {
                        path: shape.clone(),
                        rule: FillRule::NonZero,
                        paint: paint(fill),
                    });
                }
                if let Some(style) = &p.stroke {
                    ops.push(stroke(shape, style));
                }
            },
            // The leaf paints no clips; any other command carries nothing a sheet draws.
            _ => {},
        }
    }
    for label in labels {
        ops.push(Op::Text {
            x: label.x,
            y: label.y,
            size: LABEL_SIZE,
            text: label.text.clone(),
            anchor: Anchor::Start,
            face: Face::Reading,
            paint: Paint::new(Role::Ink),
        });
    }
    VectorSheet { width, height, ops }
}
