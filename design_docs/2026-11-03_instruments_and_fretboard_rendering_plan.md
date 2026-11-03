# Instrument and fretboard

Historical source: [Stringed Instrument Variety and Fretboard Rendering Plan](archive_docs/2026-11-03/2026-07-14_instruments_and_fretboard_rendering_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Design and open decisions
Realistic logarithmic and schematic equal-cell spacing are independent of full-neck and windowed extent. Fretted notes occupy cells, fretless notes semitone bands, open strings a nut marker; settings for spacing, extent, markers and fill remain user configurable. Original decisions lines 53–107. Confirm context-aware defaults, whether scale length is needed now, and course representation; non-12-TET is a prerequisite to faithful microtonal instruments (109–120, 214–224).

## Work and done conditions
Recheck current instrument catalog and per-instrument standard fret counts; microtonal A3 requires pitch-model work. Complete any missing leaf geometry, fretless zones, proper inlays, degree fill, marker controls and instrument-filtered tuning picker. Done when standard counts drive full-neck extent, both spacing/extent modes and marker settings persist and render, degree/note fill is selectable, a violin/cello renders honest fretless regions, and picker filters tunings (122–213). Confirm current status before assigning implementation.

## Deferred and limits
Real course pairs, non-12-TET and true cross-instrument physical scale need separate modeling; do not imply flat courses are faithful. Theme-derived board colors and remaining lens adoption need rechecking against current code (264–302). Paint tests do not establish user or native visual acceptance.


## Findings and progress

No fresh build, test, headed, audio or hardware gate was run for this successor. Recheck open work against current source before implementation.

## Prototype backlog for current-source reconciliation

Carry shared musical context with independent catalog selections and configurable neck window from compact voicing to full neck. Preserve requested free tuner and explicit baritone presets. Check non-tertiary scales, spelling-aware recognition, CAGED/playability and nuanced fingering, nearest-string tuner targeting, practice generators, minor-jazz progressions and diatonic-arpeggio exercises against current theory/audio code before naming defects or completion. The later horizontal/vertical setting governs orientation. Mobile fit, touch and accessible custom leaves require their own qualification. Original 2026-11-03 prototype carry.

## Coverage question for review

Which A–F settings, degree fills, inlays and catalog details are already implemented in current source, and how should historical course/default/scale-length questions be resolved?
