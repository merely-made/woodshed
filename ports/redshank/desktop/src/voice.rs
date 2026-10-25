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
    stopped: bool,
    limit_reached: bool,
}

impl CaptureBuffer {
    fn accept<T: Sample>(&mut self, input: &[T], channels: usize)
    where
        f32: FromSample<T>,
    {
        if self.stopped {
            return;
        }
        for frame in input.chunks(channels.max(1)) {
            if self.samples.len() == self.max_frames {
                self.limit_reached = true;
                break;
            }
            let mono = frame.iter().copied().map(f32::from_sample).sum::<f32>()
                / frame.len().max(1) as f32;
            self.samples.push(mono);
        }
        self.limit_reached |= self.samples.len() == self.max_frames;
    }
}

struct ActiveCapture {
    stream: Option<Stream>,
    samples: Arc<Mutex<CaptureBuffer>>,
    error: Arc<Mutex<Option<String>>>,
    sample_rate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CaptureProgress {
    pub(super) frames: usize,
    pub(super) sample_rate: u32,
    pub(super) stopped: bool,
    pub(super) limit_reached: bool,
}

impl CaptureProgress {
    pub(super) fn duration_ms(self) -> u64 {
        self.frames as u64 * 1_000 / u64::from(self.sample_rate)
    }
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

    pub(super) fn progress(&self) -> Option<CaptureProgress> {
        let active = self.active.as_ref()?;
        let buffer = active
            .samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Some(CaptureProgress {
            frames: buffer.samples.len(),
            sample_rate: active.sample_rate,
            stopped: buffer.stopped,
            limit_reached: buffer.limit_reached,
        })
    }

    pub(super) fn stop(&mut self) -> Result<(), String> {
        let active = self.active.as_mut().ok_or("No voice note is recording")?;
        active
            .samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .stopped = true;
        active.stream = None;
        Ok(())
    }

    /// PCM custody ends only after the host observes durable model publication.
    pub(super) fn acknowledge_saved(&mut self) {
        self.active = None;
    }

    #[cfg(test)]
    pub(super) fn begin_test_capture(&mut self, sample_rate: u32, max_frames: usize) {
        assert!(self.active.is_none() && sample_rate > 0 && max_frames > 0);
        self.active = Some(ActiveCapture {
            stream: None,
            samples: Arc::new(Mutex::new(CaptureBuffer {
                samples: Vec::new(),
                max_frames,
                stopped: false,
                limit_reached: false,
            })),
            error: Arc::new(Mutex::new(None)),
            sample_rate,
        });
    }

    #[cfg(test)]
    pub(super) fn test_input(&mut self, input: &[f32], channels: usize) {
        self.active
            .as_ref()
            .unwrap()
            .samples
            .lock()
            .unwrap()
            .accept(input, channels);
    }

    #[cfg(test)]
    pub(super) fn test_error(&mut self, error: &str) {
        *self.active.as_ref().unwrap().error.lock().unwrap() = Some(error.into());
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
            stopped: false,
            limit_reached: false,
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
            stream: Some(stream),
            samples,
            error,
            sample_rate,
        });
        Ok(())
    }

    fn finish_capture(&mut self) -> Result<NoteBody, Self::Error> {
        self.stop()?;
        let active = self.active.as_ref().expect("stopped capture remains owned");
        let buffer = active
            .samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // An input failure stops acquisition; an explicit Finish may still
        // preserve its accepted prefix. Blob failure leaves every sample here.
        write_blob(&self.root, active.sample_rate, &buffer.samples)
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
                capture.accept(input, channels);
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

pub(super) fn write_blob(
    root: &Path,
    sample_rate: u32,
    samples: &[f32],
) -> Result<NoteBody, String> {
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
    let encoded_length = temporary
        .as_file()
        .metadata()
        .map_err(|error| error.to_string())?
        .len();
    match temporary.persist_noclobber(&destination) {
        Ok(_) => {},
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {},
        Err(error) => return Err(format!("Could not publish the voice note: {}", error.error)),
    }
    let mut published = BufReader::new(
        File::open(&destination)
            .map_err(|error| format!("Could not reopen the voice note: {error}"))?,
    );
    if published
        .get_ref()
        .metadata()
        .map_err(|error| error.to_string())?
        .len()
        != encoded_length
    {
        return Err(
            "The stored voice-note bytes differ from this capture; captured audio is retained"
                .into(),
        );
    }
    let mut readback = blake3::Hasher::new();
    readback
        .update_reader(&mut published)
        .map_err(|error| format!("Could not verify the published voice note: {error}"))?;
    if readback.finalize().to_hex().as_str() != digest {
        return Err(
            "The stored voice-note bytes differ from this capture; captured audio is retained"
                .into(),
        );
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
    fn accepted_prefix_and_limit_are_invariant_across_callback_chunks() {
        let data = tempfile::tempdir().unwrap();
        let input: Vec<_> = (0..12).flat_map(|_| [0.25, 0.75]).collect();
        let mut bodies = Vec::new();
        for chunk_frames in [1, 2, 3, 12] {
            let mut capture = LocalVoiceCapture::new(data.path());
            capture.begin_test_capture(10, 5);
            for chunk in input.chunks(chunk_frames * 2) {
                capture.test_input(chunk, 2);
            }
            let progress = capture.progress().unwrap();
            assert_eq!(progress.frames, 5);
            assert_eq!(progress.duration_ms(), 500);
            assert!(progress.limit_reached && !progress.stopped);
            let body = capture.finish_capture().unwrap();
            assert!(capture.progress().unwrap().stopped);
            assert_eq!(
                capture.progress().unwrap().frames,
                5,
                "encoding does not release PCM custody"
            );
            capture.test_input(&[-1.0; 8], 2);
            assert_eq!(
                capture.finish_capture().unwrap(),
                body,
                "late input and retry cannot change the retained prefix"
            );
            let NoteBody::Audio {
                blob_id,
                duration_ms,
                ..
            } = &body
            else {
                panic!("audio body")
            };
            assert_eq!(*duration_ms, 500);
            let mut wav =
                hound::WavReader::open(LocalVoiceCapture::blob_path(data.path(), blob_id).unwrap())
                    .unwrap();
            assert_eq!(wav.duration(), 5);
            assert_eq!(
                wav.samples::<i16>().map(Result::unwrap).collect::<Vec<_>>(),
                vec![16384; 5]
            );
            bodies.push(body);
            capture.acknowledge_saved();
            assert!(capture.progress().is_none());
        }
        assert!(bodies.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn blob_write_failure_preserves_stopped_samples_for_exact_retry_or_explicit_cancel() {
        let data = tempfile::tempdir().unwrap();
        let voice_root = data.path().join("voice");
        fs::write(&voice_root, b"blocked directory").unwrap();
        let mut capture = LocalVoiceCapture::new(data.path());
        capture.begin_test_capture(10, 20);
        capture.test_input(&[0.25; 10], 1);
        assert!(capture.finish_capture().is_err());
        let retained = capture.progress().unwrap();
        assert!(retained.stopped && !retained.limit_reached);
        assert_eq!(retained.frames, 10);
        fs::remove_file(&voice_root).unwrap();
        let body = capture.finish_capture().unwrap();
        assert_eq!(capture.progress(), Some(retained));
        assert_eq!(capture.finish_capture().unwrap(), body);
        capture.cancel_capture().unwrap();
        assert!(capture.progress().is_none());
    }

    #[test]
    fn conflicting_existing_blob_is_not_acknowledged_or_allowed_to_release_pcm() {
        let data = tempfile::tempdir().unwrap();
        let mut capture = LocalVoiceCapture::new(data.path());
        capture.begin_test_capture(10, 20);
        capture.test_input(&[0.25; 10], 1);
        let NoteBody::Audio { blob_id, .. } = capture.finish_capture().unwrap() else {
            panic!("audio body")
        };
        let path = LocalVoiceCapture::blob_path(data.path(), &blob_id).unwrap();
        fs::write(&path, b"damaged retained blob").unwrap();
        assert!(capture.finish_capture().unwrap_err().contains("differ"));
        assert_eq!(capture.progress().unwrap().frames, 10);
        assert_eq!(fs::read(&path).unwrap(), b"damaged retained blob");
        fs::remove_file(&path).unwrap();
        assert!(capture.finish_capture().is_ok());
        assert_eq!(capture.progress().unwrap().frames, 10);
    }

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
