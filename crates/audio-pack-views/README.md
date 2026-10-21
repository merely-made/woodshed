# audio-pack-views

Shared Cambium kit and step controls over a product-owned `PackSelection`.
`panel` reads the current selection and a transient status, then emits
`PackAction` through the host's callback. It owns no song, document, filesystem,
clock, audio graph or project lifecycle. Existing Cambium buttons supply input
and accessibility semantics.

The surface provides bundled/folder selection, pattern choice, three-state
steps (rest/hit/accent), mute, level, musical hit movement and MIDI export.
Hocket additionally offers creation of a normal recorder layer for audition.
Application transports remain visible alongside the controls. Authoring uses
the existing sequencer schema; both audio and MIDI derive from the retained
pattern at the app's current tempo.

Hit arrows use `scenomise::span_edit::SpanEdit` on a one-step musical span. The
product applies a revision-qualified `PackEdit` which refuses missing sources
and occupied destinations. Screen positions and automatic scene movement do
not author music. Larger rhythm/phrase scenes, typed routing and intentionally
bound dynamics remain in the
[canonical plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#mere-and-composition-surfaces-2026-10-19-assessment).
