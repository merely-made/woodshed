# Independent image review: scroll-native-v2

Reviewed 2026-10-17. All 11 actual `seed/*.png` files were opened individually using `view_image`. No image findings below are inferred from product source. Original PNG dimensions and SHA-256 hashes identify the exact files reviewed; the tool resized larger images for display, so this is not a pixel-by-pixel original-resolution audit.

## Scope and conclusion

**The bounded isolated receiving visual proof is supported, with the viewport limits listed below. No new visual blocker was found in the 11 captured states.** All images are visibly nonblank. No full-width black lower strip appears. Product header/search remain present, while inner content changes scroll position. Compact Card 3 and transport are exposed together; short Card 3 and full transport are exposed at separate scroll positions; 1.25x Card 3 plus transport and subsequent Card 2 envelope recovery are visibly demonstrated.

This evidence is only for isolated Mere revision `7fb709fb103c6d24d9df41e5da66b746ff48c1eb`, based on qualified `4c4ea11d8ea4b0210fd70af62638484e25f29d5e`, as recorded in `supplier.json`. **It does not establish adoption into production Woodshed main or Mere main.** It also does not qualify physical microphone/audio quality, arbitrary responsive sizes, or fresh-process capture reopening.

## Critical checkpoints and bounds

- **06, short Card 3:** the complete row and selector now sit below the upper clip, with visible space above the row. V1's top-clipped selector limitation is absent in this v2 image. The PNG is taken after selection: it shows an amber selected selector. The frozen `seed/scenario.scn` asserts `recorder-card3-select-visible` and `recorder-card3-clear-visible` before the click; `seed/scenario.done` records `RESULT ok` without failures. Thus pre-click visibility is supported by the executed scenario assertions, while the PNG directly shows the later fully exposed geometry. There is no separate pre-click PNG.
- **09, 1.25x Card 3:** full Card 3 selector/title/metadata/Clear and the matching transport are visible together. The initial sliver in image 08 does not limit the later demonstrated access.
- **11, recovery:** full selected Card 2 envelope returns, retaining its visible eight-region structure and 1.125 s label. Transport returns to Card 2. The scenario also invokes its existing exact-envelope verifier; image review alone cannot establish bit equality.
- **Black strip/background:** all 11 current captures retain navy background through the image bottom. The prior old native-v2 full-width black region is not visible in these captures. No claim about uncaptured intermediate frames is made.
- **Footer:** the Bars-or-Seconds explanation is complete and readable in all images, with wrapping at compact/short widths. In 06 the footer is outside the clipped recorder viewport and lower transport content is not fully visible until 07. This is a viewport boundary, not visible footer text painting over exposed controls.

## Per-image observations and SHA-256

### 01_wide_baseline_edges.png

- Original dimensions: 2360 × 2000.
- SHA-256: `1ed79c3fedb3f400be77f3f9057928a1bb2edc49a73ecb154c23053c2d30649e`.
- Visible evidence: Wide, 1x: all three rows/selectors/Clear buttons are visible. Card 2 is selected with 1.125 s / 180 bpm click / 3/4; full stepped envelope is visible. Complete transport and status lines; explanatory footer fits. Navy background covers the frame to the bottom; no black strip or visible row/transport collision.

### 02_compact_before_scroll.png

- Original dimensions: 840 × 2200.
- SHA-256: `56bf302141990de8447ceb139a2a13ac8370fc8db930119b3aceccfc4c82fd20`.
- Visible evidence: Compact initial position: three complete stacked rows including Card 3 selector/title/metadata/Clear; full selected Card 2 envelope. Only the beginning of the selected-timing transport heading is visible at the recorder bottom, so this image does not establish transport control visibility. External footer wraps completely and does not overlap card content. No black strip.

### 03_compact_third_card_edges.png

- Original dimensions: 840 × 2200.
- SHA-256: `174708b0d45dac2a4f37d26c1a091d500b04fd3e4cab267eff7fe070550d775b`.
- Visible evidence: Compact after Card 3 selection: complete selected Card 3 row, full amber selector, title, 0.875 s / 300 bpm click / 3/4 metadata and Clear. Full transport below reads matching timing and Record Selected Card 3, with all transport buttons visible. Both status lines and footer visible. Upper Card 2 title is outside the scroll viewport while its envelope remains visible. No black strip or horizontal clipping of these controls.

### 04_compact_transport_boundary_edges.png

- Original dimensions: 840 × 2200.
- SHA-256: `41e765a59ccafaedd2dd40175fccf5b3bd89a672fdf0c171f5341c502105d64e`.
- Visible evidence: Repeated-wheel compact boundary position retains complete selected Card 3 row and full transport, statuses and footer. Product header/search remain fully visible. No black strip, overlap, or horizontal clipping observed. Card 2 row is partially clipped at recorder top, but this does not cover Card 3.

### 05_short_workspace_scroll_edges.png

- Original dimensions: 840 × 1440.
- SHA-256: `e1703bd8c9b5fcd7a89820b2828ac74a725270aa81b3188486126e84d29b6d17`.
- Visible evidence: Short workspace scroll: product header/search remain fully visible; upper workspace controls have scrolled away within their own region. Chord and selected Card 2 rows, full eight-region envelope and Clear are visible. Card 3 is only a lower sliver and transport is outside view. Footer is fully legible at image bottom; no black lower region. This initial position does not establish Card 3 access.

### 06_short_third_card_edges.png

- Original dimensions: 840 × 1440.
- SHA-256: `c2fa24c63410d4ae78e7b22dbca264234e024b51d9a57633eefeb4c92efc1496`.
- Visible evidence: Short Card 3 position is visibly improved from v1: the entire selected Card 3 row has its rounded top edge below the recorder clip, with a clear vertical gap above. Full amber selector, title, Set 1/Card 3 identity, 0.875 s / 300 bpm click / 3/4 and Clear are visible. The selector no longer touches the upper clipping edge. Lower transport is intentionally cut by the recorder bottom: record/play/stop/rewind controls and caption are not completely visible here. External explanatory footer remains complete. No black strip or visible footer overpaint. This is a post-click selected-state PNG, not a pre-click screenshot.

### 07_short_transport_boundary_edges.png

- Original dimensions: 840 × 1440.
- SHA-256: `7c9f41cc1c509f338679fb659b08fc04082fd66fe9ff54a4c1675a6a35b29a95`.
- Visible evidence: Short transport boundary: only the bottom of Card 3 and its Clear remain at recorder top; selector/title are outside view. Full transport exposes Record Selected Card 3, matching 0.875 s timing, tempo down/up, click/replace and play/stop/rewind. Both status lines and complete external footer visible. Navy background reaches bottom. No black strip or overlap. Card 3 selector and all transport controls are qualified at separate short scroll positions, not simultaneously by this image.

### 08_zoom125_before_scroll.png

- Original dimensions: 2360 × 2000.
- SHA-256: `fa7437af096018e95158723c1d946095e226a71f2f58db25c6c35b19a1c6023a`.
- Visible evidence: Wide 1.25x initial position: full chord and selected Card 2 envelope rows; Card 3 is a thin sliver above the transport. Complete transport, status lines and footer; timing and record caption wrap legibly. No black strip or horizontal control clipping. Initial third-row access is not demonstrated by this image alone.

### 09_zoom125_third_card_edges.png

- Original dimensions: 2360 × 2000.
- SHA-256: `0b47a6d2f923f62967f6b3cb5a3b23100f9ffefca63aa81a3ed34be8caf91b6e`.
- Visible evidence: Wide 1.25x after selection: complete selected Card 3 row including full selector, title, identity, 0.875 s / 300 bpm click / 3/4 and Clear is visible above the distinct transport panel. Card 2 row/envelope also visible. Matching transport reads Record Selected Card 3 with visible record/play/stop/rewind, tempo and click/replace controls. Status lines and full footer visible. No visible control collision, black strip or horizontal clipping.

### 10_zoom125_transport_boundary_edges.png

- Original dimensions: 2360 × 2000.
- SHA-256: `a4607cdbc9e5e0e90267dcd161af204c0af6c0e75287b8266f6ffc6855cb034c`.
- Visible evidence: Wide 1.25x repeated-wheel boundary: full selected Card 3 row remains exposed above full matching transport. Card 2 envelope is visible; upper Card 1 is clipped at the table viewport boundary. All transport controls, status lines and footer fit. No black strip or row/transport/footer overlap observed.

### 11_zoom125_recovered_envelope_edges.png

- Original dimensions: 2360 × 2000.
- SHA-256: `9b2c9c5207e98b04ba1b5282b40c44958edadca42159dc61a16bef99615e609a`.
- Visible evidence: Wide 1.25x recovery: Card 2 is selected again with complete stepped envelope, 1.125 s / 180 bpm click / 3/4 and Clear. Transport follows Card 2 with matching timing and Record Selected Card 2. Header/search, statuses and footer visible. Card 3 returns to a thin sliver at this scroll position. No black strip. Visible waveform recovery is supported; exact PCM equality is a separate verifier result.

## Receipt observations, separate from pixels

I read the frozen scenario, its completion text, `completion.json` and `supplier.json`. `scenario.done` records `RESULT ok`, 11 captures, zero blank, 11 distinct digests, 437 presentation frames, 438 redraws and one unpresented redraw. The scenario contains available-and-zero document X/Y assertions before and after wheel/selection operations, pre-click selector/Clear assertions, individual transport visibility assertions at boundary positions, and envelope verification after recovery. These are executed-receipt claims; still pixels cannot measure a document scroll offset or prove the input path on their own.

`completion.json` records all four fields true: `scenario_passed`, `owned_app_exited`, `owned_keeper_exited`, and `original_fixture_files_unchanged`. I did not launch or inspect process state independently, nor rehash the underlying profile fixture; these exit and fixture-preservation claims are attributed to that receipt.

## Remaining visual limits

- At the initial compact position (02), transport controls are below view. At initial short workspace position (05), Card 3 and transport are below view. The subsequent positions establish access rather than simultaneous visibility of everything.
- At short Card 3 position (06), the lower transport is clipped. At short transport position (07), the Card 3 selector/title are clipped above. No single short PNG demonstrates full Card 3 and all transport controls simultaneously, and no such simultaneous fit is claimed.
- At initial/recovered zoom positions (08/11), Card 3 is a sliver. Images 09/10 independently expose it. Other row fragments also clip at viewport edges during scrolling.
- Complete eight-region envelope structure is visible in 01/02/03/04/05/08/09/10/11: near-flat start, very low, medium, tallest, lower, second-tallest, very low, near-flat end. No flattening or obvious horizontal waveform truncation was observed. Images 06/07 cannot qualify the envelope because it is outside view. Numeric amplitudes, exact sample identity and sound quality are not inferred from pixels.
- No horizontal clipping of the reviewed core controls or visible collisions between exposed card controls, transport and footer were found. This does not establish all display scales, resolutions, accessibility modes, gestures or transient frames.
- No refusal UI is shown, so refusal copy is outside this image review.

Only this external review file was written. No product, supplier, build, Cargo, Git, scenario or native-process mutation was performed.
