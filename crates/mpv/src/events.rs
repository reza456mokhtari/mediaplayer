//! Turns libmpv's events into [`EngineEvent`]s.
//!
//! Pure logic with no FFI, so it is unit-tested without libmpv. libmpv's playlist
//! entry id is used as the [`LoadId`], which lets the player drop events about a
//! file it no longer plays.

use std::ffi::c_int;
use std::time::Duration;

use mediaplayer_core::{EngineEvent, LoadId, VideoSize};

use crate::ffi;

/// `reply_userdata` values for the observed properties.
pub(crate) mod property {
    pub const TIME_POS: u64 = 1;
    pub const DURATION: u64 = 2;
    pub const WIDTH: u64 = 3;
    pub const HEIGHT: u64 = 4;
}

/// An event copied out of libmpv's memory.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RawEvent {
    Shutdown,
    StartFile {
        entry: i64,
    },
    FileLoaded,
    /// The video output was reconfigured: new size, format or filters, or no video.
    VideoReconfig,
    EndFile {
        entry: i64,
        reason: c_int,
        error: c_int,
    },
    Property {
        id: u64,
        value: PropertyValue,
    },
    Log {
        level: String,
        prefix: String,
        text: String,
    },
    Other,
}

/// A property value in one of the formats we observe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PropertyValue {
    /// Unavailable, e.g. `time-pos` while nothing is loaded.
    None,
    Double(f64),
    Int(i64),
    Flag(bool),
}

/// Tracks which playlist entry is current and the picture size.
#[derive(Debug, Default)]
pub(crate) struct EventMapper {
    current: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    video: Option<VideoSize>,
}

impl EventMapper {
    pub(crate) fn start_file(&mut self, entry: i64) {
        self.current = Some(entry);
        // The player forgets the picture whenever a file opens; report it afresh.
        self.video = None;
    }

    /// `duration` is read from libmpv when FILE_LOADED arrives.
    pub(crate) fn file_loaded(&self, duration: Option<f64>) -> Option<EngineEvent> {
        self.current.map(|entry| EngineEvent::Loaded {
            id: load_id(entry),
            duration: duration.and_then(seconds),
        })
    }

    pub(crate) fn end_file(
        &mut self,
        entry: i64,
        reason: c_int,
        message: impl FnOnce() -> String,
    ) -> Option<EngineEvent> {
        if self.current == Some(entry) {
            self.current = None;
        }
        let id = load_id(entry);
        match reason {
            ffi::END_FILE_REASON_EOF => Some(EngineEvent::Ended { id }),
            ffi::END_FILE_REASON_ERROR => Some(EngineEvent::Failed {
                id,
                message: message(),
            }),
            // Stopped, replaced, quit or redirected: the player asked for it.
            _ => None,
        }
    }

    pub(crate) fn property(&mut self, id: u64, value: PropertyValue) -> Option<EngineEvent> {
        match (id, value) {
            (property::TIME_POS, PropertyValue::Double(position)) => {
                seconds(position).map(EngineEvent::Position)
            }
            (property::DURATION, value) => Some(EngineEvent::DurationChanged(match value {
                PropertyValue::Double(duration) => seconds(duration),
                _ => None,
            })),
            (property::WIDTH | property::HEIGHT, value) => {
                let dimension = match value {
                    PropertyValue::Int(pixels) if pixels > 0 => Some(pixels),
                    _ => None,
                };
                if id == property::WIDTH {
                    self.width = dimension;
                } else {
                    self.height = dimension;
                }
                self.publish_video()
            }
            _ => None,
        }
    }

    /// `width` and `height` are libmpv's current `dwidth` and `dheight`, read when
    /// VIDEO_RECONFIG arrives. libmpv coalesces property changes (client.h), so a
    /// file replaced by one of the same size may never report them as changed.
    pub(crate) fn video_reconfigured(
        &mut self,
        width: Option<i64>,
        height: Option<i64>,
    ) -> Option<EngineEvent> {
        self.width = width.filter(|&pixels| pixels > 0);
        self.height = height.filter(|&pixels| pixels > 0);
        self.publish_video()
    }

    /// Reports the picture size when it differs from what was last reported.
    fn publish_video(&mut self) -> Option<EngineEvent> {
        let video = self.width.zip(self.height).and_then(|(width, height)| {
            Some(VideoSize {
                width: u32::try_from(width).ok()?,
                height: u32::try_from(height).ok()?,
            })
        });
        (video != self.video).then(|| {
            self.video = video;
            EngineEvent::VideoChanged(video)
        })
    }
}

/// libmpv playlist entry ids start at 1 and only grow.
fn load_id(entry: i64) -> LoadId {
    LoadId(entry.unsigned_abs())
}

fn seconds(value: f64) -> Option<Duration> {
    if value.is_finite() {
        Duration::try_from_secs_f64(value.max(0.0)).ok()
    } else {
        None
    }
}

/// User-facing wording for an END_FILE error code; otherwise libmpv's own text.
pub(crate) fn describe_error(code: c_int, fallback: impl FnOnce() -> String) -> String {
    match code {
        ffi::ERROR_LOADING_FAILED => "the file couldn't be opened or read".to_owned(),
        ffi::ERROR_UNKNOWN_FORMAT => "the file format isn't recognised".to_owned(),
        ffi::ERROR_NOTHING_TO_PLAY => "no audio or video in it could be decoded".to_owned(),
        ffi::ERROR_AO_INIT_FAILED => "the audio device couldn't be opened".to_owned(),
        ffi::ERROR_VO_INIT_FAILED => "video output couldn't start".to_owned(),
        _ => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EngineEvent::{DurationChanged, Ended, Failed, Loaded, Position, VideoChanged};

    const STOP: c_int = 2;
    const QUIT: c_int = 3;
    const REDIRECT: c_int = 5;

    #[test]
    fn file_loaded_is_reported_for_the_entry_that_started() {
        let mut mapper = EventMapper::default();
        mapper.start_file(7);
        assert_eq!(
            mapper.file_loaded(Some(12.5)),
            Some(Loaded {
                id: LoadId(7),
                duration: Some(Duration::from_millis(12_500))
            })
        );
    }

    #[test]
    fn file_loaded_without_a_started_entry_is_dropped() {
        assert_eq!(EventMapper::default().file_loaded(Some(1.0)), None);
    }

    #[test]
    fn end_of_file_and_errors_carry_their_own_entry_id() {
        let mut mapper = EventMapper::default();
        mapper.start_file(3);
        assert_eq!(
            mapper.end_file(3, ffi::END_FILE_REASON_EOF, String::new),
            Some(Ended { id: LoadId(3) })
        );
        assert_eq!(
            mapper.end_file(4, ffi::END_FILE_REASON_ERROR, || "broken".to_owned()),
            Some(Failed {
                id: LoadId(4),
                message: "broken".to_owned()
            })
        );
    }

    #[test]
    fn stopped_replaced_and_quit_files_report_nothing() {
        let mut mapper = EventMapper::default();
        for reason in [STOP, QUIT, REDIRECT] {
            assert_eq!(mapper.end_file(1, reason, String::new), None);
        }
    }

    #[test]
    fn the_end_of_an_older_entry_keeps_the_current_one() {
        let mut mapper = EventMapper::default();
        mapper.start_file(2);
        mapper.end_file(1, ffi::END_FILE_REASON_EOF, String::new);
        assert_eq!(
            mapper.file_loaded(None),
            Some(Loaded {
                id: LoadId(2),
                duration: None
            })
        );
    }

    #[test]
    fn the_current_entry_is_forgotten_once_it_ends() {
        let mut mapper = EventMapper::default();
        mapper.start_file(2);
        mapper.end_file(2, ffi::END_FILE_REASON_ERROR, String::new);
        assert_eq!(mapper.file_loaded(None), None);
    }

    #[test]
    fn time_pos_becomes_the_position() {
        let mut mapper = EventMapper::default();
        let time_pos = |mapper: &mut EventMapper, value| mapper.property(property::TIME_POS, value);
        assert_eq!(
            time_pos(&mut mapper, PropertyValue::Double(1.25)),
            Some(Position(Duration::from_millis(1250)))
        );
        assert_eq!(
            time_pos(&mut mapper, PropertyValue::Double(-0.5)),
            Some(Position(Duration::ZERO))
        );
        assert_eq!(time_pos(&mut mapper, PropertyValue::Double(f64::NAN)), None);
        assert_eq!(time_pos(&mut mapper, PropertyValue::None), None);
    }

    #[test]
    fn duration_changes_are_forwarded() {
        let mut mapper = EventMapper::default();
        assert_eq!(
            mapper.property(property::DURATION, PropertyValue::Double(90.0)),
            Some(DurationChanged(Some(Duration::from_secs(90))))
        );
        assert_eq!(
            mapper.property(property::DURATION, PropertyValue::None),
            Some(DurationChanged(None))
        );
    }

    #[test]
    fn video_size_is_reported_once_both_dimensions_are_known() {
        let mut mapper = EventMapper::default();
        assert_eq!(
            mapper.property(property::WIDTH, PropertyValue::Int(1280)),
            None
        );
        assert_eq!(
            mapper.property(property::HEIGHT, PropertyValue::Int(720)),
            Some(VideoChanged(Some(VideoSize {
                width: 1280,
                height: 720
            })))
        );
        // Unchanged values aren't repeated.
        assert_eq!(
            mapper.property(property::HEIGHT, PropertyValue::Int(720)),
            None
        );
        assert_eq!(
            mapper.property(property::WIDTH, PropertyValue::None),
            Some(VideoChanged(None))
        );
        assert_eq!(mapper.property(property::HEIGHT, PropertyValue::None), None);
    }

    const HD: VideoSize = VideoSize {
        width: 1280,
        height: 720,
    };

    fn playing_hd(mapper: &mut EventMapper) {
        mapper.start_file(1);
        mapper.property(property::WIDTH, PropertyValue::Int(1280));
        mapper.property(property::HEIGHT, PropertyValue::Int(720));
    }

    #[test]
    fn each_file_reports_its_video_size_even_when_unchanged() {
        let mut mapper = EventMapper::default();
        playing_hd(&mut mapper);
        mapper.end_file(1, STOP, String::new);
        mapper.start_file(2);
        assert_eq!(
            mapper.property(property::HEIGHT, PropertyValue::Int(720)),
            Some(VideoChanged(Some(HD)))
        );
    }

    #[test]
    fn video_reconfig_reports_the_current_size() {
        let mut mapper = EventMapper::default();
        playing_hd(&mut mapper);
        // A same-sized replacement: libmpv sends no dwidth/dheight change at all.
        mapper.start_file(2);
        assert_eq!(
            mapper.video_reconfigured(Some(1280), Some(720)),
            Some(VideoChanged(Some(HD)))
        );
        assert_eq!(mapper.video_reconfigured(Some(1280), Some(720)), None);
        assert_eq!(
            mapper.video_reconfigured(None, Some(720)),
            Some(VideoChanged(None))
        );
    }

    #[test]
    fn unknown_properties_are_ignored() {
        assert_eq!(
            EventMapper::default().property(99, PropertyValue::Flag(true)),
            None
        );
    }

    #[test]
    fn common_errors_get_plain_wording() {
        assert_eq!(
            describe_error(ffi::ERROR_UNKNOWN_FORMAT, String::new),
            "the file format isn't recognised"
        );
        assert_eq!(
            describe_error(-20, || "generic error".to_owned()),
            "generic error"
        );
    }
}
