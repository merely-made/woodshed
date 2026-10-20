//! Bounded chart, tablature and staff notation for Sprigging hosts.
//!
//! Musical outlines are Bravura (SMuFL), not approximated clefs or Unicode
//! fallback text. Ordinary labels are exposed for the host's DOM text overlay.
//! Geometry is authored once and shared with the host's event controls.
//!
//! The default `accessibility` feature supplies fallback leaf labels and a
//! playback description for native consumers. Hosts that supply accessible
//! event controls and their own leaf label can disable default features; the
//! same paint, layout, labels and event rectangles remain available.
pub use sprigging::ColorF;
use sprigging::{Leaf, PaintCx, Path, Size, SizeHint};
use woodshedding::notation::{NoteValue, Score};

pub const NOTATION_LEAF_KEY: u64 = 0x5753_4e54;
const FONT: &[u8] = include_bytes!("../assets/bravura/Bravura.otf");
const GAP: f32 = 10.0;
const PAD: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotationView {
    Chart,
    Tab,
    Score,
}
impl NotationView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chart => "Chart",
            Self::Tab => "Tab",
            Self::Score => "Score",
        }
    }
}

/// Top-left coordinates of plain text, rendered by the host in its usual font.
#[derive(Clone, Debug)]
pub struct NotationLabel {
    pub text: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug)]
struct Bar {
    index: usize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    stave_top: f32,
}

pub struct NotationLeaf {
    score: Score,
    view: NotationView,
    selected: (usize, usize),
    playhead: Option<(usize, usize)>,
    size: Size,
    bars: Vec<Bar>,
    ink: ColorF,
    accent: ColorF,
    dirty: bool,
}

impl NotationLeaf {
    pub fn new(
        score: Score,
        view: NotationView,
        selected: (usize, usize),
        width: f32,
        ink: ColorF,
        accent: ColorF,
    ) -> Self {
        let width = if width.is_finite() {
            width.max(240.0)
        } else {
            640.0
        };
        let usable = width - 2.0 * PAD;
        let (_, base, transpose) = clef_profile(&score.instrument);
        let mut bars = Vec::new();
        let (mut x, mut y) = (PAD, PAD);
        let mut row_start = 0;
        let align_row = |bars: &mut [Bar]| {
            let top = bars.iter().map(|b| b.stave_top).fold(0.0_f32, f32::max);
            let below = bars
                .iter()
                .map(|b| b.height - b.stave_top)
                .fold(0.0_f32, f32::max);
            for bar in bars.iter_mut() {
                bar.stave_top = top;
                bar.height = top + below;
            }
            top + below
        };
        for (index, measure) in score.measures.iter().enumerate() {
            let desired = (84.0 + measure.events.len().max(4) as f32 * 38.0).min(usable);
            let mut highest = base + 8;
            let mut lowest = base;
            for note in measure.events.iter().flat_map(|event| &event.notes) {
                if let Some(open) = score.tuning.get(usize::from(note.string.saturating_sub(1))) {
                    let pitch = staff_pitch(i32::from(*open) + i32::from(note.fret) + transpose).0;
                    highest = highest.max(pitch);
                    lowest = lowest.min(pitch);
                }
            }
            let stave_top = if view == NotationView::Score {
                94.0 + (highest - base - 8) as f32 * 5.0
            } else {
                52.0
            };
            let height = match view {
                NotationView::Chart => 124.0,
                NotationView::Tab => 142.0 + score.tuning.len().saturating_sub(1) as f32 * 14.0,
                NotationView::Score => stave_top + 76.0 + (base - lowest) as f32 * 5.0,
            };
            if x > PAD && x + desired > width - PAD + 0.1 {
                x = PAD;
                y += align_row(&mut bars[row_start..]) + 16.0;
                row_start = bars.len();
            }
            bars.push(Bar {
                index,
                x,
                y,
                width: desired,
                height,
                stave_top,
            });
            x += desired;
        }
        let row_height = align_row(&mut bars[row_start..]);
        Self {
            score,
            view,
            selected,
            playhead: None,
            size: Size {
                width,
                height: (y + row_height + PAD).max(100.0),
            },
            bars,
            ink,
            accent,
            dirty: true,
        }
    }

    /// Mark the current playback event independently of editing selection.
    /// `None` clears the cursor; positions outside this score are ignored.
    /// This is a paint/a11y projection and does not choose transport policy.
    pub fn with_playhead(mut self, playhead: Option<(usize, usize)>) -> Self {
        let playhead = playhead.filter(|&(bar, event)| {
            self.score
                .measures
                .get(bar)
                .is_some_and(|m| event < m.events.len())
        });
        self.dirty |= self.playhead != playhead;
        self.playhead = playhead;
        self
    }

    pub fn intrinsic_size(&self) -> Size {
        self.size
    }

    /// One stable hit rectangle for every authored event, independent of view.
    pub fn event_rects(&self) -> Vec<(usize, usize, (f32, f32, f32, f32))> {
        self.bars
            .iter()
            .flat_map(|bar| {
                let n = self.score.measures[bar.index].events.len();
                let step = (bar.width - 70.0) / n.max(1) as f32;
                (0..n).map(move |event| {
                    (
                        bar.index,
                        event,
                        (
                            bar.x + 62.0 + event as f32 * step,
                            bar.y + 30.0,
                            step,
                            bar.height - 38.0,
                        ),
                    )
                })
            })
            .collect()
    }

    pub fn labels(&self) -> Vec<NotationLabel> {
        let mut labels = Vec::new();
        for bar in &self.bars {
            labels.push(NotationLabel {
                text: format!("{}", bar.index + 1),
                x: bar.x + 4.0,
                y: bar.y,
            });
            if let Some(chord) = &self.score.measures[bar.index].harmony {
                labels.push(NotationLabel {
                    text: chord.label(),
                    x: bar.x + 62.0,
                    y: bar.y + 2.0,
                });
            }
        }
        labels
    }

    fn glyph(
        &self,
        cx: &mut PaintCx<'_>,
        face: &ttf_parser::Face<'_>,
        code: u32,
        x: f32,
        y: f32,
        staff_space: f32,
        color: ColorF,
    ) {
        let Some(glyph) = char::from_u32(code).and_then(|c| face.glyph_index(c)) else {
            return;
        };
        // SMuFL's em contains four staff spaces.
        let scale = staff_space * 4.0 / face.units_per_em() as f32;
        let mut outline = Outline {
            path: Path::new(),
            x,
            y,
            scale,
        };
        if face.outline_glyph(glyph, &mut outline).is_some() {
            cx.fill_path(outline.path.build(), color);
        }
    }

    fn digits(
        &self,
        cx: &mut PaintCx<'_>,
        face: &ttf_parser::Face<'_>,
        number: u8,
        x: f32,
        y: f32,
        scale: f32,
        color: ColorF,
    ) {
        let text = number.to_string();
        for (i, digit) in text.bytes().enumerate() {
            self.glyph(
                cx,
                face,
                0xe080 + (digit - b'0') as u32,
                x + i as f32 * scale * 0.85,
                y,
                scale,
                color,
            );
        }
    }

    fn fret_digits(
        &self,
        cx: &mut PaintCx<'_>,
        face: &ttf_parser::Face<'_>,
        number: u8,
        x: f32,
        y: f32,
        color: ColorF,
    ) {
        let (scale, advance) = tab_digit_metrics(face);
        for (index, digit) in number.to_string().bytes().enumerate() {
            let code = 0xe080 + u32::from(digit - b'0');
            let bounds = char::from_u32(code)
                .and_then(|c| face.glyph_index(c))
                .and_then(|g| face.glyph_bounding_box(g));
            if let Some(bounds) = bounds {
                let baseline =
                    y + (f32::from(bounds.y_max) + f32::from(bounds.y_min)) * scale / 2.0;
                self.glyph(
                    cx,
                    face,
                    code,
                    x + index as f32 * advance,
                    baseline,
                    scale * f32::from(face.units_per_em()) / 4.0,
                    color,
                );
            }
        }
    }

    fn rhythm(
        &self,
        cx: &mut PaintCx<'_>,
        face: &ttf_parser::Face<'_>,
        value: NoteValue,
        rest: bool,
        x: f32,
        y: f32,
        color: ColorF,
    ) {
        if rest {
            let code = match value {
                NoteValue::Whole => 0xe4e3,
                NoteValue::Half => 0xe4e4,
                NoteValue::Quarter => 0xe4e5,
                _ => 0xe4e6,
            };
            self.glyph(cx, face, code, x, y, GAP, color);
        } else {
            let head = match value {
                NoteValue::Whole => 0xe0a2,
                NoteValue::Half => 0xe0a3,
                _ => 0xe0a4,
            };
            self.glyph(cx, face, head, x, y, GAP, color);
            if value != NoteValue::Whole {
                cx.fill_rect(x + 11.0, y - 32.0, 1.2, 32.0, color);
                if matches!(value, NoteValue::Eighth | NoteValue::TripletEighth) {
                    self.glyph(cx, face, 0xe240, x + 11.0, y - 32.0, GAP, color);
                }
            }
        }
    }
}

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
    fn accessibility(&mut self, node: &mut accesskit::Node) {
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

// Fret digits must fit between strings, independently of the font's
// time-signature em metrics. Center their actual outlines on each string.
fn tab_digit_metrics(face: &ttf_parser::Face<'_>) -> (f32, f32) {
    let mut height = 1.0_f32;
    let mut width = 1.0_f32;
    for code in 0xe080..=0xe089 {
        if let Some(bounds) = char::from_u32(code)
            .and_then(|c| face.glyph_index(c))
            .and_then(|g| face.glyph_bounding_box(g))
        {
            height = height.max(f32::from(bounds.y_max) - f32::from(bounds.y_min));
            width = width.max(f32::from(bounds.x_max) - f32::from(bounds.x_min));
        }
    }
    let scale = 11.0 / height;
    (scale, width * scale + 1.5)
}

// Same instrument conventions as the MusicXML writer. Tuple values are the
// SMuFL clef glyph, diatonic index of the bottom staff line and written octave
// transposition above sounding pitch. Reentrant tuning order is unrelated.
fn clef_profile(instrument: &str) -> (u32, i32, i32) {
    let name = instrument.to_ascii_lowercase();
    if name.contains("bass") {
        (0xe062, 18, 12)
    } else if name == "cello" {
        (0xe062, 18, 0)
    } else if name == "viola" {
        (0xe05c, 24, 0)
    } else if name.contains("guitar") {
        (0xe050, 30, 12)
    } else {
        (0xe050, 30, 0)
    }
}

/// Same chromatic spelling as MusicXML: C♯, E♭, F♯, A♭, B♭.
/// This prototype has no key-signature engraver.
fn staff_pitch(midi: i32) -> (i32, i8) {
    const STEPS: [i32; 12] = [0, 0, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6];
    let pc = midi.rem_euclid(12) as usize;
    (
        midi.div_euclid(12) * 7 - 7 + STEPS[pc],
        match pc {
            1 | 6 => 1,
            3 | 8 | 10 => -1,
            _ => 0,
        },
    )
}

struct Outline {
    path: Path,
    x: f32,
    y: f32,
    scale: f32,
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path = std::mem::take(&mut self.path)
            .move_to(self.x + x * self.scale, self.y - y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path = std::mem::take(&mut self.path)
            .line_to(self.x + x * self.scale, self.y - y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path = std::mem::take(&mut self.path).quad_to(
            self.x + x1 * self.scale,
            self.y - y1 * self.scale,
            self.x + x * self.scale,
            self.y - y * self.scale,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path = std::mem::take(&mut self.path).cubic_to(
            self.x + x1 * self.scale,
            self.y - y1 * self.scale,
            self.x + x2 * self.scale,
            self.y - y2 * self.scale,
            self.x + x * self.scale,
            self.y - y * self.scale,
        );
    }
    fn close(&mut self) {
        self.path = std::mem::take(&mut self.path).close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn leaf(width: f32, view: NotationView) -> NotationLeaf {
        NotationLeaf::new(
            Score::progression_example(),
            view,
            (0, 0),
            width,
            ColorF {
                r: 0.9,
                g: 0.9,
                b: 0.9,
                a: 1.0,
            },
            ColorF {
                r: 0.4,
                g: 0.7,
                b: 1.0,
                a: 1.0,
            },
        )
    }
    #[test]
    fn actual_tab_digit_outlines_fit_between_adjacent_strings() {
        let face = ttf_parser::Face::parse(FONT, 0).unwrap();
        let (scale, advance) = tab_digit_metrics(&face);
        for code in 0xe080..=0xe089 {
            let glyph = face.glyph_index(char::from_u32(code).unwrap()).unwrap();
            let bounds = face.glyph_bounding_box(glyph).unwrap();
            let height = (f32::from(bounds.y_max) - f32::from(bounds.y_min)) * scale;
            assert!(
                height < 14.0,
                "fret digits must leave space between 14px string lines"
            );
            assert!((f32::from(bounds.x_max) - f32::from(bounds.x_min)) * scale < advance);
        }
    }

    #[test]
    fn complete_measures_wrap_and_event_hitboxes_stay_inside() {
        for width in [320.0, 640.0, 1000.0] {
            for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
                let leaf = leaf(width, view);
                for (_, _, (x, y, w, h)) in leaf.event_rects() {
                    assert!(w > 0.0 && h > 0.0);
                    assert!(x >= 0.0 && x + w <= leaf.size.width);
                    assert!(y >= 0.0 && y + h <= leaf.size.height);
                }
                for bar in &leaf.bars {
                    assert!(bar.x + bar.width <= width);
                    assert!(bar.y + bar.height <= leaf.size.height);
                }
            }
        }
        assert!(
            leaf(320.0, NotationView::Score).size.height
                > leaf(1000.0, NotationView::Score).size.height
        );
    }
    #[test]
    fn each_view_emits_native_paint_commands() {
        for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
            for score in [Score::progression_example(), Score::rhythm_example()] {
                let mut leaf = NotationLeaf::new(
                    score,
                    view,
                    (0, 0),
                    320.0,
                    ColorF {
                        r: 0.9,
                        g: 0.9,
                        b: 0.9,
                        a: 1.0,
                    },
                    ColorF {
                        r: 0.4,
                        g: 0.7,
                        b: 1.0,
                        a: 1.0,
                    },
                );
                assert!(leaf.paint_dirty());
                let mut cmds = Vec::new();
                let size = leaf.size;
                leaf.paint(&mut PaintCx::new(&mut cmds, size));
                assert!(!leaf.paint_dirty());
                assert!(
                    cmds.len() > 10,
                    "{view:?} emits staves and real glyph outlines"
                );
            }
        }
    }
    #[test]
    fn score_measures_share_staff_lines_within_each_wrapped_row() {
        let score = Score::rhythm_example();
        let wide = NotationLeaf::new(
            score.clone(),
            NotationView::Score,
            (0, 0),
            1000.0,
            leaf(320.0, NotationView::Score).ink,
            leaf(320.0, NotationView::Score).accent,
        );
        assert_eq!(wide.bars[0].y, wide.bars[1].y);
        assert_eq!(wide.bars[0].stave_top, wide.bars[1].stave_top);
        assert_eq!(wide.bars[0].height, wide.bars[1].height);
        let narrow = NotationLeaf::new(
            score,
            NotationView::Score,
            (0, 0),
            320.0,
            wide.ink,
            wide.accent,
        );
        assert!(narrow.bars[1].y >= narrow.bars[0].y + narrow.bars[0].height);
    }
    #[test]
    fn playback_cursor_is_separate_from_selection_in_all_views_and_rests() {
        for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
            for rest in [false, true] {
                let mut base = leaf(640.0, view);
                if rest {
                    let event = &mut base.score.measures[1].events[0];
                    event.notes.clear();
                    event.rest = true;
                }
                let authored = base.score.clone();
                let rects = base.event_rects();
                let mut baseline = Vec::new();
                let size = base.size;
                base.paint(&mut PaintCx::new(&mut baseline, size));
                assert!(!base.paint_dirty());
                let mut playing = base.with_playhead(Some((1, 0)));
                assert!(playing.paint_dirty());
                assert_eq!(playing.selected, (0, 0));
                assert_eq!(playing.score, authored);
                assert_eq!(playing.event_rects(), rects);
                let mut commands = Vec::new();
                playing.paint(&mut PaintCx::new(&mut commands, size));
                assert_eq!(commands.len(), baseline.len() + 3);
                assert!(matches!(&commands[0], sprigging::PaintCmd::DrawRect(rect)
                    if rect.color.a == playing.accent.a * 0.12));
                // Existing musical glyphs and their authoring-selection colors
                // remain identical. Playback adds only its background geometry.
                assert_eq!(
                    serde_json::to_value(&commands[3..]).unwrap(),
                    serde_json::to_value(&baseline).unwrap()
                );
                #[cfg(feature = "accessibility")]
                let mut node = accesskit::Node::new(accesskit::Role::GenericContainer);
                #[cfg(feature = "accessibility")]
                {
                    node.set_label("Authored score label");
                    playing.accessibility(&mut node);
                    assert_eq!(node.label(), Some("Authored score label"));
                    assert_eq!(
                        node.state_description(),
                        Some(if rest {
                            "Playback: bar 2, event 1, rest"
                        } else {
                            "Playback: bar 2, event 1, sounding notes"
                        })
                    );
                }
                let mut stopped = playing.with_playhead(None);
                assert!(stopped.paint_dirty());
                #[cfg(feature = "accessibility")]
                {
                    stopped.accessibility(&mut node);
                    assert!(node.state_description().is_none());
                }
                let mut cleared = Vec::new();
                stopped.paint(&mut PaintCx::new(&mut cleared, size));
                assert_eq!(
                    serde_json::to_value(&cleared).unwrap(),
                    serde_json::to_value(&baseline).unwrap()
                );
            }
        }
    }

    #[test]
    fn invalid_playback_positions_are_ignored_and_default_has_no_cursor() {
        let mut leaf = leaf(640.0, NotationView::Score);
        assert_eq!(leaf.playhead, None);
        let mut commands = Vec::new();
        let size = leaf.size;
        leaf.paint(&mut PaintCx::new(&mut commands, size));
        let leaf = leaf.with_playhead(Some((usize::MAX, 0)));
        assert_eq!(leaf.playhead, None);
        assert!(!leaf.paint_dirty());
        let leaf = leaf.with_playhead(Some((0, usize::MAX)));
        assert_eq!(leaf.playhead, None);
        assert_eq!(leaf.selected, (0, 0));
    }

    #[test]
    fn chord_chart_uses_authored_meter_beat_count() {
        let mut counts = Vec::new();
        for (beats, beat_type, value) in [
            (3, 4, NoteValue::Quarter),
            (4, 4, NoteValue::Quarter),
            (6, 8, NoteValue::Eighth),
        ] {
            let mut leaf = leaf(320.0, NotationView::Chart);
            leaf.score.measures.truncate(1);
            leaf.bars.truncate(1);
            let measure = &mut leaf.score.measures[0];
            measure.meter = woodshedding::notation::TimeSignature { beats, beat_type };
            let mut event = measure.events[0].clone();
            event.value = value;
            measure.events = vec![event; usize::from(beats)];
            let mut cmds = Vec::new();
            let size = leaf.size;
            leaf.paint(&mut PaintCx::new(&mut cmds, size));
            counts.push(cmds.len());
        }
        assert_eq!(counts[1], counts[0] + 1);
        assert_eq!(counts[2], counts[0] + 3);
    }

    #[test]
    fn bundled_font_has_real_music_outlines() {
        let face = ttf_parser::Face::parse(FONT, 0).unwrap();
        for code in [
            0xe050, 0xe062, 0xe05c, 0xe0a2, 0xe0a3, 0xe0a4, 0xe101, 0xe240, 0xe260, 0xe262, 0xe4e3,
            0xe4e4, 0xe4e5, 0xe4e6, 0xe080, 0xe089,
        ] {
            let glyph = face
                .glyph_index(char::from_u32(code).unwrap())
                .expect("SMuFL glyph");
            let mut outline = Outline {
                path: Path::new(),
                x: 0.0,
                y: 0.0,
                scale: 1.0,
            };
            assert!(face.outline_glyph(glyph, &mut outline).is_some());
        }
    }
    #[test]
    fn instrument_clefs_and_written_octaves_match_conventional_staves() {
        for (name, profile, reference_midi, reference_line) in [
            ("Guitar", (0xe050, 30, 12), 55, 32), // G3 sounds as written G4
            ("Bass", (0xe062, 18, 12), 41, 24),   // F2 sounds as written F3
            ("Double Bass", (0xe062, 18, 12), 41, 24),
            ("Viola", (0xe05c, 24, 0), 60, 28), // C4 on alto middle line
            ("Cello", (0xe062, 18, 0), 53, 24), // F3 on bass fourth line
            ("Violin", (0xe050, 30, 0), 67, 32), // G4 on treble second line
            ("Ukulele", (0xe050, 30, 0), 67, 32),
        ] {
            assert_eq!(clef_profile(name), profile, "{name}");
            assert_eq!(
                staff_pitch(reference_midi + profile.2).0,
                reference_line,
                "{name}"
            );
        }
    }

    #[test]
    fn arbitrary_string_count_and_extreme_ledger_space_are_measured() {
        use woodshedding::notation::{FrettedNote, Measure, NoteEvent, TimeSignature};
        let score = Score {
            title: "Range".into(),
            instrument: "Extended guitar".into(),
            tuning: vec![100, 90, 80, 70, 60, 50, 40, 30],
            tempo: 100,
            measures: vec![Measure {
                meter: TimeSignature {
                    beats: 4,
                    beat_type: 4,
                },
                harmony: None,
                events: vec![NoteEvent {
                    notes: vec![
                        FrettedNote {
                            string: 1,
                            fret: 24,
                        },
                        FrettedNote { string: 8, fret: 0 },
                    ],
                    value: NoteValue::Whole,
                    rest: false,
                }],
            }],
        };
        let tab = NotationLeaf::new(
            score.clone(),
            NotationView::Tab,
            (0, 0),
            320.0,
            ColorF {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            ColorF {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        );
        assert_eq!(tab.score.tuning.len(), 8);
        let staff = NotationLeaf::new(
            score,
            NotationView::Score,
            (0, 0),
            320.0,
            tab.ink,
            tab.accent,
        );
        assert!(staff.bars[0].stave_top > 94.0);
        assert!(staff.bars[0].height > 220.0);
        assert_eq!(staff.event_rects().len(), 1);
        assert_eq!(clef_profile("Guitar"), (0xe050, 30, 12));
        assert_eq!(clef_profile("Bass"), (0xe062, 18, 12));
        assert_eq!(clef_profile("Ukulele"), (0xe050, 30, 0));
        assert_eq!(staff_pitch(64), (30, 0));
        assert_eq!(staff_pitch(65), (31, 0));
        assert_eq!(staff_pitch(66), (31, 1));
        assert_eq!(staff_pitch(63), (30, -1)); // E-flat4
        assert_eq!(staff_pitch(68), (33, -1)); // A-flat4
        assert_eq!(staff_pitch(70), (34, -1)); // B-flat4
    }
}
