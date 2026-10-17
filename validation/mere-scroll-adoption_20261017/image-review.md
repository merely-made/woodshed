# Native image review — 2026-10-17

Source: committed Woodshed `6b2c6b3e48cd7c029830a0928cf1b8d1afa24fcf`, main-based Mere `2d95d95e9`. Accepted trial is native-v2. Trial v1 is excluded. Reviewers operated read-only on the actual PNGs; they performed no builds, UI actions or source/Git mutations. The parent separately verified process, scenario, source and export receipts.

| Reviewer / lane | Coverage | Findings |
|---|---:|---|
| Planck / seed | 11/11 | Wide rows/envelope/transport/status/footer visible. Compact and short destination frames expose full Card 3/Clear and complete transport at their appropriate scroll positions. At 125%, Card 2’s stepped envelope and timing return intact. No blocking visual defect or black lower strip in these states. |
| Franklin / fresh reopen | 11/11 | Same envelope, owner and clock visible. Full controls and footer become visible across compact/short/125% matrix. No black lower strip or unpainted lower region. Both initial and final reopened metadata match seed except advancing output frame count; WAV hashes match. |
| Planck / P2 canvas | 9/9 | Overview, relationship and Related fit/zoom/wheel captures render; camera controls and adjacent lists remain readable and complete. Fit contains node markers; zoom/wheel can move graph content outside its own viewport. No blank canvas, black lower strip, or graph obscuring the camera controls. |

Intermediate compact/short frames intentionally clip lower rows or transport before a further scroll reveals them; the reviewed destination frames demonstrate full visibility. At short height, full Card 3 and full transport appear at separate positions. Metadata/Record wording wraps at 125% but remains visible. This is captured-state review; input delivery and continuous scenario progression are established by the separate native assertions.

Related Fit labels overlap: top Major Pentatonic/Minor 7, lower labels and the central/nearby Quartal Triad text. This limits graph readability and is retained as P8 polish. The parent compared the current image with the October 15 qualified capture and verified that older PNG against the SHA in its historical Git receipt; overlap predates this adoption. See [baseline comparison](related-label-baseline.json). Relationship images show a deliberately scrolled page position: the complete camera row/canvas is visible, while some earlier controls are above the current clip. No broader all-controls-at-once claim is made.

The parent also inspected key compact/short recorder destination images, final overview wheel, and historical/current Related Fit images. This review adds no microphone, acoustic, physical mouse/trackpad, other-platform, or whole-product layout qualification.
