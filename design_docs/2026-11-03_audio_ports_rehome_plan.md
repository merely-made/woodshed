# Audio foundation and packs

Historical source: [Audio ports rehome plan](archive_docs/2026-11-03/2026-09-09_audio_ports_rehome_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Ownership
Hocket and Ringdown remain independently named nested workspaces; product execution, scheduling, media ownership and audio devices stay in their hosts. Shared primitives return facts rather than replacing product transport or custody. Rehome itself landed; audio foundation and packs retain open work (original 1–51, 129–153).

## Work and done conditions
Recheck F4/F5 portable seams and shared DSP against current owners before extraction. Preserve Hocket and Woodshed distinct capture scheduling, overflow and format contracts; qualify actual adapters and allocation guards. Retain input resampling, sample-accurate calibration, callback-safe output/recording, native handoff and project/reopen gates where not subsequently qualified (184–376, 396–692).

AP1/AP2 implemented and AP3 has bounded macOS/guitar evidence. Complete desktop platform matrix, latency and broader instruments; verify exact saved pack sources and asset restoration, production runtime MIDI/WAV parity and actual instrument delivery separately. Mere quantized operation and Firewheel/FunDSP adapter directions remain distinct supplier work; variable patch graphs and composition surfaces are future work (693–1000, 1014–1118).

## Deferred and limits
Physical input, audibility and timing are not established by offline PCM; release terms and adapter provenance belong to Ringdown. No additional hardware operation is authorized by this draft.


## Findings and progress

No fresh build, test, headed, audio or hardware gate was run for this successor. Recheck open work against current source before implementation.

## Closure carry-forward check (2026-11-03)

Shared capture accumulator/seams extraction is complete only within its recorded CPU/offline scope. Native, reopened-profile, physical-input, mixed-device and latency evidence were not refreshed. Scheduling, clock identity, integrity admission, storage and completion stamps remain product-owned. Original Closure carry-forward.

## Port ruling details

Original ruling, lines 27–45: generic multi-product code belongs in root `crates/`, and a port must not depend on another solely because they share a repository. The independent workspaces retain their own locks, package versions, executable hosts, release tags, indexes and checks. Product-qualified tags are `woodshed-v*`, `hocket-v*`, `redshank-v*`, `ringdown-v*`; Hocket updater requires its own stable signed manifest URL rather than repository-wide latest-release shorthand. Preserve this ruling when proposing source placement or releases.

## Portable seam acceptance details

Original F5, lines 351–375: Hocket project/media/store/export/handoff work without device runtime; Redshank shares transitions for actual standalone and Turnstone hosts with explicit playback/microphone capability admission. Shared path crates need independent Git/package sourcing before extraction. Package renaming, updater retirement and browser-preview promotion require explicit product decisions. Project-worker session/generation correlation was historically flagged; recheck the subsequently completed correlation slice before reopening it. Browser fixtures do not prove real clocks, physical capture or durable storage.

## Guitar-specific export follow-up

The observed five-bar 4/4 file supplied a first-bar lead-in followed by repeated bars 2–5. Ringdown's proposed guitar profile should explicitly distinguish lead-in and repeat body while preserving generic MIDI export. Other lengths, meters, firmware and phase/latency remain unqualified. This is planned adaptation, not shipped behavior (original 939–955).

## Coverage question for review

Which F3–F5 portable seam and AP0 supplier follow-ups have since landed, and which are owned by the nested port plans? Preserve remaining gates and explicit decision points before implementation or claiming closure.

## Shared recorder boundary and Hocket adoption

The shared recorder boundary remains: `hocket-model` owns Session/History/Phrase/Layer; `hocket-project` owns portable media, waveform, storage, export and handoff; `hocket-recorder` owns gestures, capture promotion and fallible host-audio seams; `hocket-views` owns shared presentation. The Hocket host owns native devices, identity, workers, dialogs, appearance, updates and lifecycle. Device-free library construction must not open devices or create host services. Woodshed retains Set/Card/saved-Song timing, accepted backing, custody, capture companions and WAV export; `.hock` identity and Woodshed companion digest remain separate. Capture completions must remain correlated with session generation, stable target and ticket. [Host-refactor:397–428.]

Shared frame-driven DSP/audio-kernel consolidation is still later work. Done when both product adapters preserve their admitted capture schedules, audio authority, custody, timing and export behavior, with focused shared-boundary checks and separate native/physical gates. The historical standalone packet does not qualify arbitrary sizes, compact scrolling, default startup or physical audio; later native project/handoff and worker correlation have their own bounded evidence. [Host-refactor:430–454, 520–550.]
