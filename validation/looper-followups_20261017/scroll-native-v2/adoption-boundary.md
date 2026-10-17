# Mere clip-fix adoption boundary

2026-10-17. Read-only source assessment; no adoption authorized. Parent prioritizes a real focused-capture native receipt on the current qualified Woodshed suppliers.

## Exact source boundary

| Supplier | Qualified Woodshed lock | Reviewed Mere main dependency target |
|---|---|---|
| Mere | `git+https://github.com/merely-made/mere.git?branch=main#4c4ea11d8ea4b0210fd70af62638484e25f29d5e` | Main baseline `515b30bed4572d1811a4e80d34c1c930f2f3843e`; main-descendant clip-fix candidate not yet qualified |
| Genet | `git+https://github.com/merely-made/genet.git?branch=main#6186c81ad7019ea51dd28861ef83574bfc992ed1` | `git+https://github.com/merely-made/genet.git?branch=main#7f13b85f53c48fa90f433a6cd2ad45cd744e8b02` |
| Netrender family | `git+https://github.com/merely-made/netrender.git?branch=main#8960024f7b1f2b7a5ba382972f08b2f75eb7a0f0` | `git+https://github.com/merely-made/netrender.git?rev=e8cda06efc11c11595f2fa1bb30bce7e72979537#e8cda06efc11c11595f2fa1bb30bce7e72979537` |

The isolated fix is `4140a8a31cdbe40ef7f2cfbb34caf4202610833f`, directly on Mere `4c4ea11`; optional document-scroll observation `7fb709fb103c6d24d9df41e5da66b746ff48c1eb` directly follows it. Their common ancestor with reviewed main is `4c4ea11`. A future publication candidate must preserve current main history by applying the fix/query onto main, without force-publishing the older-base branch.

## Consumed family: exactly 8 changed / 44 locked packages

Between Mere `4c4ea11..515b30b`, these eight Woodshed-consumed packages have changed Rust source or package manifests; the other 36 have neither kind of change. Root workspace manifest changes apply additionally.

| Package | Relevant change |
|---|---|
| `cambium-rootstock` | Incremental styling from drained mutations; checked presentation; browser renderer access restriction |
| `cambium-genet-winit-host` | Retained-projection selector matching; test fixture feature |
| `mesquite` | Test matcher and fixture feature |
| `tabard-workshop` | Taproot fixture feature and selector migrations, principally tests |
| `pictograph` | Shared image storage: `Vec<u8>` to `Arc<Vec<u8>>` |
| `document-canvas` | Public `DecodedImage.rgba8` becomes `Arc<Vec<u8>>`; Serde `rc` |
| `mere-document-lanes` | Image decoding adapts to shared storage |
| `esp` | Decoder/model architecture changes; current Woodshed lock selects only ESP's `serde`/`zeroize` dependency footprint |

## Required adoption work and proof

- Mere main is incompatible with unchanged Genet `6186`: main names `SurfaceAcquire::Reconfigured` and requests Taproot `estimated-probe-fixtures`, both absent there. Incremental styling and projection matching already exist at `6186` and do not themselves force an upgrade.
- Main's exact locked Genet `7f13b85` supplies those APIs but removes `taproot::matching`. Woodshed has production scenario callers plus tests and separate Hocket callers to migrate. Class selectors use `matching_classes`; role selectors require the retained projection API. A blanket class-matcher rename would panic on role selectors.
- Align direct Netrender dependencies/patches to the same qualified source. Mere/Genet's explicit `rev=e8cda06` and Woodshed's `branch=main` are different source identities even if package versions match. A split risks incompatible paint-list types, including waveform tests.
- This is a broader rendering/style/selector adoption, not just the clip patch. The Genet advance includes layout, text, image, transform, and selector changes. `7f13b85` is the exact reviewed Mere-main lock target, not a claim of earliest compatibility or Woodshed qualification.
- Before future publication approval: qualify an exact main-descendant fix/query SHA, repeat clip regressions across initial layout and relayout, then qualify an adapted Woodshed consumer with exact suppliers and parent-owned native clipping/scroll/selector proof.

Parent-reported negative 5 failures / fixed 114 passed, 1 ignored qualify the isolated older-base patch only. A local `7fb` consumer check does not qualify the main-based supplier combination. No new execution evidence was produced by this review. Current qualified suppliers remain the focused-capture native target; no Mere adoption changes now.
