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
mod tests;

mod leaf;
