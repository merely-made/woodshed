# Licenses in this repository

**This repository: MPL-2.0.** Every file Mark wrote carries Exhibit A and the
SPDX tag `MPL-2.0`, per the
[license posture brief](../mere/design_docs/2026-08-22_license_posture_brief.md)
of 2026-08-22 (mere `design_docs/2026-08-22_license_posture_brief.md`). The full
text is in [`LICENSE`](LICENSE).

This file is the provenance ledger. It is the authority for what the relicense
tool (mere `scripts/relicense_headers.py`) skips: the backtick-quoted paths in
the **Retained licenses** table are never touched. Provenance comes before
license — a file gets Exhibit A only if Mark wrote it.

## Retained licenses

Third-party code keeps its own license and its own notices. Nothing here is
relicensed, and nothing here receives a Merely copyright line.

| Path | License | Upstream | Notice files |
|---|---|---|---|

**None.** Hocket vendors no third-party source. The discovery sweep of
2026-09-03 — `Copyright` unqualified, `Licensed under`, `Permission is hereby
granted`, `Apache License`, and every SPDX line — found no foreign notice in any
of the 31 tracked sources. The only `Copyright` hits in the tree were the root
`LICENSE-MIT` / `LICENSE-APACHE` pair's own boilerplate, which carries Mark's
own `Mark AB (markik)` line and is replaced by this sweep.

Hocket is an audio application, so the sweep looked specifically for lifted DSP
and codec code. There is none in-tree: the WavPack codec is **wavicle**
(`repos/wavicle`, a clean-room pure-Rust implementation with its own repository
and its own sweep phase), the shared DSP primitives — click synthesis, streaming
onset and tempo detection, latency estimation, waveform peaks — are
`audio-primitives` in the woodshed repository, and the audio graph is upstream
[Firewheel](https://github.com/BillyDM/Firewheel). All three arrive as
dependencies, not as vendored files, so their terms travel with them and none of
them is this repository's to relicense.

`crates/hocket-genet/src/scenario.rs` says it was "adopted from turnstone".
Turnstone is Mark's own repository, so that is his code moving between his own
trees, not a third-party import.

## Derivatives carrying MPL-2.0 with an upstream notice retained

| Path | Upstream | Notices kept |
|---|---|---|

**None.** No file in this repository is a derivative of anyone else's work, so
`--retain-notice` was not needed for the sweep.

**This section is deliberately not the skip list.** The tool reads only the
`## Retained licenses` table above. Adding a path here documents a disposition;
it does not exempt the path from receiving a header.

## Exceptions under the fork/vendor criterion

**None.** The brief's §4 test — a crate stays MIT OR Apache-2.0 only when a
third party would need to *modify or vendor* it rather than merely link it —
admits nothing in this repository. Hocket's four crates are all `publish =
false` or unpublished application code; nothing here is a reference
implementation or a contract for foreign implementers.

If one is ever granted, its manifest says `MIT OR Apache-2.0` explicitly with a
comment naming the brief, and it is listed here.

## How to add a file from elsewhere

1. Do not delete or rewrite the upstream copyright or license notice, ever.
2. Add its path to **Retained licenses** above with its license, upstream URL,
   and where its notice text lives. The tool then skips it automatically.
3. If it is a substantial derivative rather than a verbatim import, the brief's
   rule is MPL-2.0 on the derivative *with the upstream notice retained* —
   record it in that section so the distinction is not lost.
4. Never add `license-file` to an owned manifest; the field is for retained
   third-party crates only.
5. Re-run `python ../mere/scripts/relicense_headers.py --repo . --audit` and
   confirm the owned source count moved by exactly what you expected.
