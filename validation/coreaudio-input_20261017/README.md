# CoreAudio input admission — 2026-10-17

Status: authorization-first implementation passes software checks; committed native selected-Card recording and sealed fresh reopening are pending.

Baseline: Woodshed `4d9434a187e4eea2d230f779c80a47a39a31125f`.

The input worker now resolves macOS microphone authorization through AVFoundation before entering CPAL/HAL. An explicit consent wait is visible and excludes the hardware-opening deadline. After authorization, the existing ten-second bound starts at the worker's actual hardware-open timestamp. Denied, restricted or unknown states refuse without opening input. Stop/reset and frozen capture authority still prevent a late result from resurrecting a cancelled take.

The device remained the iMac Microphone, mono 48 kHz, alive and without a hog owner. Fresh GUI diagnostic probes reported undetermined permission and stalled assigning the HAL input device. System TCC logs identified pending microphone consent. An authorization-first probe received the user's approval, changed from status 0 to 3, and completed device registration in 20 ms. A CPAL-only probe that had timed out while consent requests were pending then opened its input stream in about 59 ms, received 96,256 samples in two seconds and closed normally. No raw audio was stored by these probes. These controls support consent waiting as the observed stall mechanism; they do not claim all future HAL failures share that cause.

The bounded source change adds one registry package, `objc2-av-foundation 0.3.2`, using already-locked Objective-C/block bindings and only authorization/media-type features. Existing dependency package records and all Git supplier revisions are retained. Four unrelated broad-range edges rewritten by Cargo were restored from the baseline and the locked gate passes.

Software checks: 700 affected cases, with repeated suites counted once; five capture child invocations are separate. Seventeen admission cases include waiting beyond ten seconds without HAL opening, authorization refusal, cancellation followed by late grant, native timeout, rate/quota/output/backing guards and worker-owned native teardown. Device-free tests never request OS permission.

Native acceptance must still require at least fifteen monotonic seconds and 720,000 output frames of actual recording on Card 2, exactly one 54,000-frame take, matching owner and explicit Seconds clock, sealed decoded float bits, and byte-identical native-worker/reopened-worker/separate-reference WAVs. Permission waiting and count-in earn no active-recording time.

Apple's [authorization guidance](https://developer.apple.com/documentation/avfoundation/requesting-authorization-to-capture-and-save-media) and [authorization status API](https://developer.apple.com/documentation/AVFoundation/AVCaptureDevice/authorizationStatus(for:)) describe the explicit consent boundary. Precise round-trip latency and physical listening/input judgments remain separate from these receipts.
