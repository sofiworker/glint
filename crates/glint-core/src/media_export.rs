//! Export choices are separate from capture defaults and document history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
    Vp9,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoContainer {
    #[default]
    Mp4,
    Mov,
    Mkv,
    WebM,
}
impl VideoContainer {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Mkv => "mkv",
            Self::WebM => "webm",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoQuality {
    High,
    #[default]
    Balanced,
    Small,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoExport {
    pub container: VideoContainer,
    pub codec: VideoCodec,
    /// None follows the source. Export never raises the source frame rate.
    pub fps: Option<u32>,
    pub quality: VideoQuality,
    /// None removes audio; otherwise AAC (MP4/MOV/MKV) or Opus (WebM) bits per second.
    pub audio_bitrate: Option<u32>,
}
impl Default for VideoExport {
    fn default() -> Self {
        Self {
            container: VideoContainer::Mp4,
            codec: VideoCodec::H264,
            fps: None,
            quality: VideoQuality::Balanced,
            audio_bitrate: Some(192_000),
        }
    }
}
impl VideoExport {
    pub fn validate(self) -> Result<(), &'static str> {
        if (self.container == VideoContainer::WebM) != (self.codec == VideoCodec::Vp9) {
            return Err("WebM requires VP9; MP4/MOV/MKV require H.264 or H.265");
        }
        if self.fps.is_some_and(|f| !(1..=144).contains(&f))
            || self
                .audio_bitrate
                .is_some_and(|v| ![128_000, 192_000, 256_000].contains(&v))
        {
            return Err("Invalid export frame rate or audio bitrate");
        }
        Ok(())
    }
    pub fn frame_rate(self, source: u32, gif: bool) -> u32 {
        self.fps
            .unwrap_or(source)
            .min(source)
            .clamp(1, if gif { 30 } else { 144 })
    }
    pub fn video_bitrate(self, width: u32, height: u32, fps: u32) -> u32 {
        let divisor = match self.quality {
            VideoQuality::High => 4,
            VideoQuality::Balanced => 8,
            VideoQuality::Small => 16,
        };
        let bitrate = u64::from(width) * u64::from(height) * u64::from(fps) / divisor;
        (if matches!(self.codec, VideoCodec::Hevc | VideoCodec::Vp9) {
            bitrate * 2 / 3
        } else {
            bitrate
        })
        .clamp(250_000, 80_000_000) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_rate_and_quality_obey_source_and_format_limits() {
        let v = VideoExport {
            fps: Some(60),
            ..Default::default()
        };
        assert_eq!(v.frame_rate(24, false), 24);
        assert_eq!(v.frame_rate(120, true), 30);
        let high = VideoExport {
            quality: VideoQuality::High,
            ..v
        };
        assert!(high.video_bitrate(1920, 1080, 30) > v.video_bitrate(1920, 1080, 30));
        assert!(
            VideoExport {
                audio_bitrate: Some(1),
                ..v
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn container_codec_pairs_are_validated_before_output() {
        let webm = VideoExport {
            container: VideoContainer::WebM,
            codec: VideoCodec::Vp9,
            ..Default::default()
        };
        assert!(webm.validate().is_ok());
        assert!(
            VideoExport {
                codec: VideoCodec::H264,
                ..webm
            }
            .validate()
            .is_err()
        );
        assert!(
            VideoExport {
                container: VideoContainer::Mov,
                ..webm
            }
            .validate()
            .is_err()
        );
        assert!(
            VideoExport {
                container: VideoContainer::Mkv,
                codec: VideoCodec::Hevc,
                ..webm
            }
            .validate()
            .is_ok()
        );
    }
}
