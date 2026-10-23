//! Bounded, in-memory vertical stitching. No platform, UI or image-codec types.

const BUDGET: usize = 64 * 1024 * 1024;
const MAX_FRAGMENTS: usize = 128;
mod sequence;
pub use sequence::ScrollSequence;

pub struct Fragment {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Fragment {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, StitchError> {
        let size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width < 32 || height < 64 || size != Some(rgba.len()) || rgba.len() > BUDGET {
            return Err(StitchError::InvalidFrame);
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StitchError {
    InvalidFrame,
    Budget,
    NoPending,
    InvalidOverlap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Append {
    First,
    Added { overlap: u32 },
    Duplicate,
    NeedsReview,
}

#[derive(Default)]
pub struct ScrollSession {
    frames: Vec<Fragment>,
    overlaps: Vec<u32>,
    pending: Option<Fragment>,
    pending_upward: bool,
    unmatched: Vec<(Fragment, bool)>,
    additions: Vec<bool>,
    retained: usize,
}

impl ScrollSession {
    pub fn count(&self) -> usize {
        self.frames.len()
    }
    pub fn pending_height(&self) -> Option<u32> {
        self.pending.as_ref().map(|f| f.height)
    }
    pub fn pending_upward(&self) -> bool {
        self.pending.is_some() && self.pending_upward
    }
    pub fn unresolved_count(&self) -> usize {
        self.unmatched.len() + usize::from(self.pending.is_some())
    }

    /// Continue capturing after a failed match without overwriting its pixels.
    /// Unmatched frames remain explicitly reviewable and share the same budget.
    pub fn capture(&mut self, frame: Fragment, upward: bool) -> Result<Append, StitchError> {
        if self.pending.as_ref().is_some_and(|f| f.rgba == frame.rgba)
            || self.unmatched.iter().any(|(f, _)| f.rgba == frame.rgba)
        {
            return Ok(Append::Duplicate);
        }
        self.check_budget(&frame)?;
        if let Some(pending) = self.pending.take() {
            self.unmatched.push((pending, self.pending_upward));
        }
        self.append_direction(frame, upward)
    }

    pub fn review_next(&mut self) {
        if self.unmatched.is_empty() {
            return;
        }
        if let Some(frame) = self.pending.take() {
            self.unmatched.push((frame, self.pending_upward));
        }
        let (frame, upward) = self.unmatched.remove(0);
        self.pending = Some(frame);
        self.pending_upward = upward;
    }

    fn check_budget(&self, frame: &Fragment) -> Result<(), StitchError> {
        let bytes = self.retained
            + self.pending.as_ref().map_or(0, |f| f.rgba.len())
            + self
                .unmatched
                .iter()
                .map(|(f, _)| f.rgba.len())
                .sum::<usize>();
        if self.frames.len() + self.unresolved_count() >= MAX_FRAGMENTS
            || bytes + frame.rgba.len() > BUDGET
        {
            return Err(StitchError::Budget);
        }
        if self
            .frames
            .first()
            .is_some_and(|f| f.width != frame.width || f.height != frame.height)
        {
            return Err(StitchError::InvalidFrame);
        }
        Ok(())
    }
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        let first = self.frames.first()?;
        Some((
            first.width,
            first.height
                + self
                    .frames
                    .iter()
                    .skip(1)
                    .zip(&self.overlaps)
                    .map(|(frame, overlap)| frame.height - overlap)
                    .sum::<u32>(),
        ))
    }
    pub fn seams(&self) -> Vec<u32> {
        let Some(first) = self.frames.first() else {
            return vec![];
        };
        let mut bottom = first.height;
        self.frames
            .iter()
            .skip(1)
            .zip(&self.overlaps)
            .map(|(frame, overlap)| {
                let seam = bottom;
                bottom += frame.height - overlap;
                seam
            })
            .collect()
    }

    /// Failed/uncertain matching keeps the entire incoming fragment for manual review.
    /// A second capture cannot overwrite an unresolved pending fragment.
    pub fn append(&mut self, frame: Fragment) -> Result<Append, StitchError> {
        self.append_direction(frame, false)
    }

    /// Wheel direction determines which end grows. Revisiting captured content
    /// leaves the result unchanged, including when reversing within the image.
    pub fn append_direction(
        &mut self,
        frame: Fragment,
        upward: bool,
    ) -> Result<Append, StitchError> {
        if self.pending.is_some() {
            return Err(StitchError::NoPending);
        }
        if let Some(previous) = self.frames.last()
            && (previous.width != frame.width || previous.height != frame.height)
        {
            return Err(StitchError::InvalidFrame);
        }
        self.check_budget(&frame)?;
        let Some(previous) = self.frames.last() else {
            self.retained = frame.rgba.len();
            self.frames.push(frame);
            return Ok(Append::First);
        };
        if previous.rgba == frame.rgba || self.frames.iter().any(|f| f.rgba == frame.rgba) {
            return Ok(Append::Duplicate);
        }
        let composed = self.compose(None)?;
        let stride = frame.width as usize * 4;
        if composed
            .rgba
            .chunks_exact(stride)
            .enumerate()
            .any(|(row, _)| {
                let start = row * stride;
                composed.rgba.get(start..start + frame.rgba.len()) == Some(frame.rgba.as_slice())
            })
        {
            return Ok(Append::Duplicate);
        }
        let overlap = if upward {
            find_overlap(&frame, &self.frames[0])
        } else {
            find_overlap(previous, &frame)
        };
        self.pending_upward = upward;
        self.pending = Some(frame);
        if let Some(overlap) = overlap {
            self.accept_pending(overlap)?;
            Ok(Append::Added { overlap })
        } else {
            Ok(Append::NeedsReview)
        }
    }

    pub fn accept_pending(&mut self, overlap: u32) -> Result<(), StitchError> {
        let frame = self.pending.as_ref().ok_or(StitchError::NoPending)?;
        if overlap >= frame.height {
            return Err(StitchError::InvalidOverlap);
        }
        let (_, height) = self.dimensions().ok_or(StitchError::InvalidFrame)?;
        if u64::from(height + frame.height - overlap) * u64::from(frame.width) * 4 > BUDGET as u64 {
            return Err(StitchError::Budget);
        }
        let frame = self.pending.take().ok_or(StitchError::NoPending)?;
        self.retained += frame.rgba.len();
        if self.pending_upward {
            self.frames.insert(0, frame);
            self.overlaps.insert(0, overlap);
        } else {
            self.frames.push(frame);
            self.overlaps.push(overlap);
        }
        self.additions.push(self.pending_upward);
        Ok(())
    }
    pub fn discard_pending(&mut self) {
        self.pending = None;
    }
    pub fn undo(&mut self) {
        if self.pending.take().is_some() {
            return;
        }
        if self.frames.len() > 1 {
            let upward = self.additions.pop().unwrap_or(false);
            let removed = if upward {
                Some(self.frames.remove(0))
            } else {
                self.frames.pop()
            };
            if let Some(frame) = removed {
                self.retained -= frame.rgba.len();
            }
            if upward {
                self.overlaps.remove(0);
            } else {
                self.overlaps.pop();
            }
        }
    }

    /// Includes a proposed pending join when requested; original fragments stay intact.
    pub fn compose(&self, pending_overlap: Option<u32>) -> Result<Fragment, StitchError> {
        let (width, mut height) = self.dimensions().ok_or(StitchError::InvalidFrame)?;
        if let Some(overlap) = pending_overlap {
            let frame = self.pending.as_ref().ok_or(StitchError::NoPending)?;
            if overlap >= frame.height {
                return Err(StitchError::InvalidOverlap);
            }
            height += frame.height - overlap;
        }
        let length = u64::from(width) * u64::from(height) * 4;
        if length > BUDGET as u64 {
            return Err(StitchError::Budget);
        }
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length as usize)
            .map_err(|_| StitchError::Budget)?;
        if self.pending_upward && pending_overlap.is_some() {
            let frame = self.pending.as_ref().ok_or(StitchError::NoPending)?;
            rgba.extend_from_slice(&frame.rgba);
        }
        for (index, frame) in self.frames.iter().enumerate() {
            let skip = if index == 0 {
                if self.pending_upward {
                    pending_overlap.unwrap_or(0)
                } else {
                    0
                }
            } else {
                self.overlaps[index - 1]
            };
            rgba.extend_from_slice(&frame.rgba[(skip * width) as usize * 4..]);
        }
        if !self.pending_upward
            && let Some(overlap) = pending_overlap
        {
            let frame = self.pending.as_ref().ok_or(StitchError::NoPending)?;
            rgba.extend_from_slice(&frame.rgba[(overlap * width) as usize * 4..]);
        }
        Ok(Fragment {
            width,
            height,
            rgba,
        })
    }
}

/// Only confident downward matches are accepted. Flat/repeating/animated content
/// remains pending instead of inventing a seam. Fixed headers must be excluded.
fn find_overlap(previous: &Fragment, next: &Fragment) -> Option<u32> {
    let h = previous.height;
    let minimum = (h / 4).max(32);
    let max_shift = h - minimum;
    let mut scores = Vec::with_capacity(max_shift as usize);
    for shift in 1..=max_shift {
        let overlap = h - shift;
        let mut difference = 0u64;
        let mut detail = 0u32;
        for row in 0..24u32 {
            let y = row * (overlap - 1) / 23;
            for column in 0..40u32 {
                let x = 2 + column * (previous.width - 4) / 39;
                let a = ((y + shift) * previous.width + x) as usize * 4;
                let b = (y * next.width + x) as usize * 4;
                for channel in 0..3 {
                    difference +=
                        u64::from(previous.rgba[a + channel].abs_diff(next.rgba[b + channel]));
                }
                if (0..3).any(|c| next.rgba[b + c].abs_diff(next.rgba[b - 4 + c]) > 12) {
                    detail += 1;
                }
            }
        }
        scores.push((difference as f64 / (24. * 40. * 3.), shift, detail));
    }
    let best = scores.iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
    if best.0 > 2.5 || best.2 < 12 {
        return None;
    }
    let alternative = scores
        .iter()
        .filter(|s| s.1.abs_diff(best.1) >= 4)
        .map(|s| s.0)
        .min_by(f64::total_cmp)
        .unwrap_or(f64::INFINITY);
    (alternative > best.0 + 1.5).then_some(h - best.1)
}
