# Appearance and native title bars

Historical source: [Shared Tabard authoring and native title bars](archive_docs/2026-11-03/2026-10-08_tabard_adoption_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Ownership and finding
Shared Tabard owns definitions and workshop; Woodshed owns app selection, native title-bar composition and device-local presentation. Keep authored definition library distinct from persona selection, and recover missing/damaged definitions without replacing authored files (original 19–75).

## Work and done conditions
Windows and Linux headed presentation, native OS decorations/traffic-light regions, live screen-reader acceptance and a formal contrast review remain separate from the bounded macOS client-area receipt. Verify pale Settings navigation label legibility, narrow-start seed/reopen pairs, default startup and circular-arrow glyph. Done when requested fonts/theme/mode survive fresh process reopen, controls and title-bar hit/drag/resize work in each targeted native platform, screen reader observes meaningful semantics, and actual contrast findings are resolved (174–183, 360–418).

## Deferred and limits
Other family consumers own adoption; Hocket project/media/custody and physical devices retain their respective plans. Prior screenshots cannot qualify operating-system chrome or other platforms.


## Findings and progress

No fresh build, test, headed, audio or hardware gate was run for this successor. Recheck open work against current source before implementation.

## Font-role authority

Original lines 29–43: shared `ThemeChoice.fonts`, `FontPreferences` and `FontRoleEditor` govern ordered requests and explicit reset/missing-family intent; authored typography supplies defaults and ordinary CSS retains cascade authority. Preserve failed-write stability and measured text, hit geometry and paint on mapped surfaces, retaining product state across reopening. Inventory alone does not prove the `⟳` glyph paints.

Current-source pointer: `crates/woodshed-views/src/appearance.rs:10` consumes `ThemeChoice` and `appearance/fonts.rs:6` uses shared font types; this does not prove native glyph or screen-reader acceptance.

## Coverage question for review

Which narrow-start, platform chrome, font-role and contrast gates have newer matching-cohort evidence? Retain platform-specific gaps without reopening bounded completed macOS work.
