# Listening annotation port receipts

## 2026-11-01: retained note readability

Base: `e1d93b0`. Shared note headings preserve the entire note body and put
copy-verification/navigation/privacy controls on a separate wrapping row.
`cargo test --locked -p redshank-surfaces --lib` in `ports/redshank`: 112 passed,
0 failed. Source review found no action-identity change. Turnstone consumer
layout and native interaction verification are separate, not established here.

Bounded run: 3 GiB memory, zero scope swap, CPU 150 percent, one build/test
worker, shared machine lock, 600-second cap. Actual run 45.99 seconds; maximum
RSS 754168 KiB, swaps 0. External record:
`Code/testing/custody-contract-20261029/followups-redshank-20261101-1.json`;
log SHA-256 `2fedce8a92f7a0b9ab89754dc933e2d09f22a36a87859a0aaa0e5cffeb0cc106`.
