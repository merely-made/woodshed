/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Private admission adapters over the worker's one real backend.

use std::{cell::RefCell, rc::Rc, time::Duration};

use redshank_model::MediaSource as RedshankSource;
use servo_media_player::{
    PlayerError,
    controller::{
        MediaInfo, MediaSource, PlaybackCommand, PlaybackController, PlaybackSink, PlaybackSource,
    },
};

use crate::Backend;

pub(super) type BackendRef = Rc<RefCell<Backend>>;

/// Map Redshank's durable source identity onto Genet's neutral vocabulary.
pub(super) fn map_source(source: &RedshankSource) -> MediaSource {
    match source {
        RedshankSource::Local { path } => MediaSource::Local { path: path.clone() },
        RedshankSource::Enclosure { url } => MediaSource::Http { url: url.clone() },
        RedshankSource::Cached { path, .. } => MediaSource::Local { path: path.clone() },
        RedshankSource::HostBlob { id } => MediaSource::HostBlob { id: id.clone() },
    }
}

#[derive(Clone)]
pub(super) struct SourceAdapter(pub(super) BackendRef);

impl PlaybackSource for SourceAdapter {
    fn load(&mut self, source: &MediaSource) -> Result<MediaInfo, String> {
        self.0.borrow_mut().load(source)
    }

    fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.0.borrow_mut().seek(position)
    }

    fn set_rate(&mut self, _rate: f64) -> Result<(), String> {
        Err("playback-rate changes are not enabled in this bounded desktop runtime".into())
    }
}

#[derive(Clone)]
pub(super) struct SinkAdapter(pub(super) BackendRef);

impl PlaybackSink for SinkAdapter {
    fn set_playing(&mut self, playing: bool) -> Result<(), String> {
        self.0.borrow_mut().set_playing(playing)
    }

    fn reset(&mut self, position: Duration) -> Result<(), String> {
        self.0.borrow_mut().reset(position)
    }

    fn position(&self) -> Result<Duration, String> {
        self.0.borrow().position()
    }
}

pub(super) type Controller = PlaybackController<SourceAdapter, SinkAdapter>;

pub(super) fn controller(backend: BackendRef) -> Controller {
    PlaybackController::new(SourceAdapter(backend.clone()), SinkAdapter(backend))
}

pub(super) fn load(
    controller: &mut Controller,
    source: &RedshankSource,
) -> Result<(), PlayerError> {
    controller.command(PlaybackCommand::Load(map_source(source)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSource {
        fail_load: bool,
    }

    impl PlaybackSource for TestSource {
        fn load(&mut self, _: &MediaSource) -> Result<MediaInfo, String> {
            if self.fail_load {
                return Err("decoder refused source".into());
            }
            Ok(MediaInfo {
                duration: Some(Duration::from_secs(1)),
                capabilities: servo_media_player::controller::PlaybackCapabilities {
                    seekable: true,
                    playback_rates: None,
                },
            })
        }
        fn seek(&mut self, _: Duration) -> Result<(), String> {
            Ok(())
        }
        fn set_rate(&mut self, _: f64) -> Result<(), String> {
            Ok(())
        }
    }

    struct TestSink(Duration);
    impl PlaybackSink for TestSink {
        fn set_playing(&mut self, _: bool) -> Result<(), String> {
            Ok(())
        }
        fn reset(&mut self, position: Duration) -> Result<(), String> {
            self.0 = position;
            Ok(())
        }
        fn position(&self) -> Result<Duration, String> {
            Ok(self.0)
        }
    }

    #[test]
    fn maps_all_durable_source_variants() {
        assert_eq!(
            map_source(&RedshankSource::Local {
                path: "recording.mp3".into()
            }),
            MediaSource::Local {
                path: "recording.mp3".into()
            },
        );
        assert_eq!(
            map_source(&RedshankSource::Enclosure {
                url: "https://example.test/a.mp3".into()
            }),
            MediaSource::Http {
                url: "https://example.test/a.mp3".into()
            },
        );
        assert_eq!(
            map_source(&RedshankSource::Cached {
                path: "cache/audio".into(),
                origin_url: "https://example.test/a.mp3".into(),
                representation: Box::default(),
            }),
            MediaSource::Local {
                path: "cache/audio".into()
            },
        );
        assert_eq!(
            map_source(&RedshankSource::HostBlob {
                id: "blob-1".into()
            }),
            MediaSource::HostBlob {
                id: "blob-1".into()
            },
        );
    }

    #[test]
    fn controller_keeps_typed_seek_and_backend_failures() {
        let local = MediaSource::Local {
            path: "recording.mp3".into(),
        };
        let mut controller = PlaybackController::new(
            TestSource { fail_load: false },
            TestSink(Duration::from_millis(321)),
        );
        controller
            .command(PlaybackCommand::Load(local.clone()))
            .unwrap();
        assert_eq!(
            controller.command(PlaybackCommand::Seek(Duration::from_secs(2))),
            Err(PlayerError::SeekOutOfRange)
        );
        controller.sink_mut().0 = Duration::from_millis(321);
        assert_eq!(
            controller.snapshot().unwrap().position,
            Duration::from_millis(321)
        );

        let mut failed =
            PlaybackController::new(TestSource { fail_load: true }, TestSink(Duration::ZERO));
        assert_eq!(
            failed.command(PlaybackCommand::Load(local)),
            Err(PlayerError::Backend("decoder refused source".into()))
        );
    }
}
