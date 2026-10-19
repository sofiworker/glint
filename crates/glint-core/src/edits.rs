use crate::{Annotation, Rect};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
struct Change {
    index: usize,
    before: Option<Arc<Annotation>>,
    after: Option<Arc<Annotation>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GlyphMask, Point, Redaction, Stroke, TextAnnotation};
    fn mark(x: u32) -> Annotation {
        Redaction {
            bounds: Rect {
                x,
                y: 10,
                width: 20,
                height: 20,
            },
            color: [0, 0, 0, 255],
        }
        .into()
    }
    #[test]
    fn replacement_deletion_and_branch_preserve_order() {
        let mut e = Edits::default();
        e.push(mark(10));
        e.push(mark(40));
        e.push(mark(70));
        let old = e.marks().to_vec();
        e.replace(1, mark(50)).unwrap();
        e.remove(0).unwrap();
        e.remove(1).unwrap();
        assert_eq!(
            e.marks().iter().map(|m| m.bounds().x).collect::<Vec<_>>(),
            vec![50]
        );
        e.undo();
        e.undo();
        e.undo();
        assert_eq!(e.marks(), old);
        e.redo();
        assert_eq!(e.marks()[1].bounds().x, 50);
        e.replace(1, mark(50)).unwrap();
        assert!(e.can_redo()); // no-op retains redo
        assert!(e.replace(99, mark(1)).is_none());
        assert!(e.can_redo());
        e.remove(1).unwrap();
        assert!(!e.can_redo());
        e.undo();
        assert_eq!(e.marks()[1].bounds().x, 50);
        while e.can_undo() {
            e.undo();
        }
        assert!(e.marks().is_empty());
        while e.can_redo() {
            e.redo();
        }
        assert_eq!(e.marks().len(), 2);
    }
    #[test]
    fn history_counts_old_stroke_versions_and_shares_text_masks() {
        let mut e = Edits::default();
        e.push(Stroke {
            points: vec![Point { x: 20., y: 20. }; 65536],
            width: 3.,
            color: [0; 4],
        });
        for x in 1..15 {
            let m = e.marks()[0].translated(x, 0, (1000, 1000)).unwrap();
            assert!(e.replace(0, m).is_some());
        }
        let m = e.marks()[0].translated(1, 0, (1000, 1000)).unwrap();
        assert!(e.replace(0, m).is_none());
        let retained_strokes = e.retained_bytes();
        while e.can_undo() {
            e.undo();
        }
        assert!(e.marks().is_empty());
        assert!(e.retained_bytes() >= retained_strokes / 2); // Redo still owns the stroke versions.
        let mut e = Edits::default();
        e.push(TextAnnotation {
            origin: (10, 10),
            text: "shared".into(),
            font: "font".into(),
            size: 22,
            color: [0; 4],
            mask: GlyphMask {
                width: 1024,
                height: 1024,
                coverage: vec![255; 1024 * 1024].into(),
            },
        });
        for _ in 0..30 {
            let m = e.marks()[0].translated(1, 0, (4096, 4096)).unwrap();
            assert!(e.replace(0, m).is_some());
        }
        assert!(e.retained_bytes() < 2 * 1024 * 1024); // The 1 MiB glyph mask is shared by all versions.
        let Annotation::Text(t) = e.marks()[0].as_ref() else {
            panic!()
        };
        let ptr = t.mask.coverage.as_ptr();
        e.undo();
        let Annotation::Text(t) = e.marks()[0].as_ref() else {
            panic!()
        };
        assert_eq!(ptr, t.mask.coverage.as_ptr());
    }
}
impl Change {
    fn damage(&self) -> Rect {
        self.before
            .iter()
            .chain(&self.after)
            .map(|m| m.bounds())
            .fold(Rect::default(), Rect::union)
    }
}
#[derive(Clone, Default)]
pub struct Edits {
    marks: Vec<Arc<Annotation>>,
    undo: Vec<Change>,
    redo: Vec<Change>,
}
impl Edits {
    /// Restore current editable objects as a new session baseline, without old undo data.
    pub fn from_current(marks: Vec<Annotation>) -> Option<Self> {
        let mut edits = Self::default();
        for mark in marks {
            if !edits.push(mark) {
                return None;
            }
        }
        edits.undo.clear();
        Some(edits)
    }
    pub fn marks(&self) -> &[Arc<Annotation>] {
        &self.marks
    }
    /// Estimate the unique versions retained by this edit history, including redo.
    pub fn retained_bytes(&self) -> u64 {
        let mut objects = HashSet::new();
        let mut masks = HashSet::new();
        let mut bytes = std::mem::size_of::<Self>() as u64
            + (self.marks.capacity() * std::mem::size_of::<Arc<Annotation>>()) as u64
            + ((self.undo.capacity() + self.redo.capacity()) * std::mem::size_of::<Change>())
                as u64;
        for mark in self.marks.iter().chain(
            self.undo
                .iter()
                .chain(&self.redo)
                .flat_map(|c| c.before.iter().chain(&c.after)),
        ) {
            if !objects.insert(Arc::as_ptr(mark)) {
                continue;
            }
            bytes += (std::mem::size_of::<Annotation>() + 2 * std::mem::size_of::<usize>()) as u64;
            match mark.as_ref() {
                Annotation::Stroke(s) => {
                    bytes += (s.points.capacity() * std::mem::size_of::<crate::Point>()) as u64
                }
                Annotation::Text(t) => {
                    bytes += (t.text.capacity() + t.font.capacity()) as u64;
                    if masks.insert(t.mask.coverage.as_ptr()) {
                        bytes +=
                            t.mask.coverage.len() as u64 + 2 * std::mem::size_of::<usize>() as u64;
                    }
                }
                _ => {}
            }
        }
        bytes
    }
    /// Ephemeral display list, sharing unchanged objects and carrying no undo history.
    pub fn preview_replace(&self, index: usize, mark: Annotation) -> Option<Self> {
        if index >= self.marks.len() || !mark.is_valid() {
            return None;
        }
        let mut marks = self.marks.clone();
        marks[index] = Arc::new(mark);
        Some(Self {
            marks,
            undo: vec![],
            redo: vec![],
        })
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    fn accepts(&self, mark: &Annotation, removed: Option<usize>) -> bool {
        if !mark.is_valid()
            || self.undo.len() >= 4096
            || self.marks.len() - usize::from(removed.is_some()) >= 4096
        {
            return false;
        }
        // New commits discard redo. Count every retained object version once,
        // and shared glyph masks once even across many moves of the same text.
        let mut objects = HashSet::new();
        let mut masks = HashSet::new();
        let mut points = 0;
        let mut bytes = 0;
        for m in self
            .marks
            .iter()
            .chain(
                self.undo
                    .iter()
                    .flat_map(|c| c.before.iter().chain(&c.after)),
            )
            .map(Arc::as_ref)
            .chain(std::iter::once(mark))
        {
            if objects.insert(m as *const Annotation) {
                points += m.points_len();
            }
            if let Annotation::Text(t) = m
                && masks.insert(t.mask.coverage.as_ptr())
            {
                bytes += t.mask.coverage.len();
            }
        }
        points <= 1_000_000 && bytes <= 16 * 1024 * 1024
    }
    fn apply(&mut self, change: &Change, forward: bool) {
        let (before, after) = if forward {
            (&change.before, &change.after)
        } else {
            (&change.after, &change.before)
        };
        match (before, after) {
            (None, Some(mark)) => self.marks.insert(change.index, mark.clone()),
            (Some(_), None) => {
                self.marks.remove(change.index);
            }
            (Some(_), Some(mark)) => self.marks[change.index] = mark.clone(),
            _ => {}
        }
    }
    fn commit(&mut self, change: Change) -> Rect {
        let damage = change.damage();
        self.apply(&change, true);
        self.undo.push(change);
        self.redo.clear();
        damage
    }
    pub fn push(&mut self, mark: impl Into<Annotation>) -> bool {
        let mark = mark.into();
        if !self.accepts(&mark, None) {
            return false;
        }
        self.commit(Change {
            index: self.marks.len(),
            before: None,
            after: Some(Arc::new(mark)),
        });
        true
    }
    pub fn replace(&mut self, index: usize, mark: impl Into<Annotation>) -> Option<Rect> {
        let mark = mark.into();
        let before = self.marks.get(index)?.clone();
        if before.as_ref() == &mark {
            return Some(Rect::default());
        }
        if !self.accepts(&mark, Some(index)) {
            return None;
        }
        Some(self.commit(Change {
            index,
            before: Some(before),
            after: Some(Arc::new(mark)),
        }))
    }
    pub fn remove(&mut self, index: usize) -> Option<Rect> {
        if self.undo.len() >= 4096 {
            return None;
        }
        let before = self.marks.get(index)?.clone();
        Some(self.commit(Change {
            index,
            before: Some(before),
            after: None,
        }))
    }
    pub fn undo(&mut self) -> Rect {
        let Some(change) = self.undo.pop() else {
            return Rect::default();
        };
        let damage = change.damage();
        self.apply(&change, false);
        self.redo.push(change);
        damage
    }
    pub fn redo(&mut self) -> Rect {
        let Some(change) = self.redo.pop() else {
            return Rect::default();
        };
        let damage = change.damage();
        self.apply(&change, true);
        self.undo.push(change);
        damage
    }
}
