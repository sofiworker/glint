//! Ordered source frames: acquisition never depends on a successful join.
//! Each join belongs to its two neighbours and is reviewed only after capture.
//! A cursor tracks the frame matching the latest viewport, so scrolling up,
//! down, or back and forth always places a new frame relative to what the user
//! is currently looking at instead of blindly extending one end.
use super::{Append, BUDGET, Fragment, MAX_FRAGMENTS, StitchError};

/// One accepted capture: where its frame lives and what a later undo must restore.
enum Operation {
    /// Appended at one end of the strip.
    End { upward: bool },
    /// Inserted between `index - 1` and `index`; `restored` is the join that
    /// previously connected those neighbours and must come back on undo.
    Insert { index: usize, restored: Option<u32> },
}

#[derive(Default)]
pub struct ScrollSequence {
    frames: Vec<Fragment>,
    joins: Vec<Option<u32>>,
    cuts: Vec<u32>,
    operations: Vec<Operation>,
    retained: usize,
    review: usize,
    cursor: usize,
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

    /// Strip position of every frame's top row, when known. Positions derive
    /// from verified joins only; an unresolved join makes everything past it
    /// unknown until the join is accepted or bridged.
    fn positions(&self) -> Vec<Option<i64>> {
        let Some(first) = self.frames.first() else {
            return Vec::new();
        };
        let height = first.height as i64;
        let mut positions = Vec::with_capacity(self.frames.len());
        positions.push(Some(0i64));
        for (index, join) in self.joins.iter().enumerate() {
            let next = match (positions[index], join) {
                (Some(previous), Some(overlap)) => Some(previous + height - i64::from(*overlap)),
                _ => None,
            };
            positions.push(next);
        }
        positions
    }

    pub fn capture(&mut self, frame: Fragment, upward: bool) -> Result<Append, StitchError> {
        if self
            .frames
            .first()
            .is_some_and(|f| f.width != frame.width || f.height != frame.height)
        {
            return Err(StitchError::InvalidFrame);
        }
        if self.frames.is_empty() {
            self.retained = frame.rgba.len();
            self.frames.push(frame);
            self.cursor = 0;
            return Ok(Append::First);
        }
        // A transient frame must not poison the chain forever. Reacquiring a
        // verified overlap with the source before a gap replaces that tail.
        if !upward
            && let Some(anchor) = self.joins.iter().position(Option::is_none)
            && align(&self.frames[anchor], &frame).is_some()
        {
            for removed in self.frames.drain(anchor + 1..) {
                self.retained -= removed.rgba.len();
            }
            self.joins.truncate(anchor);
            self.cuts.truncate(anchor);
            self.operations.clear();
            self.review = 0;
            self.cursor = self.cursor.min(anchor);
        }
        let cursor = self.cursor.min(self.frames.len() - 1);
        if self.frames.iter().any(|f| f.rgba == frame.rgba)
            || stationary(&self.frames[cursor], &frame)
        {
            return Ok(Append::Duplicate);
        }
        self.compact_for(frame.rgba.len());
        self.cursor = self.cursor.min(self.frames.len() - 1);
        if self.frames.len() >= MAX_FRAGMENTS || self.retained + frame.rgba.len() > BUDGET {
            return Err(StitchError::Budget);
        }
        // Align the viewport against the frame the user last looked at, then
        // the ends: a revisit lands next to the cursor, new territory extends
        // the strip. `upward` only reorders the cheap candidates.
        let last = self.frames.len() - 1;
        let mut candidates: Vec<usize> = vec![self.cursor, if upward { 0 } else { last }];
        candidates.push(if upward { last } else { 0 });
        candidates.dedup();
        let mut placement: Option<(usize, bool, u32)> = None;
        for candidate in candidates {
            if let Some(overlap) = align(&self.frames[candidate], &frame) {
                placement = Some((candidate, true, overlap));
                break;
            }
            if let Some(overlap) = align(&frame, &self.frames[candidate]) {
                placement = Some((candidate, false, overlap));
                break;
            }
        }
        let Some((candidate, below, overlap)) = placement else {
            // Disconnected source: retained at the hint end for review/repair.
            self.retained += frame.rgba.len();
            let index = if upward {
                self.frames.insert(0, frame);
                self.joins.insert(0, None);
                self.cuts.insert(0, 0);
                0
            } else {
                self.frames.push(frame);
                self.joins.push(None);
                self.cuts.push(0);
                self.frames.len() - 1
            };
            self.operations.push(Operation::End { upward });
            self.cursor = index;
            return Ok(Append::NeedsReview);
        };
        let height = frame.height as i64;
        let positions = self.positions();
        let Some(base) = positions[candidate] else {
            // The candidate itself sits behind an unresolved join; only a
            // verified end extension is safe without position arithmetic.
            let at_end = if below {
                candidate == last
            } else {
                candidate == 0
            };
            return if at_end {
                Ok(self.place_at_end(frame, below, overlap))
            } else {
                Ok(self.hold_unresolved(frame, upward))
            };
        };
        let position = if below {
            base + height - i64::from(overlap)
        } else {
            base + i64::from(overlap) - height
        };
        // Connected extent from the candidate: verified joins only.
        let mut connected_bottom = base + height;
        let mut connected_top = base;
        let mut bound = candidate;
        while below && bound < self.joins.len() {
            match self.joins[bound] {
                Some(_) => {
                    bound += 1;
                    connected_bottom = positions[bound].unwrap_or(connected_bottom) + height;
                }
                None => break,
            }
        }
        while !below && bound > 0 {
            match self.joins[bound - 1] {
                Some(_) => {
                    bound -= 1;
                    connected_top = positions[bound].unwrap_or(connected_top);
                }
                None => break,
            }
        }
        let inside = if below {
            position + height <= connected_bottom
        } else {
            position >= connected_top
        };
        if inside {
            // Revisiting captured content: nothing new, just follow the viewport.
            self.cursor = Self::frame_at(&positions, position, candidate);
            return Ok(Append::Duplicate);
        }
        // Walk to the neighbour pair the frame belongs between.
        let mut index = candidate;
        if below {
            while index + 1 < self.frames.len()
                && matches!(positions[index + 1], Some(p) if p <= position)
            {
                index += 1;
            }
        } else {
            while index > 0 && matches!(positions[index - 1], Some(p) if p >= position) {
                index -= 1;
            }
        }
        // Joins derive from verified positions; alignment only fills unknowns.
        let left = if below || index > 0 {
            let neighbour = if below { index } else { index - 1 };
            match positions[neighbour] {
                Some(p) => clamp_join(p + height - position, height),
                None => align(&self.frames[neighbour], &frame),
            }
        } else {
            None
        };
        let right = if !below || index + 1 < self.frames.len() {
            let neighbour = if below { index + 1 } else { index };
            match positions[neighbour] {
                Some(p) => clamp_join(position + height - p, height),
                None => align(&frame, &self.frames[neighbour]),
            }
        } else {
            None
        };
        self.retained += frame.rgba.len();
        let applied;
        if below {
            let has_right = index + 1 < self.frames.len();
            let restored = if has_right { self.joins[index] } else { None };
            let left = left.unwrap_or(overlap);
            applied = left;
            self.frames.insert(index + 1, frame);
            if has_right {
                self.joins[index] = Some(left);
                self.cuts[index] = left / 2;
                self.joins.insert(index + 1, right);
                self.cuts.insert(index + 1, right.unwrap_or(0) / 2);
                self.operations.push(Operation::Insert {
                    index: index + 1,
                    restored,
                });
            } else {
                self.joins.push(Some(left));
                self.cuts.push(left / 2);
                self.operations.push(Operation::End { upward: false });
            }
            self.cursor = index + 1;
        } else {
            let has_left = index > 0;
            let restored = if has_left {
                self.joins[index - 1]
            } else {
                None
            };
            let right = right.unwrap_or(overlap);
            applied = right;
            self.frames.insert(index, frame);
            if has_left {
                self.joins[index - 1] = left;
                self.cuts[index - 1] = left.unwrap_or(0) / 2;
                self.joins.insert(index, Some(right));
                self.cuts.insert(index, right / 2);
                self.operations.push(Operation::Insert { index, restored });
            } else {
                self.joins.insert(0, Some(right));
                self.cuts.insert(0, right / 2);
                self.operations.push(Operation::End { upward: true });
            }
            self.cursor = index;
        }
        Ok(Append::Added { overlap: applied })
    }

    /// The deepest frame whose verified position contains `position`.
    fn frame_at(positions: &[Option<i64>], position: i64, fallback: usize) -> usize {
        let mut best = fallback;
        let mut best_pos = i64::MIN;
        for (index, known) in positions.iter().enumerate() {
            if let Some(p) = known
                && *p <= position
                && *p >= best_pos
            {
                best = index;
                best_pos = *p;
            }
        }
        best
    }

    /// Extend one end with a verified overlap; no neighbour pair to re-join.
    fn place_at_end(&mut self, frame: Fragment, below: bool, overlap: u32) -> Append {
        self.retained += frame.rgba.len();
        if below {
            self.frames.push(frame);
            self.joins.push(Some(overlap));
            self.cuts.push(overlap / 2);
            self.operations.push(Operation::End { upward: false });
            self.cursor = self.frames.len() - 1;
        } else {
            self.frames.insert(0, frame);
            self.joins.insert(0, Some(overlap));
            self.cuts.insert(0, overlap / 2);
            self.operations.push(Operation::End { upward: true });
            self.cursor = 0;
        }
        Append::Added { overlap }
    }

    /// A frame that matches no verified neighbour stays at the hint end with an
    /// unresolved join, exactly like the previous acquisition model.
    fn hold_unresolved(&mut self, frame: Fragment, upward: bool) -> Append {
        self.retained += frame.rgba.len();
        let index = if upward {
            self.frames.insert(0, frame);
            self.joins.insert(0, None);
            self.cuts.insert(0, 0);
            0
        } else {
            self.frames.push(frame);
            self.joins.push(None);
            self.cuts.push(0);
            self.frames.len() - 1
        };
        self.operations.push(Operation::End { upward });
        self.cursor = index;
        Append::NeedsReview
    }

    pub fn append(&mut self, frame: Fragment) -> Result<Append, StitchError> {
        self.capture(frame, false)
    }
    /// Remove only a redundant intermediate viewport with a separately verified
    /// direct overlap. Do not raise the memory ceiling to retain duplicate rows.
    fn compact_for(&mut self, incoming: usize) {
        if self
            .operations
            .iter()
            .any(|op| !matches!(op, Operation::End { upward: false }))
        {
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
            self.operations.clear();
            if self.cursor >= index {
                self.cursor = self.cursor.saturating_sub(1);
            }
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
        // After a user edits the sequence, recorded insertion points are stale.
        self.operations.clear();
        self.cursor = if self.cursor > index {
            self.cursor - 1
        } else if self.cursor == index {
            index.min(self.frames.len().saturating_sub(1))
        } else {
            self.cursor
        };
        self.review = index.min(self.joins.len());
    }
    pub fn undo(&mut self) {
        let Some(operation) = self.operations.pop() else {
            return;
        };
        match operation {
            Operation::End { upward } => {
                if self.frames.len() <= 1 {
                    return;
                }
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
                if upward && self.cursor > 0 {
                    self.cursor -= 1;
                }
                self.cursor = self.cursor.min(self.frames.len() - 1);
            }
            Operation::Insert { index, restored } => {
                if index >= self.frames.len() {
                    return;
                }
                let frame = self.frames.remove(index);
                self.retained -= frame.rgba.len();
                self.joins.remove(index);
                if index > 0 && index - 1 < self.joins.len() {
                    self.joins[index - 1] = restored;
                    self.cuts[index - 1] = restored.unwrap_or(0) / 2;
                }
                if self.cursor >= index && self.cursor > 0 {
                    self.cursor -= 1;
                }
                self.cursor = self.cursor.min(self.frames.len() - 1);
            }
        }
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
                self.operations.clear();
                if self.cursor >= next {
                    self.cursor -= next - index - 1;
                } else if self.cursor > index {
                    self.cursor = index + 1;
                }
            }
            index += 1;
        }
        self.cursor = self.cursor.min(self.frames.len().saturating_sub(1));
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

/// Derived overlaps must stay inside one frame height; a contradiction keeps
/// the join unresolved instead of inventing geometry.
fn clamp_join(value: i64, height: i64) -> Option<u32> {
    if value < 0 || value >= height {
        None
    } else {
        Some(value as u32)
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
