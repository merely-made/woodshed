use super::*;

impl Leaf for FretboardLeaf {
    fn measure(&mut self, _known: SizeHint, _available: SizeHint) -> Size {
        self.size
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        if self.string_count == 0 {
            self.dirty = false;
            return;
        }
        let g = self.geom();
        let orient = self.orientation;
        // Content spans (with pad): across the strings, and along the neck.
        let cross0 = PAD;
        let cross1 = PAD + g.cross_len();
        let neck0 = PAD;
        let neck1 = PAD + g.neck_len();

        // Inlays: faint marks on the neck's centre line (single) or paired
        // (octaves), at absolute frets so a windowed board still anchors to the
        // real 5/7/12 dots. `to_xy` places them for whichever orientation.
        {
            let ir = 2.5;
            let mid = g.cross_len() / 2.0;
            let mut inlay = |along: f32, cross: f32| {
                let (x, y) = g.to_xy(along, cross);
                cx.fill_rect(x - ir, y - ir, ir * 2.0, ir * 2.0, C_INLAY);
            };
            for &f in INLAY_FRETS.iter() {
                if g.in_window(f) {
                    inlay(g.fret_axis(f), mid);
                }
            }
            for &f in OCTAVE_FRETS.iter() {
                if g.in_window(f) {
                    inlay(g.fret_axis(f), mid - STRING_SP);
                    inlay(g.fret_axis(f), mid + STRING_SP);
                }
            }
        }

        // Strings: run the length of the neck at each string's cross position,
        // thickness per string (low string thick).
        for i in 0..self.string_count {
            let c = PAD + g.string_cross(i);
            let th = string_thickness(i, self.string_count);
            match orient {
                Orientation::Horizontal => {
                    cx.fill_rect(neck0, c - th / 2.0, neck1 - neck0, th, C_STRING)
                },
                Orientation::Vertical => {
                    cx.fill_rect(c - th / 2.0, neck0, th, neck1 - neck0, C_STRING)
                },
            }
        }

        // Fret wires cross the neck at each drawn cell's far edge; the window's
        // start edge is a thick bright nut (open-anchored) or a plain boundary.
        {
            let mut cross_line = |along: f32, thick: f32, color: ColorF| {
                let a = PAD + along;
                match orient {
                    Orientation::Horizontal => {
                        cx.fill_rect(a - thick / 2.0, cross0, thick, cross1 - cross0, color)
                    },
                    Orientation::Vertical => {
                        cx.fill_rect(cross0, a - thick / 2.0, cross1 - cross0, thick, color)
                    },
                }
            };
            for f in self.fret_start.max(1)..=self.fret_count {
                cross_line(g.wire_axis(f), 1.0, C_WIRE);
            }
            let (nt, nc) = if self.fret_start == 0 {
                (3.0, C_NUT)
            } else {
                (1.0, C_WIRE)
            };
            cross_line(g.open_off(), nt, nc);
        }

        // Touch path trail: a translucent line threaded through the markers in
        // visit order, under the markers so they stay legible.
        if self.show_path && self.path.len() >= 2 {
            let pts: Vec<(f32, f32)> = self
                .path
                .iter()
                .filter(|(s, f)| *s < self.string_count && g.in_window(*f))
                .map(|&(s, f)| g.note_pos(s, f))
                .collect();
            if pts.len() >= 2 {
                cx.stroke_path(
                    sprigging::Path::polyline(&pts),
                    sprigging::round_stroke(C_PATH, 2.5),
                );
            }
        }

        // Note markers: quiet rects, root warmer. Labels are drawn by the CSS
        // overlay that sits over this leaf at the same marker centres. The marker
        // box swaps extents with orientation so it fits its cell either way.
        let (mw, mh) = g.marker_size();
        let radius = mw.min(mh) * 0.56;
        for d in &self.dots {
            if d.string_index >= self.string_count || !g.in_window(d.fret) {
                continue;
            }
            let (x, y) = g.note_pos(d.string_index, d.fret);
            let color = if self.active.contains(&(d.string_index, d.fret)) {
                C_ACTIVE
            } else if d.excluded {
                C_EXCLUDED
            } else if d.is_root {
                C_ROOT
            } else {
                C_NOTE
            };
            let (mx, my) = (x - mw / 2.0, y - mh / 2.0);
            match self.marker_style {
                MarkerStyle::Sharp => cx.fill_rect(mx, my, mw, mh, color),
                MarkerStyle::Rounded => cx.fill_path(rounded_rect(mx, my, mw, mh, 5.0), color),
                MarkerStyle::Circle => {
                    cx.fill_path(sprigging::Path::circle(x, y, radius), color);
                },
                MarkerStyle::Diamond => {
                    let (dx, dy) = (mw * 0.46, mh * 0.66);
                    let path = sprigging::Path::new()
                        .move_to(x, y - dy)
                        .line_to(x + dx, y)
                        .line_to(x, y + dy)
                        .line_to(x - dx, y)
                        .close()
                        .build();
                    cx.fill_path(path, color);
                },
            }
            // An outline ring tracking the marker's silhouette (a halo, not a
            // second marker), inflated by `inflate`.
            let ring_at = |inflate: f32| match self.marker_style {
                MarkerStyle::Circle => sprigging::Path::circle(x, y, radius + inflate),
                MarkerStyle::Diamond => {
                    let (dx, dy) = (mw * 0.46 + inflate, mh * 0.66 + inflate);
                    sprigging::Path::new()
                        .move_to(x, y - dy)
                        .line_to(x + dx, y)
                        .line_to(x, y + dy)
                        .line_to(x - dx, y)
                        .close()
                        .build()
                },
                _ => rounded_rect(
                    mx - inflate,
                    my - inflate,
                    mw + inflate * 2.0,
                    mh + inflate * 2.0,
                    5.0,
                ),
            };
            // Colorblind aid: a neutral outline on the root, so it is identifiable
            // by shape, not by its warm colour alone. The mark ring takes over
            // when a note is also marked.
            if self.distinguish_root && d.is_root && !d.marked {
                cx.stroke_path(ring_at(2.0), sprigging::round_stroke(C_ROOT_RING, 2.0));
            }
            // Selection ring for a marked note.
            if d.marked {
                cx.stroke_path(ring_at(3.0), sprigging::round_stroke(C_MARK, 2.0));
            }
        }

        self.dirty = false;
    }

    fn paint_dirty(&self) -> bool {
        self.dirty
    }

    /// The painted neck is an opaque graphic to the DOM, so it speaks for itself:
    /// it announces as a graphics object with a structural summary. The specific
    /// board's identity (material, note count) rides as the author's `aria-label`
    /// on the enclosing region and outranks this fallback; the markers over the
    /// leaf are their own named buttons, so a screen reader navigates the notes
    /// while this describes the surface they sit on.
    fn accessibility(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::GraphicsObject);
        if node.label().is_none() {
            let orientation = match self.orientation {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            };
            node.set_label(format!(
                "{orientation} fretboard, {} strings, frets {}\u{2013}{}, {} notes",
                self.string_count,
                self.fret_start,
                self.fret_count,
                self.dots.len(),
            ));
        }
    }
}
