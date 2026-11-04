use super::*;

impl Leaf for NotationLeaf {
    fn paint_dirty(&self) -> bool {
        self.dirty
    }
    fn measure(&mut self, _known: SizeHint, _available: SizeHint) -> Size {
        self.size
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let Ok(face) = ttf_parser::Face::parse(FONT, 0) else {
            return;
        };
        // A shape distinct from authoring's accented glyphs: one translucent
        // column with a solid leading and bottom edge, including silent rests.
        if let Some(playhead) = self.playhead {
            if let Some((_, _, (x, y, width, height))) = self
                .event_rects()
                .into_iter()
                .find(|(bar, event, _)| (*bar, *event) == playhead)
            {
                let wash = ColorF {
                    a: self.accent.a * 0.12,
                    ..self.accent
                };
                cx.fill_rect(x, y, width, height, wash);
                cx.fill_rect(x, y, 2.0, height, self.accent);
                cx.fill_rect(x, y + height - 2.0, width, 2.0, self.accent);
            }
        }
        let (clef, base, transpose) = clef_profile(&self.score.instrument);
        for bar in &self.bars {
            let measure = &self.score.measures[bar.index];
            let top = bar.y + bar.stave_top;
            let lines = match self.view {
                NotationView::Chart => 1,
                NotationView::Tab => self.score.tuning.len().max(1),
                NotationView::Score => 5,
            };
            let spacing = if self.view == NotationView::Tab {
                14.0
            } else {
                GAP
            };
            let bottom = top + lines.saturating_sub(1) as f32 * spacing;
            for line in 0..lines {
                cx.fill_rect(bar.x, top + line as f32 * spacing, bar.width, 0.8, self.ink);
            }
            cx.fill_rect(bar.x, top, 1.0, (bottom - top).max(16.0), self.ink);
            cx.fill_rect(
                bar.x + bar.width - 1.0,
                top,
                1.0,
                (bottom - top).max(16.0),
                self.ink,
            );
            if self.view == NotationView::Score {
                self.glyph(
                    cx,
                    &face,
                    clef,
                    bar.x + 4.0,
                    top + match clef {
                        0xe062 => 10.0,
                        0xe05c => 20.0,
                        _ => 30.0,
                    },
                    GAP,
                    self.ink,
                );
                // Guitar and bass conventionally sound an octave below their stave.
                if transpose != 0 {
                    self.digits(cx, &face, 8, bar.x + 12.0, top + 54.0, 5.0, self.ink);
                }
            }
            self.digits(
                cx,
                &face,
                measure.meter.beats,
                bar.x + 38.0,
                top + 14.0,
                7.0,
                self.ink,
            );
            self.digits(
                cx,
                &face,
                measure.meter.beat_type,
                bar.x + 38.0,
                top + 30.0,
                7.0,
                self.ink,
            );
            if self.view == NotationView::Chart
                && measure.harmony.is_some()
                && measure.events.first().is_some_and(|first| {
                    first.value.units() % (24 / u32::from(measure.meter.beat_type).max(1)).max(1)
                        == 0
                        && measure.events.iter().all(|event| {
                            event.notes == first.notes
                                && event.rest == first.rest
                                && event.value == first.value
                        })
                })
            {
                let beats = u32::from(measure.meter.beats).max(1);
                let unit = 24 / u32::from(measure.meter.beat_type).max(1);
                let beat_value = match measure.meter.beat_type {
                    1 => NoteValue::Whole,
                    2 => NoteValue::Half,
                    8 => NoteValue::Eighth,
                    _ => NoteValue::Quarter,
                };
                for beat in 0..beats {
                    let x = bar.x + 64.0 + beat as f32 * (bar.width - 70.0) / beats as f32;
                    let mut start = 0;
                    let active = measure.events.iter().enumerate().find(|(_, event)| {
                        let end = start + event.value.units();
                        let contains = beat * unit >= start && beat * unit < end;
                        start = end;
                        contains
                    });
                    let color =
                        if active.is_some_and(|(event, _)| self.selected == (bar.index, event)) {
                            self.accent
                        } else {
                            self.ink
                        };
                    if active.is_some_and(|(_, event)| event.rest) {
                        self.rhythm(cx, &face, beat_value, true, x, top, color);
                    } else {
                        self.glyph(cx, &face, 0xe101, x, top, GAP, color);
                    }
                }
                continue;
            }
            let step = (bar.width - 70.0) / measure.events.len().max(1) as f32;
            for (ei, event) in measure.events.iter().enumerate() {
                let x = bar.x + 64.0 + ei as f32 * step;
                let color = if self.selected == (bar.index, ei) {
                    self.accent
                } else {
                    self.ink
                };
                match self.view {
                    NotationView::Chart => {
                        self.rhythm(cx, &face, event.value, event.rest, x, top, color);
                    },
                    NotationView::Tab => {
                        if event.rest {
                            self.rhythm(cx, &face, event.value, true, x, bottom + 64.0, color);
                        } else {
                            for note in &event.notes {
                                let string = usize::from(note.string.saturating_sub(1));
                                if string < self.score.tuning.len() {
                                    let y = top + string as f32 * spacing;
                                    self.fret_digits(cx, &face, note.fret, x, y, color);
                                }
                            }
                            self.rhythm(cx, &face, event.value, false, x, bottom + 64.0, color);
                        }
                    },
                    NotationView::Score => {
                        if event.rest {
                            self.rhythm(cx, &face, event.value, true, x, top + 20.0, color);
                        } else {
                            let mut ys = Vec::new();
                            for note in &event.notes {
                                if let Some(open) = self
                                    .score
                                    .tuning
                                    .get(usize::from(note.string.saturating_sub(1)))
                                {
                                    let midi = i32::from(*open) + i32::from(note.fret) + transpose;
                                    let (diatonic, accidental) = staff_pitch(midi);
                                    // Bottom stave line is written E4 (diatonic index 30).
                                    let y = bottom - (diatonic - base) as f32 * (GAP / 2.0);
                                    ys.push(y);
                                    if accidental != 0 {
                                        self.glyph(
                                            cx,
                                            &face,
                                            if accidental < 0 { 0xe260 } else { 0xe262 },
                                            x - 12.0,
                                            y,
                                            GAP,
                                            color,
                                        );
                                    }
                                    let head = match event.value {
                                        NoteValue::Whole => 0xe0a2,
                                        NoteValue::Half => 0xe0a3,
                                        _ => 0xe0a4,
                                    };
                                    self.glyph(cx, &face, head, x, y, GAP, color);
                                    let mut ledger = bottom + GAP;
                                    while ledger <= y + 0.1 {
                                        cx.fill_rect(x - 3.0, ledger, 19.0, 0.9, color);
                                        ledger += GAP;
                                    }
                                    ledger = top - GAP;
                                    while ledger >= y - 0.1 {
                                        cx.fill_rect(x - 3.0, ledger, 19.0, 0.9, color);
                                        ledger -= GAP;
                                    }
                                }
                            }
                            if event.value != NoteValue::Whole && !ys.is_empty() {
                                let highest = ys.iter().copied().fold(f32::INFINITY, f32::min);
                                let lowest = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                                cx.fill_rect(
                                    x + 11.0,
                                    highest - 32.0,
                                    1.2,
                                    lowest - highest + 32.0,
                                    color,
                                );
                                if matches!(
                                    event.value,
                                    NoteValue::Eighth | NoteValue::TripletEighth
                                ) {
                                    self.glyph(
                                        cx,
                                        &face,
                                        0xe240,
                                        x + 11.0,
                                        highest - 32.0,
                                        GAP,
                                        color,
                                    );
                                }
                            }
                        }
                    },
                }
            }
            let mut index = 0;
            while index + 2 < measure.events.len() {
                let group = &measure.events[index..index + 3];
                if !group
                    .iter()
                    .all(|event| event.value == NoteValue::TripletEighth)
                {
                    index += 1;
                    continue;
                }
                let baseline = match self.view {
                    NotationView::Chart => top,
                    NotationView::Tab => bottom + 64.0,
                    NotationView::Score => group
                        .iter()
                        .flat_map(|event| &event.notes)
                        .filter_map(|note| {
                            self.score
                                .tuning
                                .get(usize::from(note.string.saturating_sub(1)))
                                .map(|open| {
                                    bottom
                                        - (staff_pitch(
                                            i32::from(*open) + i32::from(note.fret) + transpose,
                                        )
                                        .0 - base) as f32
                                            * GAP
                                            / 2.0
                                })
                        })
                        .fold(top + 20.0, f32::min),
                };
                let start = bar.x + 64.0 + index as f32 * step;
                let end = start + 2.0 * step + 11.0;
                let y = baseline - 48.0;
                let center = (start + end) / 2.0;
                cx.fill_rect(start, y, (center - start - 6.0).max(0.0), 1.0, self.ink);
                cx.fill_rect(
                    center + 6.0,
                    y,
                    (end - center - 6.0).max(0.0),
                    1.0,
                    self.ink,
                );
                cx.fill_rect(start, y, 1.0, 5.0, self.ink);
                cx.fill_rect(end, y, 1.0, 5.0, self.ink);
                self.digits(cx, &face, 3, center - 3.0, y + 3.0, 6.0, self.ink);
                index += 3;
            }
        }
        self.dirty = false;
    }
    #[cfg(feature = "accessibility")]
    fn accessibility(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::GraphicsObject);
        if let Some((bar, event)) = self.playhead {
            let kind = if self.score.measures[bar].events[event].rest {
                "rest"
            } else {
                "sounding notes"
            };
            node.set_state_description(format!(
                "Playback: bar {}, event {}, {kind}",
                bar + 1,
                event + 1
            ));
        } else {
            node.clear_state_description();
        }
        if node.label().is_none() {
            node.set_label(format!(
                "{}: {:?}, {} measures",
                self.score.title,
                self.view,
                self.score.measures.len()
            ));
        }
    }
}
