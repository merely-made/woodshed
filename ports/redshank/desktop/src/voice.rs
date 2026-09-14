//! Standalone-host microphone capture and durable voice blobs.

use std::{
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use cpal::{
    Device, FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use redshank_model::{AudioCaptureHost, NoteBody};

const MAX_CAPTURE_SECONDS: u64 = 10 * 60;
const MEDIA_TYPE: &str = "audio/wav";
const BLOB_PREFIX: &str = "voice:";

struct CaptureBuffer {
    samples: Vec<f32>,
    max_frames: usize,
}

struct ActiveCapture {
    _stream: Stream,
    samples: Arc<Mutex<CaptureBuffer>>,
    error: Arc<Mutex<Option<String>>>,
    sample_rate: u32,
}

pub(super) struct LocalVoiceCapture {
    root: PathBuf,
    active: Option<ActiveCapture>,
}

impl LocalVoiceCapture {
    pub(super) fn new(data_root: &Path) -> Self {
        Self {
            root: data_root.join("voice"),
            active: None,
        }
    }

    pub(super) fn is_available() -> bool {
        cpal::default_host()
            .default_input_device()
            .and_then(|device| device.default_input_config().ok())
            .is_some()
    }

    /// The default input device's name, for the Capture settings readout.
    pub(super) fn label() -> Option<String> {
        cpal::default_host()
            .default_input_device()
            .and_then(|device| device.description().ok())
            .map(|description| description.name().to_owned())
    }

    pub(super) fn active_error(&self) -> Option<String> {
        self.active.as_ref().and_then(|active| {
            active
                .error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        })
    }

    pub(super) fn remove_blob(data_root: &Path, blob_id: &str) -> Result<bool, String> {
        let path = Self::blob_path(data_root, blob_id)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!("Could not remove voice note audio: {error}")),
        }
    }

    pub(super) fn blob_path(data_root: &Path, blob_id: &str) -> Result<PathBuf, String> {
        blob_path(data_root, blob_id)
    }
}

impl AudioCaptureHost for LocalVoiceCapture {
    type Error = String;

    fn begin_capture(&mut self) -> Result<(), Self::Error> {
        if self.active.is_some() {
            return Err("A voice note is already recording".into());
        }
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No microphone is available")?;
        let supported = device
            .default_input_config()
            .map_err(|error| format!("Could not read the microphone configuration: {error}"))?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = usize::from(config.channels);
        let max_frames =
            usize::try_from(u64::from(sample_rate) * MAX_CAPTURE_SECONDS).unwrap_or(usize::MAX);
        let samples = Arc::new(Mutex::new(CaptureBuffer {
            samples: Vec::with_capacity(sample_rate as usize),
            max_frames,
        }));
        let error = Arc::new(Mutex::new(None));
        let stream = match sample_format {
            SampleFormat::I8 => input_stream::<i8>(&device, &config, channels, &samples, &error),
            SampleFormat::I16 => input_stream::<i16>(&device, &config, channels, &samples, &error),
            SampleFormat::I32 => input_stream::<i32>(&device, &config, channels, &samples, &error),
            SampleFormat::I64 => input_stream::<i64>(&device, &config, channels, &samples, &error),
            SampleFormat::U8 => input_stream::<u8>(&device, &config, channels, &samples, &error),
            SampleFormat::U16 => input_stream::<u16>(&device, &config, channels, &samples, &error),
            SampleFormat::U32 => input_stream::<u32>(&device, &config, channels, &samples, &error),
            SampleFormat::U64 => input_stream::<u64>(&device, &config, channels, &samples, &error),
            SampleFormat::F32 => input_stream::<f32>(&device, &config, channels, &samples, &error),
            SampleFormat::F64 => input_stream::<f64>(&device, &config, channels, &samples, &error),
            other => Err(format!("The microphone uses unsupported {other} samples")),
        }?;
        stream
            .play()
            .map_err(|error| format!("Could not start the microphone: {error}"))?;
        self.active = Some(ActiveCapture {
            _stream: stream,
            samples,
            error,
            sample_rate,
        });
        Ok(())
    }

    fn finish_capture(&mut self) -> Result<NoteBody, Self::Error> {
        let active = self.active.take().ok_or("No voice note is recording")?;
        drop(active._stream);
        if let Some(error) = active
            .error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            return Err(error);
        }
        let samples = std::mem::take(
            &mut active
                .samples
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .samples,
        );
        write_blob(&self.root, active.sample_rate, &samples)
    }

    fn cancel_capture(&mut self) -> Result<(), Self::Error> {
        self.active = None;
        Ok(())
    }
}

fn input_stream<T>(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    samples: &Arc<Mutex<CaptureBuffer>>,
    capture_error: &Arc<Mutex<Option<String>>>,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let samples = Arc::clone(samples);
    let capture_error = Arc::clone(capture_error);
    device
        .build_input_stream(
            config,
            move |input: &[T], _| {
                let mut capture = samples
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                for frame in input.chunks(channels.max(1)) {
                    if capture.samples.len() >= capture.max_frames {
                        break;
                    }
                    let mono = frame.iter().copied().map(f32::from_sample).sum::<f32>()
                        / frame.len().max(1) as f32;
                    capture.samples.push(mono);
                }
            },
            move |error| {
                *capture_error
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                    Some(format!("Microphone input stopped: {error}"));
            },
            None,
        )
        .map_err(|error| format!("Could not open the microphone: {error}"))
}

fn write_blob(root: &Path, sample_rate: u32, samples: &[f32]) -> Result<NoteBody, String> {
    if samples.is_empty() {
        return Err("The microphone captured no audio".into());
    }
    fs::create_dir_all(root)
        .map_err(|error| format!("Could not create voice-note storage: {error}"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix("recording-")
        .suffix(".wav")
        .tempfile_in(root)
        .map_err(|error| format!("Could not create a voice-note file: {error}"))?;
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(temporary.as_file_mut(), spec)
            .map_err(|error| format!("Could not start the voice-note encoder: {error}"))?;
        for sample in samples {
            let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            writer
                .write_sample(pcm)
                .map_err(|error| format!("Could not encode the voice note: {error}"))?;
        }
        writer
            .finalize()
            .map_err(|error| format!("Could not finish the voice note: {error}"))?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Could not flush the voice note: {error}"))?;
    let mut reader = BufReader::new(
        File::open(temporary.path())
            .map_err(|error| format!("Could not verify the voice note: {error}"))?,
    );
    let mut hasher = blake3::Hasher::new();
    hasher
        .update_reader(&mut reader)
        .map_err(|error| format!("Could not hash the voice note: {error}"))?;
    let digest = hasher.finalize().to_hex().to_string();
    let destination = root.join(format!("{digest}.wav"));
    if !destination.exists() {
        temporary
            .persist_noclobber(&destination)
            .map_err(|error| format!("Could not publish the voice note: {}", error.error))?;
    }
    Ok(NoteBody::Audio {
        blob_id: format!("{BLOB_PREFIX}{digest}"),
        media_type: MEDIA_TYPE.into(),
        duration_ms: samples.len() as u64 * 1_000 / u64::from(sample_rate),
    })
}

fn blob_path(data_root: &Path, blob_id: &str) -> Result<PathBuf, String> {
    let digest = blob_id
        .strip_prefix(BLOB_PREFIX)
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or("Voice note has an invalid blob identity")?;
    Ok(data_root.join("voice").join(format!("{digest}.wav")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_blob_is_content_addressed_and_path_confined() {
        let data = tempfile::tempdir().unwrap();
        let voice_root = data.path().join("voice");
        let body = write_blob(&voice_root, 48_000, &[0.0; 4_800]).unwrap();
        let NoteBody::Audio {
            blob_id,
            media_type,
            duration_ms,
        } = body
        else {
            panic!("voice capture must produce audio")
        };
        assert_eq!(media_type, MEDIA_TYPE);
        assert_eq!(duration_ms, 100);
        let path = blob_path(data.path(), &blob_id).unwrap();
        assert_eq!(path.parent(), Some(voice_root.as_path()));
        assert!(path.exists());
        assert!(blob_path(data.path(), "voice:../escape").is_err());
    }
}
