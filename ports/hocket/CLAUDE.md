# Hocket Repository Guide

## Product

Hocket is a cross-platform loop recorder with asynchronous turn-taking. The
core musical gesture is adding a layer to a short loop, then handing a session
to another person when sharing is available. It is not an Ableton-shaped DAW
and it is not a real-time network jam tool.

The default looper-pedal profile starts with four tracks whose unmuted layers
sum. The named Deeler profile starts with ten tracks and selects one active
layer per track. Counts and capture settings are stored in the session so they
remain configurable.

`design_docs/PROJECT_DESCRIPTION.md` is maintainer-owned product authority.
Read `design_docs/DOC_README.md` before planning or changing subsystem scope.

## Workspace

```
crates/
  hocket-model/     Session and history authority; framework independent
  hocket-engine/    Firewheel capture, playback, click, and media abstraction
  hocket-headless/  Scripted audio-engine harness
  hocket-genet/    Genet/winit application host and recorder UI
```

Run the desktop application with `cargo run -p hocket-genet`. The retired
Masonry application and `hocket-widgets` crate are not part of this workspace.

The sibling `../woodshed/crates/audio-primitives` path dependency provides
shared pure DSP helpers. Do not couple Hocket to a Woodshed application crate.

## Boundaries

- Keep `hocket-model` independent of UI and audio frameworks. Session edits
  that must survive undo or synchronization belong in `Edit` and `History`.
- Keep `hocket-engine` as a runtime projection. It can own real-time graph and
  device concerns, but not authoritative session state.
- Keep `hocket-genet` thin. Host-local presentation state is acceptable;
  session, media, and collaboration semantics are not.
- Build local durability before peer synchronization: a peer cannot reliably
  import or share a session that the originating host cannot reopen.
- Do not add plugin hosting or an arrange view while the loop recorder, export,
  and hand-off flows are incomplete.

## Workspace tooling

sem and weave are wired into this repo. Both are described once in
`Code/CLAUDE.md`, which loads for any session at or below `Code`.

## Documentation

Follow `design_docs/DOC_POLICY.md`. Non-trivial work gets a dated plan in
`design_docs/`, whose progress reflects the live code and verification state.
Do not edit `PROJECT_DESCRIPTION.md` without explicit maintainer direction.
