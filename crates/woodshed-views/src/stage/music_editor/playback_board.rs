//! Read-only playback context. Contacts come from all events in the requested
//! bars; the current event only changes their emphasis, never the context.
use super::{Score, UiChild, UiState, music_width, note_name};
use cambium::{el, text};
use std::{collections::BTreeSet, ops::RangeInclusive};

type Contact = (u8, u8);
#[derive(Debug, PartialEq, Eq)]
enum Current {
    None,
    OutsideScope,
    Rest { bar: usize, event: usize },
    Notes { bar: usize, event: usize },
}
#[derive(Debug, PartialEq, Eq)]
struct Context {
    scope: Option<(usize, usize)>,
    contacts: BTreeSet<Contact>,
    sounding: BTreeSet<Contact>,
    current: Current,
}
fn valid_contact(score: &Score, note: &woodshedding::notation::FrettedNote) -> bool {
    note.fret <= 36 && score.pitch(*note).is_ok()
}
fn context(
    score: &Score,
    playhead: Option<(usize, usize)>,
    bars: RangeInclusive<usize>,
) -> Context {
    let (first, last) = bars.into_inner();
    let scope = (first <= last && first < score.measures.len())
        .then(|| (first, last.min(score.measures.len() - 1)));
    let mut result = Context {
        scope,
        contacts: BTreeSet::new(),
        sounding: BTreeSet::new(),
        current: Current::None,
    };
    if let Some((first, last)) = scope {
        for event in score.measures[first..=last]
            .iter()
            .flat_map(|bar| &bar.events)
            .filter(|e| !e.rest)
        {
            for note in event.notes.iter().filter(|note| valid_contact(score, note)) {
                result.contacts.insert((note.string, note.fret));
            }
        }
    }
    if let Some((bar, event)) = playhead {
        if let Some(event_data) = score.measures.get(bar).and_then(|m| m.events.get(event)) {
            if !scope.is_some_and(|(first, last)| (first..=last).contains(&bar)) {
                result.current = Current::OutsideScope;
            } else if event_data.rest {
                result.current = Current::Rest { bar, event };
            } else {
                result.current = Current::Notes { bar, event };
                result.sounding.extend(
                    event_data
                        .notes
                        .iter()
                        .filter(|note| valid_contact(score, note))
                        .map(|note| (note.string, note.fret)),
                );
            }
        }
    }
    result
}
fn fret_range(contacts: &BTreeSet<Contact>) -> (u8, u8) {
    let Some(minimum) = contacts.iter().map(|(_, f)| *f).min() else {
        return (0, 8);
    };
    let maximum = contacts.iter().map(|(_, f)| *f).max().unwrap().min(36);
    if maximum - minimum > 8 {
        (0, maximum)
    } else {
        let first = if maximum > 8 {
            minimum.saturating_sub(1)
        } else {
            0
        };
        (first, (first + 8).max(maximum).min(36))
    }
}
fn fret_sections(first: u8, last: u8, width: f32) -> Vec<(u8, u8)> {
    // Keep labels readable at narrow widths by wrapping neck segments vertically.
    let columns = ((width - 48.0) / 44.0).floor().max(1.0) as u8;
    let mut sections = Vec::new();
    let mut start = first;
    while start <= last {
        let end = start.saturating_add(columns - 1).min(last);
        sections.push((start, end));
        if end == last {
            break;
        }
        start = end + 1;
    }
    sections
}

/// Display the contact union for the chosen zero-based inclusive bar range.
/// Playback emphasis is independent of authoring selection and has no inputs.
pub(super) fn board(
    ui: &UiState,
    score: &Score,
    playhead: Option<(usize, usize)>,
    bars: RangeInclusive<usize>,
) -> UiChild {
    let ctx = context(score, playhead, bars);
    let width = music_width(ui);
    let palette = ui.appearance.resolve(&ui.app_settings.appearance).palette;
    let css = |c: tinct::Srgb| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    let ink = css(palette.text);
    let dim = css(palette.text_dim);
    let ground = css(palette.surface);
    let primary = css(palette.primary);
    let on_primary = css(palette.on_primary);
    let scope = match ctx.scope {
        Some((first, last)) if first == last => format!("Bar {}", first + 1),
        Some((first, last)) => format!("Bars {}–{}", first + 1, last + 1),
        None => "No bars in this range".into(),
    };
    let names = ctx
        .sounding
        .iter()
        .filter_map(|(string, fret)| {
            score
                .pitch(woodshedding::notation::FrettedNote {
                    string: *string,
                    fret: *fret,
                })
                .ok()
        })
        .map(note_name)
        .collect::<Vec<_>>()
        .join(" · ");
    let current = match ctx.current {
        Current::None => "No current playback event.".into(),
        Current::OutsideScope => "Current playback event is outside this bar range.".into(),
        Current::Rest { bar, event: _ } => format!("Bar {} · Rest — no notes sounding", bar + 1),
        Current::Notes { bar, event: _ } => {
            format!("Bar {} · Sounding: {names}", bar + 1)
        },
    };
    let (first, last) = fret_range(&ctx.contacts);
    let mut children: Vec<UiChild> = vec![
        Box::new(el("h3", text("Playback fretboard"))),
        Box::new(
            el(
                "p",
                text(format!(
                    "{scope} · {} note positions · {} strings · string 1 at top",
                    ctx.contacts.len(),
                    score.tuning.len()
                )),
            )
            .attr("style", format!("font-size:13px;color:{dim}"))
            .attr("data-music-playback-scope-label", scope.clone()),
        ),
        Box::new(
            el("p", text(current.clone())).attr("data-music-playback-current", current.clone()),
        ),
    ];
    for (section_first, section_last) in fret_sections(first, last, width) {
        let count = usize::from(section_last - section_first) + 1;
        let cell = (width - 48.0) / count as f32;
        let row_h = 32.0;
        let top = 24.0;
        let height = top + score.tuning.len() as f32 * row_h;
        let mut neck: Vec<UiChild> = vec![];
        for fret in section_first..=section_last {
            neck.push(Box::new(el("span",text(fret.to_string())).attr("style",format!("position:absolute;left:{}px;top:0;width:{cell}px;text-align:center;font-size:12px;color:{dim}",48.0+f32::from(fret-section_first)*cell))));
        }
        for (row, open) in score.tuning.iter().enumerate() {
            let string = (row + 1) as u8;
            let y = top + row as f32 * row_h;
            // Use physical string identity, never pitch sorting: reentrant
            // strings retain their instrument order even when their pitch rises.
            neck.push(Box::new(el("span",text(format!("{} {}",string,note_name(*open)))).attr("data-music-playback-string",string.to_string()).attr("style",format!("position:absolute;left:0;top:{}px;width:46px;font-size:11px;color:{dim}",y+8.0))));
            neck.push(Box::new(el("div", ()).attr(
                "style",
                format!(
                    "position:absolute;left:48px;top:{}px;width:{}px;border-top:1px solid {dim}",
                    y + row_h / 2.0,
                    width - 48.0
                ),
            )));
            for fret in section_first..=section_last {
                let contact = (string, fret);
                let known = ctx.contacts.contains(&contact);
                let sounding = ctx.sounding.contains(&contact);
                let x = 48.0 + f32::from(fret - section_first) * cell;
                neck.push(Box::new(el("span",()).attr("style",format!("position:absolute;left:{x}px;top:{y}px;width:{cell}px;height:{row_h}px;border-right:1px solid {dim}"))));
                if known {
                    let name = score
                        .pitch(woodshedding::notation::FrettedNote { string, fret })
                        .map(note_name)
                        .unwrap_or_default();
                    let label = format!(
                        "String {string}, fret {fret}, {name}, {}",
                        if sounding {
                            "currently sounding"
                        } else {
                            "bar-range context"
                        }
                    );
                    let (bg, fg, border) = if sounding {
                        (&primary, &on_primary, &primary)
                    } else {
                        (&ground, &dim, &dim)
                    };
                    neck.push(Box::new(el("span",text(name)).attr("role","img").attr("aria-label",label)
                        .attr("class",if sounding {"music-playback-context music-playback-sounding"} else {"music-playback-context"})
                        .attr("data-music-playback-position",format!("s{string}/f{fret}/"))
                        .attr("data-music-playback-context","true").attr("data-music-playback-sounding",sounding.to_string())
                        .attr("style",format!("position:absolute;left:{}px;top:{}px;width:{}px;height:26px;line-height:24px;border:1px solid {border};border-radius:13px;text-align:center;font-size:11px;background:{bg};color:{fg}",x+3.0,y+3.0,(cell-6.0).max(1.0)))));
                }
            }
        }
        children.push(Box::new(
            el(
                "div",
                vec![
                    Box::new(
                        el("p", text(format!("Frets {section_first}–{section_last}")))
                            .attr("style", format!("font-size:13px;color:{dim}")),
                    ) as UiChild,
                    Box::new(el("div", neck).attr(
                        "style",
                        format!("position:relative;width:{width}px;height:{height}px"),
                    )),
                ],
            )
            .attr("class", "music-playback-neck-section")
            .attr("data-music-playback-first-fret", section_first.to_string())
            .attr("data-music-playback-last-fret", section_last.to_string()),
        ));
    }
    Box::new(
        el("section", children)
            .attr("role", "group")
            .attr("aria-label", format!("Playback fretboard context, {scope}"))
            .attr("class", "music-board music-playback-board")
            .attr("data-music-board", "playback-context")
            .attr(
                "data-music-playback-scope-first",
                ctx.scope
                    .map(|(b, _)| b.to_string())
                    .unwrap_or_else(|| "none".into()),
            )
            .attr(
                "data-music-playback-scope-last",
                ctx.scope
                    .map(|(_, b)| b.to_string())
                    .unwrap_or_else(|| "none".into()),
            )
            .attr(
                "data-music-playback-context-count",
                ctx.contacts.len().to_string(),
            )
            .attr(
                "data-music-playback-sounding-count",
                ctx.sounding.len().to_string(),
            )
            .attr(
                "style",
                format!("width:{width}px;max-width:100%;color:{ink}"),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::notation::{FrettedNote, Measure, NoteEvent, NoteValue, TimeSignature};
    fn score() -> Score {
        let notes = |notes: &[(u8, u8)], value| NoteEvent {
            notes: notes
                .iter()
                .map(|(string, fret)| FrettedNote {
                    string: *string,
                    fret: *fret,
                })
                .collect(),
            value,
            rest: notes.is_empty(),
        };
        Score {
            title: "Reentrant context".into(),
            instrument: "Ukulele".into(),
            tuning: vec![69, 64, 60, 67],
            tempo: 96,
            measures: vec![
                Measure {
                    meter: TimeSignature {
                        beats: 4,
                        beat_type: 4,
                    },
                    harmony: None,
                    events: vec![
                        notes(&[(4, 0), (1, 3)], NoteValue::Half),
                        notes(&[], NoteValue::Half),
                    ],
                },
                Measure {
                    meter: TimeSignature {
                        beats: 4,
                        beat_type: 4,
                    },
                    harmony: None,
                    events: vec![notes(&[(2, 4)], NoteValue::Whole)],
                },
            ],
        }
    }
    #[test]
    fn scoped_union_retains_other_events_and_deduplicates_contacts() {
        let score = score();
        let ctx = context(&score, Some((0, 0)), 0..=1);
        assert_eq!(ctx.contacts, BTreeSet::from([(1, 3), (2, 4), (4, 0)]));
        assert_eq!(ctx.sounding, BTreeSet::from([(1, 3), (4, 0)]));
        let one = context(&score, Some((1, 0)), 1..=1);
        assert_eq!(one.contacts, BTreeSet::from([(2, 4)]));
        assert_eq!(one.sounding, one.contacts);
    }
    #[test]
    fn rests_clear_highlights_without_losing_context_and_outside_playback_adds_nothing() {
        let score = score();
        let ctx = context(&score, Some((0, 1)), 0..=1);
        assert_eq!(ctx.contacts.len(), 3);
        assert!(ctx.sounding.is_empty());
        assert_eq!(ctx.current, Current::Rest { bar: 0, event: 1 });
        let outside = context(&score, Some((1, 0)), 0..=0);
        assert_eq!(outside.contacts.len(), 2);
        assert!(outside.sounding.is_empty());
        assert_eq!(outside.current, Current::OutsideScope);
    }
    #[test]
    fn reentrant_string_identity_is_preserved_and_same_pitch_contacts_are_not_merged() {
        let mut score = score();
        score.measures[1].events[0].notes = vec![
            FrettedNote { string: 3, fret: 7 },
            FrettedNote { string: 4, fret: 0 },
        ];
        assert_eq!(
            score.pitch(FrettedNote { string: 3, fret: 7 }).unwrap(),
            score.pitch(FrettedNote { string: 4, fret: 0 }).unwrap()
        );
        let ctx = context(&score, Some((1, 0)), 1..=1);
        assert_eq!(ctx.contacts, BTreeSet::from([(3, 7), (4, 0)]));
        assert_eq!(ctx.sounding, ctx.contacts);
        assert_eq!(score.tuning, vec![69, 64, 60, 67]);
    }
    #[test]
    fn wide_necks_wrap_readably_and_scopes_stay_bounded() {
        let contacts = BTreeSet::from([(1, 12), (4, 36)]);
        let (first, last) = fret_range(&contacts);
        assert_eq!((first, last), (0, 36));
        for width in [240.0, 320.0, 1000.0] {
            let sections = fret_sections(first, last, width);
            assert_eq!(sections.first().unwrap().0, 0);
            assert_eq!(sections.last().unwrap().1, 36);
            assert!(sections.windows(2).all(|pair| pair[0].1 + 1 == pair[1].0));
            assert!(
                sections
                    .iter()
                    .all(|(first, last)| (width - 48.0) / f32::from(last - first + 1) >= 44.0)
            );
        }
        assert!(context(&score(), None, 20..=usize::MAX).scope.is_none());
        assert_eq!(context(&score(), None, 0..=usize::MAX).scope, Some((0, 1)));
        assert!(context(&score(), None, 1..=0).contacts.is_empty());
    }
}
