# Ringdown partition: complete source audit

Frozen source revision: d12201bfe80cc94345c6308c46caaebf4a9fe42e.
Snapshot root: /Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf.
Audit date: 2026-10-16. Scope: read-only architecture/correctness review, especially duplicated authority, unnecessary parallel implementations, and misleading success or evidence paths.

## Completion and evidence limits

**Complete: all 23 assigned files, all 11,066 assigned lines, 392,569 bytes.** Every assigned source, test, manifest, formatting-policy and lockfile line was displayed and read. None of the assigned-file reads was truncated. The companion [ringdown-coverage.json](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/ringdown-coverage.json) records each exact read chunk, full assigned and independently recomputed SHA-256, line count, and complete status. Initial broad context/search output was truncated; the relevant context was reread in bounded chunks. No coverage is claimed from truncated search output.

All assigned hashes matched the assignment before and after review. All 23 corresponding Ringdown files in the primary checkout were also byte-identical to this snapshot at the read-only comparison. The three additional woodshed-instrument source/example files cited below also matched. **No finding below is counted as already fixed by dirty local P5a.** Its dirty-path inventory showed audio/core/host/view work; no assigned Ringdown path was changed. This is a comparison at audit time, not a claim about later concurrent edits.

No source, product documentation, lockfile, profile, commit or Git state was changed. Only the two authorized external audit artifacts were written. No builds, tests, native execution, network browsing or hardware operations were performed. Findings are source-proven execution consequences; historical hardware reports in plans were read as context and were not requalified.

Repository guidance read: root CLAUDE.md (1–117), root design_docs/DOC_README.md (1–413), Ringdown's documentation index/policy, its complete client-surface plan and effects handoff, relevant founding/phase/H47 sections, the rehome ruling, and relevant smart-instrument W1–W3 sections. Additional caller reads and their exact bounds are in the coverage artifact. The 2,605-line founding history was read selectively; it is not part of the assigned 11,066-line completeness claim.

## Authority map and reachability

| Concern | Intended authority | Verified implementation and conclusion |
|---|---|---|
| Firmware wire format, vocabulary, checksums, typed edit/model | Ringdown sans-io core | Effect and method tables are generated once. Profile applies the same Edit that plans the wire call. These are useful shared owners, not redundant copies. |
| Request/reply correlation, writes, deadlines and device refusal | ringdown-client | Guitar drives a Link; BLE supplies platform I/O. The split genuinely removes btleplug from the protocol driver. |
| Discovery and live Bluetooth connection | ringdown-ble | Platform shell creates Guitar over BtleplugLink. Its public discovery/connection paths are used by both probe and woodshed-instrument. |
| Practice-side authority and metronome policy | woodshed-instrument | Detached/Follow/Drive is product policy. Deliberately omitting denominator writes is documented product policy, not an obsolete protocol implementation. |
| Evidence interpretation and file persistence | probe | The CLI adds human-readable conclusions and local saves. Several conclusions outrun what the lower-level result actually establishes. |
| Product boundaries | Independently released nested workspace | Root Cargo.toml excludes ports/ringdown; root woodshed-instrument intentionally consumes its local libraries. This is the explicit rehome design, not a silo to flatten. |

The probe reaches Guitar through discover → connect → session. Woodshed's root library reaches it through Connection::open and exposes additional operations through Connection::guitar. Source-wide Rust caller search found no application caller of restore_bank/push_bank or Guitar::edit beyond Ringdown's tests and the internal restore composition. They are public, callable library features, but their defects below are **latent library gaps, not a claim that the Woodshed GUI currently exercises restoration**.

The smart-instrument plan explicitly says W3/product UI remains open (design_docs/2026-08-27_smart_instrument_plan.md:4–7). The root source caller search agrees: the seam and its example exist, but no woodshed-views integration was found. That planned state is not reported as dead-code cruft.

## Findings

There are seven direct Ringdown P2 findings, two P3 consolidation findings, and one additional cross-partition P2 acceptance finding for the parent to reconcile with the instrument owner. No P0/P1 finding is asserted.

| ID | Severity | Finding | Reach |
|---|---|---|---|
| R1 | P2 | Restore discards negative re-push receipts after destroying the prior chain | Public library; no current product caller found |
| R2 | P2 | Malformed non-ASCII hex panics instead of returning a shape error | Live file retrieval, probe and root instrument seam |
| R3 | P2 | Sparse status replies become fabricated observed values | Probe and root instrument status read |
| R4 | P2 | Failed download persistence still produces process success | Probe --fetch |
| R5 | P2 | Driver accepts LLT2 acknowledgements without validating their wire identity | Live multi-frame send |
| R6 | P2 | Capture decoder loses connection identity before interpreting ATT | Probe --decode-pklg with multiple connections |
| R7 | P2 | Uncheckable loop metadata can be summarized as every loop matching | Probe --index |
| R8 | P3 | Firmware wedging-method fact remains duplicated in Woodshed | Cross-partition guard cleanup |
| R9 | P3 | Recording-name parsing is duplicated and already has different contracts | Driver latest vs probe index |
| X1 | P2 | Instrument hardware example returns success after a read-back mismatch | Additional cross-partition acceptance evidence |

### R1 — Preserve failed re-push receipts in the restore result

**Source:** [ringdown-client/src/lib.rs:754](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/crates/ringdown-client/src/lib.rs:754), specifically 754–771 and 774–795; Sent::parsed is 243–248 and typed send is 582–587.

**Trigger and effect:** Call restore_bank on a known populated slot. Let RemoveEffect yield true until the chain is empty, then false; let one or more AddEffect/SetBankName/SetGainBank re-push replies be false. False is a successful RPC exchange and is deliberately represented as Sent.reply, not a transport error. push_bank retains those Sent values, but restore_bank immediately takes only the vector length. It returns Ok(Restored { drained, expected, pushed }) indistinguishably from the all-true case, after the old effects are already gone. The caller cannot identify which restoration writes were refused.

The docs promise “make slot hold exactly what bank says”; the outcome conveys request counts rather than restoration acceptance. This overstates the operation even within Ringdown's careful “parsed, never confirmed applied” model. Independently, push_bank sends chain/name/gain/sustain but omits BankSpec.fbk_onoff and fbk_params (effects.rs:396–407), so its “everything bank holds” wording is broader than its actual field coverage. That field gap should be made explicit before any exact-bank restoration claim.

**Authority owner / competing interpretation:** Sent is the canonical raw receipt owner; restore_bank's count-only Restored projection overrides it with an undifferentiated completed operation. There is no competing product bank store.

**Current tests/gap:** restore_bank_drains_to_false_then_pushes_in_h33_order (1333–1380) scripts only true replies during the push and checks method order/count. The typed-write test (1215–1235) correctly preserves false at the lower level, demonstrating the seam that restoration loses. No rejected-push or incomplete-field restore case exists in this file.

**Smallest coherent direction:** Keep restore as an explicitly destructive attempt and retain each Sent plus the planned operation in its receipt; stop or mark incomplete on literal false/unexpected reply. Distinguish attempted, parsed, and independently verified restoration. Either narrow the restore contract to the supported chain/name/gain/sustain fields or add protocol-supported feedback restoration with its own receipts. Do not create a second profile authority or infer that true proves audible restoration.

**Reachability limit:** Public API and internal composition are reachable; repository source search found only the focused test as an external caller. This is a library acceptance gap awaiting adoption, not a present GUI failure.

### R2 — Decode file hex without slicing through UTF-8 characters

**Source:** [ringdown-client/src/lib.rs:204](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/crates/ringdown-client/src/lib.rs:204), 204–212; read_file_range invokes it at 864–881.

**Trigger and effect:** A DumpFile reply with result string “aé0” is valid JSON and has four UTF-8 bytes, so the even-length check passes. The first text[0..2] slice ends inside é and panics before from_str_radix can return None. The public retrieval API promises a TransportError::Shape for a non-hex reply but can instead unwind the consumer. No large input or allocation pressure is required.

**Authority owner / competing implementation:** The driver owns hex-to-byte admission for file data. Probe render_maybe_hex (main.rs:1061–1071) separately checks every character is ASCII hex before slicing, while the reusable driver lacks that prerequisite. The safer rendering helper does not protect the actual retrieval path.

**Callers:** probe --fetch uses read_file/read_file_range (main.rs:843–875); woodshed-instrument::Connection::fetch_recording delegates to read_file (crates/woodshed-instrument/src/lib.rs:278–286). Thus this is not confined to an unused helper.

**Current tests/gap:** hex_replies_decode_and_bad_ones_are_refused (client 1111–1118) covers valid ASCII, odd length, and ASCII non-hex. It omits even-byte-length Unicode input.

**Smallest coherent direction:** Iterate ASCII byte pairs and decode nibbles, or reject non-ASCII/non-hex before any string indexing. Have diagnostic rendering consume that same decoder rather than maintaining a second admission rule. Preserve Shape errors for malformed replies.

### R3 — Keep absent status fields absent through the consumer projection

**Source:** [ringdown/src/rpc.rs:294](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/crates/ringdown/src/rpc.rs:294), 294–325; [woodshed-instrument/src/lib.rs:208](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-instrument/src/lib.rs:208), 208–219; probe main.rs:388–400.

**Trigger and effect:** A matching GetStatus response with result {} is accepted by Guitar::status. Every Status field has serde(default), yielding empty identifiers/firmware and zero battery/storage values. Connection::read_status wraps all of those in Some, presenting them as fields actually observed from the instrument. The probe prints them and announces “THE PROTOCOL MAP IS CONFIRMED AGAINST HARDWARE”.

This directly defeats the consumer's stated Option contract: None means not read and a confident zero must not be invented (instrument lib.rs:11–14 and 134–154). A sparse firmware reply can therefore become a false 0% battery reading and false complete-status/control receipt. Even a complete GetStatus controls only the exercised exchange; it cannot independently verify every bank/effect/method fact.

**Authority owner / competing interpretation:** The protocol decode discards field presence. The product projection then labels defaults as instrument-reported facts, and the probe adds a stronger all-protocol conclusion. Presence must survive until each owner can decide how to display incomplete information.

**Current tests/gap:** a_sparse_status_still_decodes (rpc.rs:1007–1015) explicitly asserts default battery zero. The instrument test unread_state_is_empty_rather_than_zero (405–412) checks only an untouched default InstrumentState, not a sparse actual reply mapped through read_status. The captured complete-status fixture is useful but does not cover this seam.

**Smallest coherent direction:** Preserve presence in a decoded status representation or alongside the compatibility Status view; map absent fields to None in InstrumentState. Define minimum live-control evidence separately and print an incomplete status honestly. Scope probe confirmation to the observed connection/banner/GetStatus exchange. Do not make the host duplicate JSON field parsing to recover presence already lost by the protocol layer.

### R4 — Propagate local save failure from --fetch to the process result

**Source:** [probe/src/main.rs:884](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/apps/probe/src/main.rs:884), 884–890; main dispatch 290–294, session 473–475, and run 353–357.

**Trigger and effect:** Run --fetch on an existing recording from a directory where the chosen output basename cannot be written, or let the disk reject the write. The transport and checksum may succeed. std::fs::write then fails, but fetch_recording prints “could not write”, returns Ok, and the top-level process exits successfully. A caller using the exit code to qualify a completed download receives success with no saved artifact.

The ReadConfig-to-file branch has the same swallowed local-write shape at main.rs:538–543, though real ReadConfig is already known to wedge this firmware; --fetch is the independently reachable practical case. run also ignores disconnect failure and prints “disconnected” unconditionally (355–356), another weaker receipt claim.

**Authority owner / competing interpretation:** Filesystem completion owns whether the recording was saved. The CLI's Result/exit status suppresses that failure while showing the stronger checksum receipt.

**Current tests/gap:** The probe's tests (1381–1419) exercise only recording-path splitting. There is no injectable filesystem error or process-outcome test for a failed output write.

**Smallest coherent direction:** Return a file-write error from the requested persistence action, then preserve the existing always-attempt-disconnect composition before returning a nonzero result. Keep “checksum verified” and “saved file” as separate outcomes; report disconnect as attempted or confirmed according to its actual Result. An exploratory multi-call command may continue after individual failures, but its aggregate outcome should be explicit.

### R5 — Make Ack2 itself enforce the acknowledgement shape

**Source:** [ringdown/src/llt2.rs:122](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/crates/ringdown/src/llt2.rs:122), 122–144; client await_llt2_ack at lib.rs:960–979; send_llt2 at 521–541.

**Trigger and effect:** During an LLT2 frame wait, a notification such as [0x58, 0x01, expected_oid, frame_lo, frame_hi, 1, 0xaa] passes Ack2::parse and answers the current frame. The function requires only length >= 6 and a known status byte; it checks neither exact acknowledgement length, transfer type 'J', nor reserved zero. answers then compares only object id and frame. The driver consumes this non-ack as a successful acknowledgement and advances the transfer.

This is a malformed/unrelated-notification trigger, not a claim that the reference guitar normally emits such bytes. The defect is concrete in the parser's documented demultiplexing contract.

**Authority owner / competing implementation:** The protocol core should decide whether bytes are an Ack2. The offline probe compensates with 'J' and exact-six-byte guards (pklg.rs:254–257), while the live driver trusts Ack2 directly. Those call sites therefore have different protocol admission rules.

**Current tests/gap:** Ack tests (llt2.rs:436–460) cover short input and an unknown status, but not wrong transfer/reserved bytes or extra payload. a_compressed_reply_is_not_mistaken_for_an_ack (463–474) only asserts its first byte differs from 'J'; it never calls Ack2::parse and therefore does not test the advertised parser distinction.

**Smallest coherent direction:** Validate the complete fixed acknowledgement envelope in Ack2::parse and include transfer type in correlation, then let both live driver and capture decoder use it. Add exact wrong-type/extra-length negative fixtures at the core seam rather than duplicating defensive prefix checks in consumers.

### R6 — Retain connection identity across capture reassembly and ATT interpretation

**Source:** [probe/src/pklg.rs:106](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/apps/probe/src/pklg.rs:106), 106–147; decode_file 32–44; AttDecoder state 151–164, prepared writes 211–228, and LLT2 accumulation 266–275.

**Trigger and effect:** Decode one PacketLogger capture containing two BLE connections with overlapping ATT handles or LLT2 object ids. AclReassembler correctly separates fragments by (connection handle, direction), then returns only PDU bytes. decode_file feeds all of them into one AttDecoder without connection identity. Characteristic names and prepared writes are keyed solely by ATT handle; LLT2 frames by (direction, low-byte object id). Names can be reassigned across devices, prepared writes can mix or be flushed by another connection's Execute Write, and transfer frames can mix or poison later decoding.

The summary then lists these messages as methods “the app sent” without preserving which connection/instrument they came from. For a tool whose output establishes protocol findings, this can contaminate provenance even when individual JSON decoding succeeds. A missing/incomplete transfer followed by reuse of its low-byte object id also lacks a frame-1 reset and remains concatenated with the older pending transfer.

**Authority owner / competing interpretation:** Connection/transfer identity belongs to the reassembly pipeline. ACL owns it initially, but the ATT layer invents a global handle/object-id namespace after it is dropped. This is actual conflicting identity scope, not product-vs-platform policy.

**Current tests/gap:** acl_fragments_reassemble_into_one_att_pdu (407–425) covers a single connection. Other decoder tests cover one plain/compressed write or ack. No two-connection same-handle fixture, prepared-write isolation, object-id reuse after truncation, or mixed-transfer rejection exists. The core llt2::reassemble (277–292) concatenates payloads without validating total length, transfer identity or frame sequence, so it does not repair the lost scope.

**Smallest coherent direction:** Return connection handle with each reassembled ATT PDU and partition AttDecoder state by connection/session. Keep characteristic handle and object id inside that scope. On a new first frame, explicitly start or reject a new transfer, and validate sequence/declared total before presenting a decoded message as complete. Expose incomplete/corrupt capture status in the receipt. This can stay in the offline tooling plus a small shared LLT2 validator; it does not justify adding capture logic to the Bluetooth shell.

### R7 — Count uncheckable loop headers separately from matching ones

**Source:** [probe/src/main.rs:727](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/apps/probe/src/main.rs:727), 727–742 and 759–798; core check_length at loopfile.rs:244–264 and nominal_samples at 139–143.

**Trigger and effect:** --index receives a syntactically valid vendor header whose metadata has tempo_bpm = 0. Parsing preserves it. nominal_samples/check_length/length_agrees return None, correctly declining to decide. The probe increments disagreed only for Some(false), still adds that metadata to seen, and prints “every loop's audio matches what its header implies” when disagreed is zero. Thus an unknown result is promoted to agreement. The same global claim ignores unreadable files logged at 744 and device-error skips at 701–703 when at least one other loop was seen.

**Authority owner / competing interpretation:** LoopHeader::check_length owns the three-way result: agreement, disagreement, or unavailable. The probe collapses unavailable/skipped into its no-disagreement success conclusion.

**Current tests/gap:** The core has a zero-tempo test (724–736) proving the result is None. Probe tests cover path splitting only and do not exercise the summary with uncheckable, unreadable, or skipped headers.

**Smallest coherent direction:** Carry checked/matched/disagreed/uncheckable/unreadable/skipped counts into the library summary. Say all checked loops match only for the checked subset, and claim all indexed loops match only when each expected item was actually checked. Keep the core's Option result intact.

### R8 — Consolidate the firmware wedging fact while preserving any absolute host policy

**Source:** [ringdown-client/src/lib.rs:215](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/crates/ringdown-client/src/lib.rs:215), 215–225 and 466–474; [woodshed-instrument/src/lib.rs:40](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-instrument/src/lib.rs:40), 40–48 and 289–297.

**Trigger and effect:** Connection::call("ReadConfig", ...) is rejected by the host's own exact-case list and error type; the same spelling in other case passes that list and is rejected by the driver's case-insensitive list. Any later firmware guard correction now has two tables and tests to maintain. The driver already re-exports its list through ringdown-ble (BLE lib.rs:27–30). The client-surface plan explicitly calls Woodshed's list redundant (plan:112–113).

**Authority owner / competing owner:** The driver owns the firmware fact. Woodshed may legitimately own a stricter policy that never invokes the research override. That is a valid reason to retain a product-level refusal boundary, but not to repeat the literal firmware inventory.

**Current tests/gap:** The client tests actual typed/named refusal before any writes (1183–1213). Woodshed's named test (instrument lib.rs:435–440) merely asserts that its local list contains the local literal; it does not exercise the guarded call path.

**Smallest coherent direction:** Reuse the driver's canonical wedging-method classification in any absolute host guard, or remove the redundant wrapper classification if driver policy is sufficient. Preserve the host's chosen prohibition if intentional. Do not infer that product authority requires its own copy of a firmware fact.

**Limit:** No ordinary default-path guard bypass was found; the driver's refusal still protects the case-sensitive host miss. This is P3 cruft/maintenance divergence, not a claim that lower-case ReadConfig reaches hardware.

### R9 — Share the recording-name grammar instead of maintaining two parsers

**Source:** [ringdown-client/src/lib.rs:187](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/crates/ringdown-client/src/lib.rs:187), 187–202, consumed at 839–847; [probe/src/main.rs:801](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/ringdown/apps/probe/src/main.rs:801), 801–816, consumed at 678–695.

**Trigger and effect:** GetLastRecordingName returns an extensionless numbered name such as “loop0032”. Driver latest_recording_name accepts it and constructs a predecessor; probe --index rejects it because its separate parser requires a dot and returns Ok after saying it cannot read the loop number. The same device naming fact therefore has two contracts. Both independently implement trailing-digit splitting and have separate small tests.

**Authority owner / competing owner:** The reusable driver currently owns next/latest recording semantics; the probe's private parser owns a competing subset of the same device naming rule.

**Current tests/gap:** The driver tests ordinary numbered/unnumbered cases at 1094–1109. Probe tests (1385–1418) explicitly pin extensionless refusal. There is no shared grammar to prevent future Unicode/path/extension/numbering corrections from drifting.

**Smallest coherent direction:** Put the device recording-name interpretation in one existing Ringdown core/client module and have both latest and index use it, or expose an indexed-name operation from the driver. Choose the actual supported grammar from device facts, rather than preserving both contracts. This needs no new general path crate.

**Limit:** The observed device names in current fixtures end in .wav, so the divergent trigger is not asserted as an existing hardware regression. The duplication is established; current fixture impact is bounded.

### X1 — Cross-partition: the metronome hardware example does not fail its own read-back gate

**Source:** [woodshed-instrument/examples/metronome_link.rs:94](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-instrument/examples/metronome_link.rs:94), 94–126 and main at 20–27.

**Trigger and effect:** The write request returns an RPC value but the instrument's later bpm differs from target.bpm. The example prints MISMATCH, proceeds to the restoration attempt, returns Ok if those I/O calls complete, and main prints “metronome link confirmed against hardware” with exit 0. It checks neither the target numerator nor equality of the restored state. Thus its executable acceptance result can certify the run that its own readback says failed.

**Authority owner / competing interpretation:** Instrument readback owns the observed state; the example's overall success result ignores it. This mirrors the evidence projection problem in the probe, but lives outside the assigned Ringdown partition and is recorded for the parent/instrument owner to reconcile.

**Current tests/gap:** The example's assertion of Pushed and AlreadyMatched compares results produced by its own session logic (84–92), not device state. Session::sync_metronome caches a sent request and labels an identical later request AlreadyMatched without a read (session.rs:149–155); its tests at 292–303 only assert cache values. Connection::set_metronome also drops the raw RPC result (instrument lib.rs:234–238). These explain why the independent readback must gate acceptance rather than being informational.

**Smallest coherent direction:** Record readback mismatch as a failure, still attempt restoration/release, compare the actually supported bpm/numerator fields, verify restoration, and return a failing aggregate outcome. If AlreadyMatched is intended to mean only “same desired request as last sent,” name/document that weaker fact; reserve live matching for a readback receipt. Do not change the deliberate policy of omitting denominator writes.

## Other cleanup and false-positive rebuttals

- **Stale protocol limitations remain in current API prose.** rpc.rs:101–104 and 588–596 still say AddBank-created banks never render and creation remains unknown; plan.rs:53–56 repeats that limit; effects.rs:372–378 still says whether feedback fields are needed is the next experiment. H47 (founding plan:660–745) retracts that claim, and plan::add_bank (174–184) plus the nonzero default gain already implement the correction. The README likewise says creation is not worked out (30–31). This is documentation cruft across current surfaces, not evidence that AddBank code is broken. Update the public docs to cite the same scoped H47 ruling instead of copying the earlier conclusion.
- **Independent locks/manifests are deliberate.** The entire nested lockfile and all manifests were read. Separate product release graphs are required by CLAUDE.md:74–77 and the rehome ruling. No port-to-port dependency or accidental root workspace membership was found in this partition.
- **Compatibility re-exports are not duplicate implementations.** rpc.rs:528–534 re-exports the canonical effects module. It does not contain another effect model.
- **Shadow persistence versus wire serialization is deliberate.** BankSpec stores product-friendly field names and builds ordered wire keys separately. A Profile is explicitly a belief/last-sent record, not a competing readback database. Its presence is justified by unavailable bank readback.
- **Two transport generations are deliberate protocol support.** LLT and LLT2 have materially different formats and a version gate; they are not interchangeable copies to merge.
- **No normal typed-write “true means applied” overclaim was found.** Sent and parsed() are carefully named and documented. R1 is the higher-level point that loses that care, and X1 is the acceptance harness that ignores its independent check.
- **No claim of general inbound framed-reply support is made.** The live driver's await_response decodes bare compressed/plain replies. The core documents inbound framed replies as unobserved and not handled by the vendor app (llt2.rs:270–276). I did not turn that explicitly bounded capability into a present hardware defect.
- **The codec is not supported only by self-roundtrips.** It also has a byte-exact captured ReadBank regression (compress.rs:860–878). Other format/property tests and corpus tests are useful within their stated scope. Stale “unverified” module comments should be reconciled, but the captured fixture should be retained.
- **Small display helpers need not become a stack package.** The two probe hexdump renderers overlap, but that alone is low-impact local cleanup. R2 identifies the actual correctness seam worth sharing first.
- **No build graph, runtime, physical audio or fresh-process acceptance claim was made from this read.** Earlier plans/receipts do not establish those states at the frozen revision.

## Unresolved questions for consolidation

1. Is restore_bank intended for imminent adoption, or should its incomplete attempt/receipt contract be made explicit before the first product consumer? Current source search found no non-test external caller.
2. Does Woodshed want an absolute ReadConfig prohibition even when its exposed Guitar has the research override enabled? That policy is legitimate; the duplicated literal inventory is unnecessary either way.
3. Should a sparse status be accepted as partial information or refused as an invalid live-control receipt? Preserve presence first so the two uses can make that choice without inventing values.
4. Which exact partial/corrupt/multiple-connection capture states should make --decode-pklg fail versus return an explicitly incomplete receipt? Current capture parser can also stop at a bad later record and return the prefix as Ok (pklg.rs:83–103); the report does not assume prefix salvage itself is wrong.
5. The library's integer nominal-sample calculation floors before block rounding (loopfile.rs:139–153). Its corpus fixtures support the recorded cases; whether a fractional target just above a block boundary should round up another block is a hardware/formula edge requiring its own evidence, not a proven defect from this audit.
6. The parent should reconcile X1 and R8 with the reviewer owning woodshed-instrument. They are cross-boundary evidence, not a second independent instrument review.

## Assigned file ledger

Full SHA-256 values and exact read chunks are in the machine-readable coverage file. Every row below is complete and hash-verified.

| Assigned file, relative to snapshot root | Lines | Status |
|---|---:|---|
| ports/ringdown/Cargo.lock | 1015 | complete |
| ports/ringdown/Cargo.toml | 25 | complete |
| ports/ringdown/apps/probe/Cargo.toml | 21 | complete |
| ports/ringdown/apps/probe/src/main.rs | 1419 | complete |
| ports/ringdown/apps/probe/src/pklg.rs | 470 | complete |
| ports/ringdown/crates/ringdown/Cargo.toml | 25 | complete |
| ports/ringdown/crates/ringdown/src/compress.rs | 900 | complete |
| ports/ringdown/crates/ringdown/src/crc32.rs | 153 | complete |
| ports/ringdown/crates/ringdown/src/effects.rs | 652 | complete |
| ports/ringdown/crates/ringdown/src/handshake.rs | 336 | complete |
| ports/ringdown/crates/ringdown/src/lib.rs | 130 | complete |
| ports/ringdown/crates/ringdown/src/link.rs | 98 | complete |
| ports/ringdown/crates/ringdown/src/llt.rs | 489 | complete |
| ports/ringdown/crates/ringdown/src/llt2.rs | 476 | complete |
| ports/ringdown/crates/ringdown/src/loopfile.rs | 737 | complete |
| ports/ringdown/crates/ringdown/src/plan.rs | 541 | complete |
| ports/ringdown/crates/ringdown/src/profile.rs | 436 | complete |
| ports/ringdown/crates/ringdown/src/rpc.rs | 1280 | complete |
| ports/ringdown/crates/ringdown-ble/Cargo.toml | 24 | complete |
| ports/ringdown/crates/ringdown-ble/src/lib.rs | 384 | complete |
| ports/ringdown/crates/ringdown-client/Cargo.toml | 27 | complete |
| ports/ringdown/crates/ringdown-client/src/lib.rs | 1409 | complete |
| ports/ringdown/rustfmt.toml | 19 | complete |
| **Total: 23 files** | **11066** | **complete** |
