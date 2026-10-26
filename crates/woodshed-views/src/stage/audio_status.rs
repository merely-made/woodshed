//! Product wording over transient host observations; controls retain user intent.
use super::UiState;
use woodshed_core::audio::AudioStreamStatus;

fn stream_status(status: &AudioStreamStatus) -> String {
    match status {
        AudioStreamStatus::Unavailable => "unavailable".into(),
        AudioStreamStatus::Unopened => "not opened".into(),
        AudioStreamStatus::Requested => "opening…".into(),
        AudioStreamStatus::AwaitingPermission => "waiting for microphone permission…".into(),
        AudioStreamStatus::Admitted => "stream admitted".into(),
        AudioStreamStatus::Offline => "offline engine; no device stream".into(),
        AudioStreamStatus::Failed(reason) => format!("failed: {reason}"),
    }
}

impl UiState {
    pub fn audio_output_status(&self) -> String {
        format!(
            "Audio output: {}",
            stream_status(&self.audio_availability.output)
        )
    }

    pub fn audio_input_status(&self) -> String {
        format!(
            "Audio input: {}",
            stream_status(&self.audio_availability.input)
        )
    }

    pub fn tuner_is_listening(&self) -> bool {
        self.audio_availability.tuner_active
            && self.audio_availability.input == AudioStreamStatus::Admitted
    }

    pub fn tuner_action_label(&self) -> &'static str {
        if self.tuner.enabled {
            "Turn tuner off"
        } else {
            "Start listening"
        }
    }

    pub fn tuner_status(&self) -> String {
        if self.tuner_is_listening() {
            return match &self.tuner.reading {
                Some(reading) => format!(
                    "{}{} {:+.0} cents{}",
                    reading.note,
                    reading.octave,
                    reading.cents,
                    if reading.in_tune { " · in tune" } else { "" }
                ),
                None => "Listening…".into(),
            };
        }
        if !self.tuner.enabled {
            return "Tuner is off".into();
        }
        match &self.audio_availability.input {
            AudioStreamStatus::Unavailable | AudioStreamStatus::Offline => {
                "Tuner unavailable: audio input is unavailable.".into()
            },
            AudioStreamStatus::Unopened => "Tuner requested; audio input has not opened.".into(),
            AudioStreamStatus::Requested => "Opening audio input…".into(),
            AudioStreamStatus::AwaitingPermission => "Waiting for microphone permission…".into(),
            AudioStreamStatus::Admitted => "Tuner requested; waiting for activation.".into(),
            AudioStreamStatus::Failed(reason) => {
                format!("Tuner unavailable: {reason}. Turn the tuner off and on to retry.")
            },
        }
    }
}
