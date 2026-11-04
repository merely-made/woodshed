use super::*;

/// Actual microphone PCM is unpredictable: report() verifies exact sealed and
/// backend bits, and these receipts retain only its digest for fresh reopening.
pub(super) fn physical_receipt(ui: &UiState, shared: &mut Shared, reopen: bool) -> bool {
    let result = (|| -> Result<(), String> {
        let mut actual = report(ui, shared)?;
        let takes = actual["captures"].as_array().ok_or("no capture report")?;
        let references = actual["manifest"]["takes"]
            .as_array()
            .ok_or("no sealed references")?;
        if actual["owner"] != 1
            || actual["playing"] != false
            || actual["rate"] != 48_000
            || actual["streams"] != 1
            || actual["requested_ready"] != true
            || actual["admitted"] != actual["requested"]
            || actual["phrase_segments"] != 2
            || actual["wav_frames"] != 132_000
            || actual["selected"] != json!({"set":1,"card":2})
            || actual["selected_index"] != 1
            || actual["selected_has_capture"] != true
            || takes.len() != 1
            || references.len() != 1
        {
            return Err("physical receipt requires stopped admitted mixed backing at 48 kHz, with exactly one selected Set 1/Card 2 take".into());
        }
        let take = &takes[0];
        let reference = &references[0];
        if take["id"] != json!({"set":1,"card":2})
            || take["frames"] != 54_000
            || take["rate"] != 48_000
            || take["seconds"] != 1.125
            || take["bars"] != 0
            || take["bpm"] != 180.0
            || take["beats"] != 3
            || take["legacy_clock"] != false
            || take["sealed_pcm_exact"] != true
            || actual["manifest"]["version"] != 4
            || reference["id"] != take["id"]
            || reference["clock"] != woodshed_core::loop_capture::SECONDS_CLOCK
            || reference["seconds"] != 1.125
            || reference["bars"] != 0
            || reference["frames"] != 54_000
            || reference["sample_rate"] != 48_000
            || reference["bpm"] != 180.0
            || reference["beats"] != 3
        {
            return Err(
                "physical take owner, 54000-frame PCM or explicit Seconds clock differs".into(),
            );
        }
        let directory =
            PathBuf::from(std::env::var_os("WOODSHED_CAPTURE_DIR").ok_or("no receipt directory")?);
        let bytes = std::fs::read(directory.join("loop-export.wav")).map_err(|e| e.to_string())?;
        if woodshed_core::loop_capture::pcm_digest(&bytes)
            != actual["wav_digest"]
                .as_str()
                .ok_or("no reference WAV digest")?
        {
            return Err("physical WAV worker file differs from admitted-snapshot reference".into());
        }
        actual["worker_wav_exact"] = json!(true);
        if reopen {
            // Parent copies this immutable expectation into the fresh process's
            // receipt directory. Missing expectation is a failure, never reseed.
            let expected: Value = serde_json::from_slice(
                &std::fs::read(path("physical-final")?).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            for key in [
                "set",
                "owner",
                "looper",
                "saved_backing",
                "manifest",
                "captures",
                "selected",
                "selected_index",
                "selected_has_capture",
                "admitted",
                "backend_admitted",
                "requested",
                "requested_ready",
                "rate",
                "streams",
                "phrase_segments",
                "sealed_exact",
                "wav_frames",
                "wav_digest",
                "worker_wav_exact",
            ] {
                if expected.get(key) != actual.get(key) {
                    return Err(format!("physical reopened {key} differs from seed"));
                }
            }
            if actual["input_unopened"] != true {
                return Err("physical reopen must leave input unopened".into());
            }
        }
        write(
            if reopen {
                "physical-reopened"
            } else {
                "physical-final"
            },
            &actual,
        )
    })();
    if let Err(error) = &result {
        eprintln!("Physical capture receipt: {error}");
    }
    result.is_ok()
}
