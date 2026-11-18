//! Non-destructive, single-document editor. History shares annotation payloads;
//! source pixels are owned separately and never copied into history.
use crate::{Annotation, Point, Rect};
use std::collections::HashSet;
use std::sync::Arc;

pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
const HISTORY: usize = 64;
const CURRENT_BYTES: u64 = 8 * 1024 * 1024;
const HISTORY_BYTES: u64 = 32 * 1024 * 1024;

fn payload_bytes<'a>(states: impl Iterator<Item = &'a State>) -> u64 {
    let mut annotations = HashSet::new();
    let mut masks = HashSet::new();
    let mut bytes = 0;
    for state in states {
        for mark in &state.marks {
            if annotations.insert(Arc::as_ptr(&mark.annotation)) {
                bytes += std::mem::size_of::<Annotation>() as u64;
                match mark.annotation.as_ref() {
                    Annotation::Stroke(s) => {
                        bytes += (s.points.len() * std::mem::size_of::<Point>()) as u64
                    }
                    Annotation::Text(t) => {
                        bytes += (t.text.len() + t.font.len()) as u64;
                        if masks.insert(t.mask.coverage.as_ptr()) {
                            bytes += t.mask.coverage.len() as u64;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    bytes
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    pub start: f64,
    pub end: f64,
}
impl Clip {
    pub fn duration(self) -> f64 {
        self.end - self.start
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub annotation: Arc<Annotation>,
    /// Source-media times, so deleting a preceding clip does not move a label.
    pub from: f64,
    pub to: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transform {
    pub crop: Rect,
    pub turns: u8,
    pub flip: bool,
    pub size: Option<(u32, u32)>,
    pub brightness: i16,
    pub contrast: i16,
    pub saturation: i16,
}
impl Transform {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            crop: Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            turns: 0,
            flip: false,
            size: None,
            brightness: 100,
            contrast: 100,
            saturation: 100,
        }
    }
    pub fn natural_size(self) -> (u32, u32) {
        if self.turns.is_multiple_of(2) {
            (self.crop.width, self.crop.height)
        } else {
            (self.crop.height, self.crop.width)
        }
    }
    pub fn dimensions(self) -> (u32, u32) {
        self.size.unwrap_or_else(|| self.natural_size())
    }
    /// Map continuous edge coordinates, not pixel centres.
    pub fn project(self, p: Point) -> Point {
        let (x, y) = (p.x - self.crop.x as f32, p.y - self.crop.y as f32);
        let (w, h) = (self.crop.width as f32, self.crop.height as f32);
        let (mut x, y) = match self.turns % 4 {
            1 => (h - y, x),
            2 => (w - x, h - y),
            3 => (y, w - x),
            _ => (x, y),
        };
        let (nw, nh) = self.natural_size();
        if self.flip {
            x = nw as f32 - x;
        }
        let (ow, oh) = self.dimensions();
        Point {
            x: x * ow as f32 / nw as f32,
            y: y * oh as f32 / nh as f32,
        }
    }
    pub fn unproject(self, p: Point) -> Point {
        let (nw, nh) = self.natural_size();
        let (ow, oh) = self.dimensions();
        let mut x = p.x * nw as f32 / ow as f32;
        let y = p.y * nh as f32 / oh as f32;
        if self.flip {
            x = nw as f32 - x;
        }
        let (w, h) = (self.crop.width as f32, self.crop.height as f32);
        let (x, y) = match self.turns % 4 {
            1 => (y, h - x),
            2 => (w - x, h - y),
            3 => (w - y, x),
            _ => (x, y),
        };
        Point {
            x: x + self.crop.x as f32,
            y: y + self.crop.y as f32,
        }
    }
    pub fn cropped(self, rect: Rect) -> Option<Self> {
        let (w, h) = self.dimensions();
        if rect.width == 0 || rect.height == 0 || rect.clipped(w, h) != rect {
            return None;
        }
        let a = self.unproject(Point {
            x: rect.x as f32,
            y: rect.y as f32,
        });
        let b = self.unproject(Point {
            x: (rect.x + rect.width) as f32,
            y: (rect.y + rect.height) as f32,
        });
        let x = a.x.min(b.x).round() as u32;
        let y = a.y.min(b.y).round() as u32;
        let right = a.x.max(b.x).round() as u32;
        let bottom = a.y.max(b.y).round() as u32;
        let crop = Rect {
            x,
            y,
            width: right.saturating_sub(x).max(1),
            height: bottom.saturating_sub(y).max(1),
        };
        Some(Self {
            crop,
            size: Some((rect.width, rect.height)),
            ..self
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub transform: Transform,
    pub marks: Vec<Mark>,
    pub clips: Vec<Clip>,
    pub volume: u16,
    pub muted: bool,
}
impl State {
    pub fn duration(&self) -> f64 {
        self.clips.iter().map(|c| c.duration()).sum()
    }
    pub fn source_time(&self, mut time: f64) -> f64 {
        for clip in &self.clips {
            if time < clip.duration() {
                return clip.start + time.max(0.);
            }
            time -= clip.duration();
        }
        self.clips.last().map_or(0., |c| c.end)
    }
    pub fn split(&mut self, time: f64) -> bool {
        let mut offset = 0.;
        for i in 0..self.clips.len() {
            let c = self.clips[i];
            let local = time - offset;
            if local > 0.001 && local < c.duration() - 0.001 {
                self.clips[i].end = c.start + local;
                self.clips.insert(
                    i + 1,
                    Clip {
                        start: c.start + local,
                        end: c.end,
                    },
                );
                return true;
            }
            offset += c.duration();
        }
        false
    }
}
pub struct Editor {
    pub state: State,
    saved: State,
    undo: Vec<State>,
    redo: Vec<State>,
    dimensions: (u32, u32),
    duration: f64,
}
impl Editor {
    pub fn new(
        width: u32,
        height: u32,
        marks: Vec<Arc<Annotation>>,
        duration: f64,
    ) -> Result<Self, &'static str> {
        if width == 0
            || height == 0
            || u64::from(width) * u64::from(height) > MAX_PIXELS
            || !duration.is_finite()
            || duration < 0.
        {
            return Err("Document exceeds editor budget");
        }
        let state = State {
            transform: Transform::new(width, height),
            marks: marks
                .into_iter()
                .map(|annotation| Mark {
                    annotation,
                    from: 0.,
                    to: duration,
                })
                .collect(),
            clips: if duration > 0. {
                vec![Clip {
                    start: 0.,
                    end: duration,
                }]
            } else {
                vec![]
            },
            volume: 100,
            muted: false,
        };
        Ok(Self {
            saved: state.clone(),
            state,
            undo: vec![],
            redo: vec![],
            dimensions: (width, height),
            duration,
        })
    }
    pub fn apply(&mut self, next: State) -> Result<bool, &'static str> {
        let t = next.transform;
        let (w, h) = t.dimensions();
        if t.crop.width == 0
            || t.crop.height == 0
            || t.crop.clipped(self.dimensions.0, self.dimensions.1) != t.crop
            || w == 0
            || h == 0
            || u64::from(w) * u64::from(h) > MAX_PIXELS
            || [t.brightness, t.contrast, t.saturation]
                .iter()
                .any(|v| !(0..=200).contains(v))
            || next.marks.len() > 256
            || payload_bytes(std::iter::once(&next)) > CURRENT_BYTES
            || next.marks.iter().any(|m| {
                !m.annotation.is_valid()
                    || !m.from.is_finite()
                    || !m.to.is_finite()
                    || m.from < 0.
                    || m.to < m.from
                    || m.to > self.duration
            })
            || next.clips.len() > 256
            || next.volume > 200
            || next.clips.iter().any(|c| {
                !c.start.is_finite()
                    || !c.end.is_finite()
                    || c.start < 0.
                    || c.end > self.duration
                    || c.duration() < 0.001
            })
            || (self.duration > 0. && next.clips.is_empty())
        {
            return Err("Invalid editor change");
        }
        if next == self.state {
            return Ok(false);
        }
        self.undo.push(std::mem::replace(&mut self.state, next));
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        while !self.undo.is_empty()
            && payload_bytes(self.undo.iter().chain([&self.state, &self.saved])) > HISTORY_BYTES
        {
            self.undo.remove(0);
        }
        Ok(true)
    }
    pub fn dirty(&self) -> bool {
        self.state != self.saved
    }
    pub fn saved(&mut self) {
        self.saved = self.state.clone();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, s));
        }
    }
    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, s));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_crop_and_cancel_are_reversible_in_all_orientations() {
        for turns in 0..4 {
            for flip in [false, true] {
                let t = Transform {
                    turns,
                    flip,
                    size: Some((600, 300)),
                    ..Transform::new(400, 200)
                };
                let p = Point { x: 34., y: 67. };
                let round = t.unproject(t.project(p));
                assert!((p.x - round.x).abs() < 0.001 && (p.y - round.y).abs() < 0.001);
                let crop = t
                    .cropped(Rect {
                        x: 60,
                        y: 30,
                        width: 300,
                        height: 150,
                    })
                    .unwrap();
                assert_eq!(crop.dimensions(), (300, 150));
                assert!(crop.crop.width > 0 && crop.crop.height > 0);
            }
        }
        let mut e = Editor::new(400, 200, vec![], 0.).unwrap();
        let mut draft = e.state.clone();
        draft.transform.turns = 1;
        draft.transform.flip = true;
        assert!(!e.dirty()); // draft is not applied
        draft.transform = draft
            .transform
            .cropped(Rect {
                x: 10,
                y: 20,
                width: 100,
                height: 200,
            })
            .unwrap();
        e.apply(draft).unwrap();
        e.undo();
        assert!(!e.dirty());
        assert!(!e.can_undo());
        e.redo();
        assert!(e.dirty());
        e.saved();
        assert!(!e.dirty());
    }
    #[test]
    fn clips_preserve_source_times_and_reject_empty_or_invalid_edits() {
        let mut e = Editor::new(96, 64, vec![], 10.).unwrap();
        let mut s = e.state.clone();
        assert!(s.split(4.));
        s.clips.remove(0);
        assert_eq!(s.source_time(2.), 6.);
        e.apply(s).unwrap();
        assert_eq!(e.state.duration(), 6.);
        e.undo();
        assert_eq!(e.state.duration(), 10.);
        let mut s = e.state.clone();
        s.clips.clear();
        assert!(e.apply(s).is_err());
        let mut s = e.state.clone();
        s.transform.size = Some((u32::MAX, 2));
        assert!(e.apply(s).is_err());
        assert!(!e.dirty());
    }
}
