//! Recording time is shared by video and audio; pauses never create media gaps.
use std::time::{Duration, Instant};

pub struct Timeline {
    start: Instant,
    paused_at: Option<Instant>,
    paused: Duration,
}
impl Default for Timeline {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            paused_at: None,
            paused: Duration::ZERO,
        }
    }
}
impl Timeline {
    pub fn elapsed(&self) -> Duration {
        self.paused_at
            .unwrap_or_else(Instant::now)
            .saturating_duration_since(self.start)
            .saturating_sub(self.paused)
    }
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }
    pub fn pause(&mut self, pause: bool) {
        match (pause, self.paused_at) {
            (true, None) => self.paused_at = Some(Instant::now()),
            (false, Some(at)) => {
                self.paused += at.elapsed();
                self.paused_at = None;
            }
            _ => {}
        }
    }
}
