//! Native input operation errors. Output is owned by `OutputRuntime`.

#[derive(Debug)]
pub enum AudioError {
    NoOutputDevice,
    NoInputDevice,
    UnsupportedSampleFormat(cpal::SampleFormat),
    // cpal 0.18 collapsed its per-operation error enums into one
    // `cpal::Error`; the variants stay split here to keep which
    // operation failed in the message.
    StreamConfig(cpal::Error),
    StreamBuild(cpal::Error),
    StreamPlay(cpal::Error),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOutputDevice => f.write_str("no audio output device available"),
            Self::NoInputDevice => f.write_str("no audio input device available"),
            Self::UnsupportedSampleFormat(fmt) => {
                write!(
                    f,
                    "audio device sample format {fmt:?} is not supported (need f32)"
                )
            },
            Self::StreamConfig(e) => write!(f, "stream config: {e}"),
            Self::StreamBuild(e) => write!(f, "stream build: {e}"),
            Self::StreamPlay(e) => write!(f, "stream play: {e}"),
        }
    }
}

impl std::error::Error for AudioError {}
