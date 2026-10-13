// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Standalone native adapter. The reusable recorder never constructs an Engine.
use hocket_engine::{AudioDeviceSelection, Engine};
use hocket_recorder::{
    AudioAvailability, AudioObservation, CaptureCompletion, CaptureMode, CapturePhase,
    CaptureToken, LayerKey, RecorderAudio,
};

pub struct DesktopRecorderAudio {
    engine: Result<Engine, String>,
    accepted: Option<(CaptureToken, u32, CaptureMode)>,
}

impl DesktopRecorderAudio {
    pub fn new(receipt: bool) -> Self {
        Self {
            engine: if receipt {
                Err("audio inactive in appearance receipt".into())
            } else {
                Engine::new().map_err(|error| error.to_string())
            },
            accepted: None,
        }
    }
    pub fn restart(&mut self, selection: &AudioDeviceSelection) -> Result<(), String> {
        if self.accepted.is_some() {
            return Err("stop capture before changing audio".into());
        }
        self.engine = Err("restarting audio".into());
        self.engine = Engine::new_with_audio_devices(selection).map_err(|error| error.to_string());
        self.engine.as_ref().map(|_| ()).map_err(Clone::clone)
    }
    fn engine(&mut self) -> Result<&mut Engine, String> {
        self.engine.as_mut().map_err(|error| error.clone())
    }
}

fn key(value: LayerKey) -> hocket_engine::LayerKey {
    hocket_engine::LayerKey::new(value.track_id, value.layer_index)
}

impl RecorderAudio for DesktopRecorderAudio {
    fn availability(&self) -> AudioAvailability {
        match &self.engine {
            Ok(engine) => AudioAvailability {
                sample_rate: engine.sample_rate(),
                input: engine.input_available(),
                error: None,
            },
            Err(error) => AudioAvailability {
                error: Some(error.clone()),
                ..Default::default()
            },
        }
    }
    fn poll(&mut self) -> AudioObservation {
        let Ok(engine) = &mut self.engine else {
            return AudioObservation {
                error: self.engine.as_ref().err().cloned(),
                ..Default::default()
            };
        };
        let error = engine.tick().err().map(|error| error.to_string());
        let phase = match engine.pending_capture_progress() {
            hocket_engine::CapturePhase::Idle => CapturePhase::Idle,
            hocket_engine::CapturePhase::Waiting {
                bars_remaining,
                samples_until_next_bar,
            } => CapturePhase::Waiting {
                bars_remaining,
                samples_until_next_bar,
            },
            hocket_engine::CapturePhase::Recording { progress } => {
                CapturePhase::Recording { progress }
            },
            hocket_engine::CapturePhase::FreeRecording { samples_done } => {
                CapturePhase::FreeRecording { samples_done }
            },
            hocket_engine::CapturePhase::Complete => CapturePhase::Complete,
        };
        let samples = engine.take_bar_aligned_capture();
        let completed = samples.and_then(|samples| {
            self.accepted
                .take()
                .map(|(token, sample_rate, _)| CaptureCompletion {
                    token,
                    sample_rate,
                    samples,
                })
        });
        let retired_capture = if matches!(phase, CapturePhase::Idle) && completed.is_none() {
            self.accepted.take().map(|(token, _, _)| token)
        } else {
            None
        };
        AudioObservation {
            phase,
            meter_db: engine.peak_db(),
            completed,
            retired_capture,
            error,
        }
    }
    fn set_clock(
        &mut self,
        bpm: f32,
        numerator: u8,
        denominator: u8,
        click: bool,
    ) -> Result<(), String> {
        let engine = self.engine()?;
        engine
            .set_tempo(bpm, numerator, denominator)
            .map_err(|error| error.to_string())?;
        // Engine's click patch is infallible after successful tempo admission.
        engine.set_click_enabled(click);
        Ok(())
    }
    fn set_click(&mut self, enabled: bool) -> Result<(), String> {
        self.engine()?.set_click_enabled(enabled);
        Ok(())
    }
    fn begin_capture(&mut self, token: CaptureToken, mode: CaptureMode) -> Result<(), String> {
        if self.accepted.is_some() {
            return Err("capture is still owned by the audio adapter".into());
        }
        let engine = self.engine()?;
        let rate = engine.sample_rate();
        match mode {
            CaptureMode::Clocked { bars, count_in } => {
                engine.arm_bar_aligned_capture(bars, count_in)
            },
            CaptureMode::Free => engine.start_free_capture(),
        }
        .map_err(|error| error.to_string())?;
        self.accepted = Some((token, rate, mode));
        Ok(())
    }
    fn stop_free_capture(&mut self, token: CaptureToken) -> Result<(), String> {
        if self
            .accepted
            .as_ref()
            .is_none_or(|(owned, _, mode)| *owned != token || *mode != CaptureMode::Free)
        {
            return Err("free capture ticket is not current".into());
        }
        self.engine()?.stop_free_capture();
        Ok(())
    }
    fn play_layer(
        &mut self,
        layer: LayerKey,
        samples: Vec<f32>,
        sample_rate: u32,
        gain: f32,
        aligned: bool,
    ) -> Result<(), String> {
        let engine = self.engine()?;
        if aligned {
            engine.play_layer_at_next_bar_at_sample_rate(
                key(layer),
                samples,
                sample_rate,
                gain,
                true,
            )
        } else {
            engine.play_layer_at_sample_rate(key(layer), samples, sample_rate, gain, true)
        }
        .map_err(|error| error.to_string())
    }
    fn stop_layer(&mut self, layer: LayerKey) -> Result<(), String> {
        self.engine()?.stop_layer(key(layer));
        Ok(())
    }
}
