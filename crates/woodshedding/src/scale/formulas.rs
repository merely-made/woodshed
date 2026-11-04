use super::*;

pub(super) static CATALOG: &[ScaleFormula] = &[
    // ── Diatonic modes ──
    ScaleFormula {
        // 1 2 3 4 5 6 7
        name: "Major",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    ScaleFormula {
        // 1 2 b3 4 5 6 b7
        name: "Dorian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    ScaleFormula {
        // 1 b2 b3 4 5 b6 b7
        name: "Phrygian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    ScaleFormula {
        // 1 2 3 #4 5 6 7
        name: "Lydian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    ScaleFormula {
        // 1 2 3 4 5 6 b7
        name: "Mixolydian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    ScaleFormula {
        // 1 2 b3 4 5 b6 b7  (natural minor)
        name: "Minor",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    ScaleFormula {
        // 1 b2 b3 4 b5 b6 b7
        name: "Locrian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Diatonic,
    },
    // ── Pentatonic ──
    ScaleFormula {
        // 1 2 3 5 6
        name: "Major Pentatonic",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
        ],
        category: ScaleCategory::Pentatonic,
    },
    ScaleFormula {
        // 1 b3 4 5 b7
        name: "Minor Pentatonic",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Pentatonic,
    },
    // ── Blues ──
    ScaleFormula {
        // 1 b3 4 b5 5 b7  (six-note minor blues)
        name: "Blues",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::DIMINISHED_FIFTH,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Blues,
    },
    ScaleFormula {
        // 1 2 b3 3 5 6  (six-note major blues)
        name: "Major Blues",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
        ],
        category: ScaleCategory::Blues,
    },
    // ── Harmonic minor and modes ──
    ScaleFormula {
        // 1 2 b3 4 5 b6 7
        name: "Harmonic Minor",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 b2 b3 4 b5 6 b7  (mode 2 of harmonic minor)
        name: "Locrian #6",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 2 3 4 #5 6 7  (mode 3 of harmonic minor)
        name: "Ionian #5",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::AUGMENTED_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 2 b3 #4 5 6 b7  (mode 4 of harmonic minor)
        name: "Dorian #4",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 b2 3 4 5 b6 b7  (mode 5 of harmonic minor)
        name: "Phrygian Dominant",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 #2 3 #4 5 6 7  (mode 6 of harmonic minor)
        name: "Lydian #2",
        intervals: &[
            I::PERFECT_UNISON,
            I::AUGMENTED_SECOND,
            I::MAJOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 b2 b3 b4 b5 b6 bb7  (mode 7 of harmonic minor)
        name: "Super Locrian bb7",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::DIMINISHED_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MINOR_SIXTH,
            I::DIMINISHED_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    // ── Melodic minor and modes (ascending form) ──
    ScaleFormula {
        // 1 2 b3 4 5 6 7
        name: "Melodic Minor",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 b2 b3 4 5 6 b7  (mode 2 of melodic minor)
        name: "Dorian b2",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 2 3 #4 #5 6 7  (mode 3 of melodic minor)
        name: "Lydian Augmented",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::AUGMENTED_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 2 3 #4 5 6 b7  (mode 4 of melodic minor)
        name: "Lydian Dominant",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 2 3 4 5 b6 b7  (mode 5 of melodic minor — Hindu / Aeolian dominant)
        name: "Mixolydian b6",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 2 b3 4 b5 b6 b7  (mode 6 of melodic minor)
        name: "Locrian #2",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    ScaleFormula {
        // 1 b2 b3 b4 b5 b6 b7  (mode 7 of melodic minor — the "altered scale")
        name: "Altered",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::DIMINISHED_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MINOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::AlteredMinor,
    },
    // ── Symmetric ──
    ScaleFormula {
        // 1 2 3 #4 #5 #6  (six tones, all whole steps)
        name: "Whole Tone",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::AUGMENTED_FIFTH,
            I::AUGMENTED_SIXTH,
        ],
        category: ScaleCategory::Symmetric,
    },
    ScaleFormula {
        // 1 2 b3 4 b5 b6 bb7 7  (W H W H W H W H from root)
        name: "Diminished (whole-half)",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MINOR_SIXTH,
            I::DIMINISHED_SEVENTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Symmetric,
    },
    ScaleFormula {
        // 1 b2 b3 3 #4 5 6 b7  (H W H W H W H W from root — common over dominant 7 chords)
        name: "Diminished (half-whole)",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::MAJOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Symmetric,
    },
    ScaleFormula {
        // 1 #2 3 5 #5 7  (alternating m3 and m2)
        name: "Augmented",
        intervals: &[
            I::PERFECT_UNISON,
            I::AUGMENTED_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::AUGMENTED_FIFTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Symmetric,
    },
    ScaleFormula {
        // 1 b2 2 b3 3 4 #4 5 b6 6 b7 7  (all 12 notes)
        name: "Chromatic",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Symmetric,
    },
    // ── Bebop ──
    ScaleFormula {
        // 1 2 3 4 5 6 b7 7
        name: "Bebop Dominant",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Bebop,
    },
    ScaleFormula {
        // 1 2 3 4 5 #5 6 7
        name: "Bebop Major",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::AUGMENTED_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Bebop,
    },
    ScaleFormula {
        // 1 2 b3 3 4 5 6 b7
        name: "Bebop Dorian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Bebop,
    },
    // ── Exotic ──
    ScaleFormula {
        // 1 2 b3 #4 5 b6 7
        name: "Hungarian Minor",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::AUGMENTED_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Exotic,
    },
    ScaleFormula {
        // 1 b2 3 4 5 b6 7  (Byzantine / Arabic)
        name: "Double Harmonic Major",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Exotic,
    },
    ScaleFormula {
        // 1 b2 b3 4 5 6 7
        name: "Neapolitan Major",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Exotic,
    },
    ScaleFormula {
        // 1 b2 b3 4 5 b6 7
        name: "Neapolitan Minor",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Exotic,
    },
    ScaleFormula {
        // 1 2 b3 5 b6
        name: "Hirajoshi",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_SECOND,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SIXTH,
        ],
        category: ScaleCategory::Exotic,
    },
    ScaleFormula {
        // 1 b2 4 5 b7
        name: "In Sen",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::PERFECT_FOURTH,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ScaleCategory::Exotic,
    },
    ScaleFormula {
        // 1 b2 3 4 b5 b6 7
        name: "Persian",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_SECOND,
            I::MAJOR_THIRD,
            I::PERFECT_FOURTH,
            I::DIMINISHED_FIFTH,
            I::MINOR_SIXTH,
            I::MAJOR_SEVENTH,
        ],
        category: ScaleCategory::Exotic,
    },
];
