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
