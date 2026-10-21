//! Pure barline derivation and grouping validation for authored arrangements.
use super::ProgressionArrangement;
use serde::{Deserialize, Serialize};

const MAX_EXTENT: u32 = 65_536;
const MAX_BARS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSignatureChange {
    pub start: u32,
    pub numerator: u8,
    pub denominator: u8,
}

/// A contiguous, zero-based range of derived bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarGroup {
    pub first: u32,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar {
    pub start: u32,
    pub length: u32,
    pub complete: bool,
}

/// Derive bars from the explicit meter map. Four existing arrangement ticks
/// make one quarter note; omitted meter deliberately yields no bars.
pub fn bars(arrangement: &ProgressionArrangement) -> Result<Vec<Bar>, String> {
    let changes = &arrangement.time_signatures;
    if arrangement.extent == 0 || arrangement.extent > MAX_EXTENT {
        return Err("Arrangement extent exceeds the bar model limit".into());
    }
    if changes.is_empty() {
        if !arrangement.bar_groups.is_empty() {
            return Err("Bar groups require an authored time signature".into());
        }
        return Ok(Vec::new());
    }
    if changes[0].start != 0 {
        return Err("Time signature map must begin at tick zero".into());
    }
    // Validate the whole map before doing any region arithmetic. In
    // particular, an out-of-order change must not underflow a region length.
    for (i, change) in changes.iter().enumerate() {
        if !(1..=32).contains(&change.numerator)
            || !matches!(change.denominator, 1 | 2 | 4 | 8 | 16)
            || change.start >= arrangement.extent
            || (i > 0 && change.start <= changes[i - 1].start)
        {
            return Err("Time signature map has an invalid or out-of-order change".into());
        }
        if i > 0 {
            let previous = changes[i - 1];
            let previous_bar_ticks =
                u32::from(previous.numerator) * 16 / u32::from(previous.denominator);
            if (change.start - previous.start) % previous_bar_ticks != 0 {
                return Err("Time signature changes must land on a bar boundary".into());
            }
        }
    }
    let mut out = Vec::new();
    for (i, change) in changes.iter().enumerate() {
        let bar_ticks = u32::from(change.numerator) * 16 / u32::from(change.denominator);
        // All supported denominators divide 16, so this is exact.
        let region_end = changes
            .get(i + 1)
            .map_or(arrangement.extent, |next| next.start);
        let mut start = change.start;
        while start < region_end {
            if out.len() == MAX_BARS {
                return Err("Arrangement exceeds the 512 bar limit".into());
            }
            let length = (region_end - start).min(bar_ticks);
            out.push(Bar {
                start,
                length,
                complete: length == bar_ticks,
            });
            start += length;
        }
    }
    validate_groups_for_bars(&arrangement.bar_groups, out.len())?;
    Ok(out)
}

pub fn validate_groups(arrangement: &ProgressionArrangement) -> Result<(), String> {
    bars(arrangement).map(|_| ())
}

/// Replace any existing groups wholly covered by the selected consecutive bar
/// range with the requested partition. The arrangement is never mutated.
pub fn regroup(
    arrangement: &ProgressionArrangement,
    selected: &[usize],
    counts: &[u32],
) -> Result<Vec<BarGroup>, String> {
    let derived = bars(arrangement)?;
    if selected.is_empty()
        || selected
            .windows(2)
            .any(|pair| pair[0].checked_add(1) != Some(pair[1]))
        || selected.iter().any(|index| *index >= derived.len())
        || counts.is_empty()
        || counts.contains(&0)
        || counts
            .iter()
            .try_fold(0usize, |sum, count| sum.checked_add(*count as usize))
            != Some(selected.len())
    {
        return Err(
            "Regrouping requires consecutive bars and positive counts covering them".into(),
        );
    }
    let first = selected[0] as u32;
    let end = (selected[selected.len() - 1] + 1) as u32;
    let mut groups = Vec::new();
    for group in &arrangement.bar_groups {
        let group_end = group.first + group.count;
        let intersects = group.first < end && first < group_end;
        if intersects {
            if group.first < first || group_end > end {
                return Err("Regrouping cannot partially replace an existing group".into());
            }
        } else {
            groups.push(*group);
        }
    }
    let mut cursor = first;
    for count in counts {
        groups.push(BarGroup {
            first: cursor,
            count: *count,
        });
        cursor += *count;
    }
    groups.sort_by_key(|group| group.first);
    validate_groups_for_bars(&groups, derived.len())?;
    Ok(groups)
}

fn validate_groups_for_bars(groups: &[BarGroup], bar_count: usize) -> Result<(), String> {
    if groups.len() > MAX_BARS {
        return Err("Arrangement exceeds the 512 bar group limit".into());
    }
    let mut occupied = vec![false; bar_count];
    for group in groups {
        let end = group.first.checked_add(group.count);
        if group.count == 0 || end.is_none_or(|end| end as usize > bar_count) {
            return Err("Bar group must select a nonempty bounded range".into());
        }
        for used in &mut occupied[group.first as usize..end.unwrap() as usize] {
            if *used {
                return Err("Bar groups cannot overlap".into());
            }
            *used = true;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrangement(
        extent: u32,
        time_signatures: Vec<TimeSignatureChange>,
    ) -> ProgressionArrangement {
        ProgressionArrangement {
            name: "bars".into(),
            extent,
            role_spans: vec![],
            cards: vec![],
            rests: vec![],
            time_signatures,
            bar_groups: vec![],
        }
    }

    #[test]
    fn meter_changes_at_bar_boundary_and_partial_final_bar_is_explicit() {
        let mut a = arrangement(
            50,
            vec![
                TimeSignatureChange {
                    start: 0,
                    numerator: 4,
                    denominator: 4,
                },
                TimeSignatureChange {
                    start: 32,
                    numerator: 3,
                    denominator: 4,
                },
            ],
        );
        let result = bars(&a).unwrap();
        assert_eq!(
            result,
            vec![
                Bar {
                    start: 0,
                    length: 16,
                    complete: true
                },
                Bar {
                    start: 16,
                    length: 16,
                    complete: true
                },
                Bar {
                    start: 32,
                    length: 12,
                    complete: true
                },
                Bar {
                    start: 44,
                    length: 6,
                    complete: false
                },
            ]
        );
        a.time_signatures[1].start = 24;
        assert!(bars(&a).is_err());
    }

    #[test]
    fn rejects_invalid_meters_and_overlapping_groups() {
        let mut a = arrangement(
            64,
            vec![TimeSignatureChange {
                start: 0,
                numerator: 0,
                denominator: 4,
            }],
        );
        assert!(bars(&a).is_err());
        a.time_signatures[0].numerator = 4;
        a.bar_groups = vec![
            BarGroup { first: 0, count: 2 },
            BarGroup { first: 1, count: 1 },
        ];
        assert!(validate_groups(&a).is_err());
    }

    #[test]
    fn malformed_order_and_zero_extent_return_errors_without_panicking() {
        let mut a = arrangement(
            64,
            vec![
                TimeSignatureChange {
                    start: 0,
                    numerator: 4,
                    denominator: 4,
                },
                TimeSignatureChange {
                    start: 32,
                    numerator: 3,
                    denominator: 4,
                },
                TimeSignatureChange {
                    start: 16,
                    numerator: 4,
                    denominator: 4,
                },
            ],
        );
        assert!(bars(&a).is_err());
        a.extent = 0;
        a.time_signatures.clear();
        assert!(bars(&a).is_err());
    }

    #[test]
    fn absent_meter_keeps_legacy_arrangements_without_bars() {
        let a = arrangement(64, vec![]);
        assert!(bars(&a).unwrap().is_empty());
        let mut value = serde_json::to_value(&a).unwrap();
        value.as_object_mut().unwrap().remove("time_signatures");
        value.as_object_mut().unwrap().remove("bar_groups");
        let old: ProgressionArrangement = serde_json::from_value(value).unwrap();
        assert!(bars(&old).unwrap().is_empty());
    }

    #[test]
    fn regroup_replaces_whole_groups_and_refuses_partial_overlap() {
        let mut a = arrangement(
            80,
            vec![TimeSignatureChange {
                start: 0,
                numerator: 4,
                denominator: 4,
            }],
        );
        a.bar_groups = vec![
            BarGroup { first: 0, count: 2 },
            BarGroup { first: 4, count: 1 },
        ];
        assert_eq!(
            regroup(&a, &[0, 1], &[1, 1]).unwrap(),
            vec![
                BarGroup { first: 0, count: 1 },
                BarGroup { first: 1, count: 1 },
                BarGroup { first: 4, count: 1 },
            ]
        );
        assert!(regroup(&a, &[1, 2], &[2]).is_err());
    }
}
