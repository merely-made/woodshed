# Wavicle design documents

This is the canonical index under [DOC_POLICY](DOC_POLICY.md). The
maintainer-reserved PROJECT_DESCRIPTION.md has not been authored; the root
README remains the public overview.

## Active plan

- [WavPack codec](2026-07-15_wavpack_codec_plan.md): M0-M6 implementation,
  remaining codec acceptance, and the bounded H7 conformance-source split.
  [Run receipt](archive_docs/receipts/2026-07-15_wavpack_codec_plan_receipts.md).

## Working principles

- Source-only fixture moves preserve every captured byte, assertion and
  encode/decode feature guard. Reference CLI evidence keeps its exact tool
  version and differs from self-round-trip evidence.
- H7 uses local tagged ceiling/Clippy gates and a stable repository Cargo
  target, two jobs, with compact receipts under the shared 100 MB cap.
