//! Ordered source frames: acquisition never depends on a successful join.
//! Each join belongs to its two neighbours and is reviewed only after capture.
use super::{Append, BUDGET, Fragment, MAX_FRAGMENTS, StitchError};

#[derive(Default)]
pub struct ScrollSequence {
    frames: Vec<Fragment>,
    joins: Vec<Option<u32>>,
    cuts: Vec<u32>,
    operations: Vec<bool>,
    retained: usize,
    review: usize,
}
impl ScrollSequence {
    pub fn count(&self) -> usize {
        self.frames.len()
    }
    pub fn unresolved_count(&self) -> usize {
        self.joins.iter().filter(|j| j.is_none()).count()
    }
    fn pending_index(&self) -> Option<usize> {
        (self.review..self.joins.len())
            .chain(0..self.review.min(self.joins.len()))
            .find(|i| self.joins[*i].is_none())
    }
    pub fn pending_height(&self) -> Option<u32> {
        self.pending_index().map(|i| self.frames[i + 1].height)
    }
    pub fn pending_upward(&self) -> bool {
        false
    }
    pub fn review_next(&mut self) {
        if let Some(index) = self.pending_index() {
            self.review = (index + 1) % self.joins.len();
        }
    }
    pub fn review_position(&self) -> Option<(usize, usize)> {
        self.pending_index().map(|i| (i + 1, i + 2))
    }
    pub fn capture(&mut self, frame: Fragment, upward: bool) -> Result<Append, StitchError> {
        if self
            .frames
            .first()
            .is_some_and(|f| f.width != frame.width || f.height != frame.height)
        {
            return Err(StitchError::InvalidFrame);
        }
        // A transient frame must not poison the chain forever. Reacquiring a
        // verified overlap with the last connected source replaces that tail.
        if !upward
            && let Some(anchor) = self.joins.iter().position(Option::is_none)
            && align(&self.frames[anchor], &frame).is_some()
        {
            for removed in self.frames.drain(anchor + 1..) {
                self.retained -= removed.rgba.len();
            }
            self.joins.truncate(anchor);
            self.cuts.truncate(anchor);
            self.operations = vec![false; anchor];
            self.review = 0;
        }
        if self.frames.iter().any(|f| f.rgba == frame.rgba)
            || (if upward {
                self.frames.first()
            } else {
                self.frames.last()
            })
            .is_some_and(|previous| stationary(previous, &frame))
        {
            return Ok(Append::Duplicate);
        }
        self.compact_for(frame.rgba.len());
        if self.frames.len() >= MAX_FRAGMENTS || self.retained + frame.rgba.len() > BUDGET {
            return Err(StitchError::Budget);
        }
        let previous = if upward {
            self.frames.first()
        } else {
            self.frames.last()
        };
        let join = previous.and_then(|previous| {
            if upward {
                align(&frame, previous)
            } else {
                align(previous, &frame)
            }
        });
        let first = self.frames.is_empty();
        self.retained += frame.rgba.len();
        if upward {
            self.frames.insert(0, frame);
            if !first {
                self.joins.insert(0, join);
                self.cuts.insert(0, join.unwrap_or(0) / 2);
            }
        } else {
            self.frames.push(frame);
            if !first {
                self.joins.push(join);
                self.cuts.push(join.unwrap_or(0) / 2);
            }
        }
        if !first {
            self.operations.push(upward);
        }
        Ok(if first {
            Append::First
        } else if let Some(overlap) = join {
            Append::Added { overlap }
        } else {
            Append::NeedsReview
        })
    }
    pub fn append(&mut self, frame: Fragment) -> Result<Append, StitchError> {
        self.capture(frame, false)
    }
    /// Remove only a redundant intermediate viewport with a separately verified
    /// direct overlap. Do not raise the memory ceiling to retain duplicate rows.
    fn compact_for(&mut self, incoming: usize) {
        if self.operations.iter().any(|upward| *upward) {
            return;
        }
        while self.retained + incoming > BUDGET || self.frames.len() >= MAX_FRAGMENTS {
            let Some(index) = (1..self.frames.len().saturating_sub(1)).find(|i| {
                let height = self.frames[*i].height;
                let (Some(a), Some(b)) = (self.joins[*i - 1], self.joins[*i]) else {
                    return false;
                };
                let shift = (height - a) + (height - b);
                shift < height.saturating_sub(32)
                    && align(&self.frames[*i - 1], &self.frames[*i + 1]) == Some(height - shift)
            }) else {
                break;
            };
            let overlap = self.joins[index - 1].unwrap() + self.joins[index].unwrap()
                - self.frames[index].height;
            self.retained -= self.frames.remove(index).rgba.len();
            self.joins.remove(index);
            self.cuts.remove(index);
            self.joins[index - 1] = Some(overlap);
            self.cuts[index - 1] = overlap / 2;
            self.operations = vec![false; self.frames.len() - 1];
            self.review = 0;
        }
    }
    pub fn accept_pending(&mut self, overlap: u32) -> Result<(), StitchError> {
        let index = self.pending_index().ok_or(StitchError::NoPending)?;
        if overlap >= self.frames[index + 1].height {
            return Err(StitchError::InvalidOverlap);
        }
        self.joins[index] = Some(overlap);
        self.cuts[index] = overlap / 2;
        Ok(())
    }
    /// Explicitly discard the later frame and recompute the new adjacent join.
    pub fn discard_pending(&mut self) {
        let Some(index) = self.pending_index() else {
            return;
        };
        let removed = self.frames.remove(index + 1);
        self.retained -= removed.rgba.len();
        self.joins.remove(index);
        self.cuts.remove(index);
        if index < self.joins.len() {
            self.joins[index] = align(&self.frames[index], &self.frames[index + 1]);
            self.cuts[index] = self.joins[index].unwrap_or(0) / 2;
        }
        // After a user edits the sequence, remaining undo operations remove the end.
        self.operations = vec![false; self.frames.len().saturating_sub(1)];
        self.review = index.min(self.joins.len());
    }
    pub fn undo(&mut self) {
        let Some(upward) = self.operations.pop() else {
            return;
        };
        let frame = if upward {
            self.joins.remove(0);
            self.cuts.remove(0);
            self.frames.remove(0)
        } else {
            self.joins.pop();
            self.cuts.pop();
            self.frames.pop().expect("operation has a source frame")
        };
        self.retained -= frame.rgba.len();
        self.review = self.review.min(self.joins.len());
    }
    pub fn retry_alignment(&mut self) {
        for (i, join) in self.joins.iter_mut().enumerate() {
            if join.is_none() {
                *join = align(&self.frames[i], &self.frames[i + 1]);
                self.cuts[i] = join.unwrap_or(0) / 2;
            }
        }
        // End-of-capture recovery can bridge a transient/animated source using
        // later pixels, rather than repeating the same failed adjacent match.
        let mut index = 0;
        while index < self.joins.len() {
            if self.joins[index].is_none()
                && let Some((next, overlap)) = (index + 2..self.frames.len()).find_map(|next| {
                    align(&self.frames[index], &self.frames[next]).map(|overlap| (next, overlap))
                })
            {
                for removed in self.frames.drain(index + 1..next) {
                    self.retained -= removed.rgba.len();
                }
                self.joins.drain(index..next);
                self.cuts.drain(index..next);
                self.joins.insert(index, Some(overlap));
                self.cuts.insert(index, overlap / 2);
                self.operations = vec![false; self.frames.len() - 1];
            }
            index += 1;
        }
        self.review = 0;
    }
    /// Only the continuous, verified range belongs to the normal preview/export.
    /// Unmatched sources remain retained, never masquerading as full-frame joins.
    fn layout(&self, proposed: Option<u32>) -> Result<Vec<(usize, u32, u32)>, StitchError> {
        let selected = self.pending_index();
        if proposed.is_some() && selected.is_none() {
            return Err(StitchError::NoPending);
        }
        if let (Some(overlap), Some(index)) = (proposed, selected)
            && overlap >= self.frames[index + 1].height
        {
            return Err(StitchError::InvalidOverlap);
        }
        let mut layout = Vec::new();
        for (index, frame) in self.frames.iter().enumerate() {
            let join = |i: usize| -> Option<(u32, u32)> {
                if selected == Some(i)
                    && let Some(overlap) = proposed
                {
                    Some((overlap, overlap / 2))
                } else {
                    self.joins[i]
                        .map(|overlap| (overlap, self.cuts[i]))
                        .or_else(|| proposed.map(|_| (0, 0)))
                }
            };
            let start = if index == 0 {
                0
            } else {
                let Some((_, cut)) = join(index - 1) else {
                    break;
                };
                cut
            };
            let end = if index < self.joins.len() {
                join(index).map_or(frame.height, |(overlap, cut)| frame.height - overlap + cut)
            } else {
                frame.height
            };
            if start >= end || end > frame.height {
                return Err(StitchError::InvalidOverlap);
            }
            layout.push((index, start, end));
        }
        Ok(layout)
    }
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        let first = self.frames.first()?;
        Some((
            first.width,
            self.layout(None)
                .ok()?
                .iter()
                .map(|(_, start, end)| end - start)
                .sum(),
        ))
    }
    pub fn seams(&self) -> Vec<u32> {
        let Ok(layout) = self.layout(self.pending_index().is_some().then_some(0)) else {
            return Vec::new();
        };
        let mut y = 0;
        layout
            .iter()
            .take(layout.len().saturating_sub(1))
            .map(|(_, start, end)| {
                y += end - start;
                y
            })
            .collect()
    }
    pub fn pending_seam(&self) -> Option<u32> {
        self.pending_index()
            .and_then(|i| self.seams().get(i).copied())
    }
    /// Build a bounded live preview directly from ordered source strips.
    pub fn thumbnail(&self, proposed: Option<u32>) -> Result<(Fragment, u32), StitchError> {
        let first = self.frames.first().ok_or(StitchError::InvalidFrame)?;
        let mut source_height = 0;
        let mut layout = Vec::new();
        for (index, start, end) in self.layout(proposed)? {
            source_height += end - start;
            layout.push((index, start, source_height));
        }
        let ratio = (480. / f64::from(first.width))
            .min((2_000_000. / (f64::from(first.width) * f64::from(source_height))).sqrt())
            .min(1.);
        let width = (f64::from(first.width) * ratio).round().max(1.) as u32;
        let height = (f64::from(source_height) * ratio).round().max(1.) as u32;
        let mut rgba = vec![0; width as usize * height as usize * 4];
        let mut strip = 0;
        for y in 0..height {
            let source_y = (u64::from(y) * u64::from(source_height) / u64::from(height)) as u32;
            while source_y >= layout[strip].2 {
                strip += 1;
            }
            let (index, skip, _) = layout[strip];
            let top = if strip == 0 { 0 } else { layout[strip - 1].2 };
            let frame = &self.frames[index];
            for x in 0..width {
                let source_x = (u64::from(x) * u64::from(first.width) / u64::from(width)) as usize;
                let from = ((source_y - top + skip) as usize * frame.width as usize + source_x) * 4;
                let to = (y as usize * width as usize + x as usize) * 4;
                rgba[to..to + 4].copy_from_slice(&frame.rgba[from..from + 4]);
            }
        }
        Ok((
            Fragment {
                width,
                height,
                rgba,
            },
            source_height,
        ))
    }
    pub fn compose(&self, proposed: Option<u32>) -> Result<Fragment, StitchError> {
        let width = self.frames.first().ok_or(StitchError::InvalidFrame)?.width;
        let layout = self.layout(proposed)?;
        let length = layout
            .iter()
            .map(|(_, start, end)| (end - start) as usize * width as usize * 4)
            .sum();
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length)
            .map_err(|_| StitchError::Budget)?;
        for (index, start, end) in layout {
            rgba.extend_from_slice(
                &self.frames[index].rgba
                    [start as usize * width as usize * 4..end as usize * width as usize * 4],
            );
        }
        Ok(Fragment {
            width,
            height: (rgba.len() / (width as usize * 4)) as u32,
            rgba,
        })
    }
}

/// Zero movement is a content decision, not full-buffer byte equality. A small
/// cursor change must not generate a new viewport and an artificial seam.
fn stationary(previous: &Fragment, next: &Fragment) -> bool {
    let mut changed = 0;
    let mut total = 0;
    for row in 0..96 {
        let y = previous.height / 12 + row * (previous.height * 5 / 6 - 1) / 95;
        for column in 0..96 {
            let x = previous.width / 12 + column * (previous.width * 5 / 6 - 1) / 95;
            let p = ((y * previous.width + x) * 4) as usize;
            changed += usize::from((0..3).any(|c| previous.rgba[p + c] != next.rgba[p + c]));
            total += 1;
        }
    }
    changed * 500 <= total
}

/// Dense, integer-pixel baseline. Score informative edges rather than letting
/// white space dominate. Independent vertical regions tolerate one animated
/// patch; competing offsets must remain distinguishable. No scaling or blending.
fn alignment_score(previous: &Fragment, next: &Fragment, shift: u32, dense: bool) -> Option<f64> {
    let overlap = previous.height - shift;
    let trim = (previous.height / 8).min(overlap / 5);
    let margin = (previous.width / 12).max(2);
    let rows = if dense {
        (overlap - trim * 2).min(512)
    } else {
        64
    };
    let columns = if dense {
        (previous.width - margin * 2).min(256)
    } else {
        64
    };
    let mut sums = [0u64; 4];
    let mut counts = [0u32; 4];
    for row in 0..rows {
        let y = trim + row * (overlap - trim * 2 - 1) / (rows - 1);
        for column in 0..columns {
            let x = margin + column * (previous.width - margin * 2 - 1) / (columns - 1);
            let a = (((y + shift) * previous.width + x) * 4) as usize;
            let b = ((y * next.width + x) * 4) as usize;
            if !(0..3).any(|c| {
                previous.rgba[a + c].abs_diff(previous.rgba[a - 4 + c]) > 10
                    || next.rgba[b + c].abs_diff(next.rgba[b - 4 + c]) > 10
            }) {
                continue;
            }
            let band = (row * 4 / rows) as usize;
            sums[band] += (0..3)
                .map(|c| u64::from(previous.rgba[a + c].abs_diff(next.rgba[b + c])))
                .sum::<u64>();
            counts[band] += 1;
        }
    }
    if counts.iter().sum::<u32>() < if dense { 24 } else { 8 } {
        return None;
    }
    let mut regions: Vec<f64> = (0..4)
        .filter(|i| counts[*i] >= 4)
        .map(|i| sums[i] as f64 / (3 * counts[i]) as f64)
        .collect();
    regions.sort_by(f64::total_cmp);
    if regions.len() >= 3 {
        regions.pop();
    }
    if regions.is_empty() {
        return None;
    }
    Some(regions.iter().sum::<f64>() / regions.len() as f64)
}

fn align(previous: &Fragment, next: &Fragment) -> Option<u32> {
    let mut candidates: Vec<_> = (1..previous.height.saturating_sub(31))
        .filter_map(|shift| {
            alignment_score(previous, next, shift, false).map(|score| (score, shift))
        })
        .filter(|(score, _)| *score <= 12.)
        .collect();
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let scores: Vec<_> = candidates
        .into_iter()
        .take(64)
        .filter_map(|(_, shift)| {
            alignment_score(previous, next, shift, true).map(|score| (score, shift))
        })
        .collect();
    let best = scores.iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
    let alternative = scores
        .iter()
        .filter(|s| s.1.abs_diff(best.1) >= 4)
        .map(|s| s.0)
        .min_by(f64::total_cmp)
        .unwrap_or(f64::INFINITY);
    (best.0 <= 7.5 && alternative > best.0 * 1.25 + 0.15).then_some(previous.height - best.1)
}
