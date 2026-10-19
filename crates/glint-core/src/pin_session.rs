//! Versioned, bounded pin-session assets. Contains source pixels and current editable objects.
//! Never use this private format for ordinary image export or portable configuration.
use crate::{
    Annotation, Edits, GlyphMask, ImageDocument, Pixelation, Point, Rect, Redaction, Shape,
    ShapeKind, Stroke, TextAnnotation,
};
use std::sync::Arc;
pub const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"GLNTPIN1";
const INVALID: &str = "Invalid or unsupported pin session";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub zoom: u16,
    pub opacity: u8,
    pub turns: u8,
    pub on_top: bool,
    pub locked: bool,
}
impl Placement {
    fn valid(self) -> bool {
        (1..=200).contains(&self.zoom) && (20..=100).contains(&self.opacity) && self.turns < 4
    }
}
#[derive(Clone)]
pub struct Pin {
    pub placement: Placement,
    pub document: Arc<ImageDocument>,
}
fn checksum(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |v, b| {
        (v ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
struct Writer(Vec<u8>);
impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > MAX_BYTES - 8)
        {
            return Err("Pin session exceeds 256 MiB");
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| "Cannot allocate pin session")?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn n(&mut self, n: u32) -> Result<(), &'static str> {
        self.put(&n.to_le_bytes())
    }
    fn point(&mut self, p: Point) -> Result<(), &'static str> {
        self.n(p.x.to_bits())?;
        self.n(p.y.to_bits())
    }
    fn rect(&mut self, r: Rect) -> Result<(), &'static str> {
        for n in [r.x, r.y, r.width, r.height] {
            self.n(n)?;
        }
        Ok(())
    }
    fn string(&mut self, s: &str) -> Result<(), &'static str> {
        self.n(s.len().try_into().map_err(|_| INVALID)?)?;
        self.put(s.as_bytes())
    }
    fn mark(&mut self, m: &Annotation) -> Result<(), &'static str> {
        if !m.is_valid() {
            return Err(INVALID);
        }
        match m {
            Annotation::Redaction(m) => {
                self.n(0)?;
                self.rect(m.bounds)?;
                self.put(&m.color)?;
            }
            Annotation::Pixelation(m) => {
                self.n(1)?;
                self.rect(m.bounds)?;
            }
            Annotation::Stroke(m) => {
                self.n(2)?;
                self.n(m.width.to_bits())?;
                self.put(&m.color)?;
                self.n(m.points.len() as u32)?;
                for p in &m.points {
                    self.point(*p)?;
                }
            }
            Annotation::Shape(m) => {
                self.n(3)?;
                self.n(match m.kind {
                    ShapeKind::Rectangle => 0,
                    ShapeKind::Arrow { filled: false } => 1,
                    ShapeKind::Arrow { filled: true } => 2,
                    ShapeKind::Highlight => 3,
                })?;
                self.n(m.width.to_bits())?;
                self.put(&m.color)?;
                self.point(m.start)?;
                self.point(m.end)?;
            }
            Annotation::Text(m) => {
                self.n(4)?;
                self.n(m.origin.0)?;
                self.n(m.origin.1)?;
                self.n(u32::from(m.size))?;
                self.put(&m.color)?;
                self.string(&m.text)?;
                self.string(&m.font)?;
                self.n(m.mask.width)?;
                self.n(m.mask.height)?;
                self.put(&m.mask.coverage)?;
            }
        }
        Ok(())
    }
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        let (v, tail) = self.0.split_at_checked(n).ok_or(INVALID)?;
        self.0 = tail;
        Ok(v)
    }
    fn n(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| INVALID)?,
        ))
    }
    fn point(&mut self) -> Result<Point, &'static str> {
        Ok(Point {
            x: f32::from_bits(self.n()?),
            y: f32::from_bits(self.n()?),
        })
    }
    fn rect(&mut self) -> Result<Rect, &'static str> {
        Ok(Rect {
            x: self.n()?,
            y: self.n()?,
            width: self.n()?,
            height: self.n()?,
        })
    }
    fn color(&mut self) -> Result<[u8; 4], &'static str> {
        self.take(4)?.try_into().map_err(|_| INVALID)
    }
    fn string(&mut self, max: usize) -> Result<String, &'static str> {
        let len = self.n()? as usize;
        if len > max {
            return Err(INVALID);
        }
        Ok(std::str::from_utf8(self.take(len)?)
            .map_err(|_| INVALID)?
            .to_owned())
    }
    fn mark(&mut self) -> Result<Annotation, &'static str> {
        let m = match self.n()? {
            0 => Annotation::Redaction(Redaction {
                bounds: self.rect()?,
                color: self.color()?,
            }),
            1 => Annotation::Pixelation(Pixelation {
                bounds: self.rect()?,
            }),
            2 => {
                let width = f32::from_bits(self.n()?);
                let color = self.color()?;
                let count = self.n()? as usize;
                if count > 65536 || count > self.0.len() / 8 {
                    return Err(INVALID);
                }
                let mut points = Vec::with_capacity(count);
                for _ in 0..count {
                    points.push(self.point()?);
                }
                Annotation::Stroke(Stroke {
                    points,
                    width,
                    color,
                })
            }
            3 => {
                let kind = match self.n()? {
                    0 => ShapeKind::Rectangle,
                    1 => ShapeKind::Arrow { filled: false },
                    2 => ShapeKind::Arrow { filled: true },
                    3 => ShapeKind::Highlight,
                    _ => return Err(INVALID),
                };
                Annotation::Shape(Shape {
                    kind,
                    width: f32::from_bits(self.n()?),
                    color: self.color()?,
                    start: self.point()?,
                    end: self.point()?,
                })
            }
            4 => {
                let origin = (self.n()?, self.n()?);
                let size = self.n()?.try_into().map_err(|_| INVALID)?;
                let color = self.color()?;
                let text = self.string(320)?;
                let font = self.string(128)?;
                let (width, height) = (self.n()?, self.n()?);
                let len = (width as usize)
                    .checked_mul(height as usize)
                    .filter(|n| *n <= 4 * 1024 * 1024)
                    .ok_or(INVALID)?;
                Annotation::Text(TextAnnotation {
                    origin,
                    text,
                    font,
                    size,
                    color,
                    mask: GlyphMask {
                        width,
                        height,
                        coverage: self.take(len)?.into(),
                    },
                })
            }
            _ => return Err(INVALID),
        };
        if !m.is_valid() {
            return Err(INVALID);
        }
        Ok(m)
    }
}
pub fn encode(pins: &[Pin]) -> Result<Vec<u8>, &'static str> {
    if pins.len() > 64 {
        return Err(INVALID);
    }
    let mut w = Writer(Vec::new());
    w.put(MAGIC)?;
    w.n(pins.len() as u32)?;
    for pin in pins {
        let p = pin.placement;
        if pin.document.source().len() > 128 * 1024 * 1024 {
            return Err("Pin source exceeds session budget");
        }
        if !p.valid() {
            return Err(INVALID);
        }
        for n in [
            p.x as u32,
            p.y as u32,
            u32::from(p.zoom),
            u32::from(p.opacity),
            u32::from(p.turns),
            u32::from(p.on_top),
            u32::from(p.locked),
        ] {
            w.n(n)?;
        }
        let (width, height) = pin.document.dimensions();
        w.n(width)?;
        w.n(height)?;
        w.put(pin.document.source())?;
        let marks = pin.document.edits().marks();
        if marks.len() > 4096 {
            return Err(INVALID);
        }
        w.n(marks.len() as u32)?;
        for mark in marks {
            w.mark(mark)?;
        }
    }
    w.0.extend_from_slice(&checksum(&w.0).to_le_bytes());
    Ok(w.0)
}
pub fn decode(bytes: &[u8]) -> Result<Vec<Pin>, &'static str> {
    if bytes.len() < 20 || bytes.len() > MAX_BYTES {
        return Err(INVALID);
    }
    let (data, check) = bytes.split_at(bytes.len() - 8);
    if check != checksum(data).to_le_bytes() {
        return Err(INVALID);
    }
    let mut r = Reader(data);
    if r.take(8)? != MAGIC {
        return Err(INVALID);
    }
    let count = r.n()?;
    if count > 64 {
        return Err(INVALID);
    }
    let mut pins = Vec::new();
    for _ in 0..count {
        let p = Placement {
            x: r.n()? as i32,
            y: r.n()? as i32,
            zoom: r.n()?.try_into().map_err(|_| INVALID)?,
            opacity: r.n()?.try_into().map_err(|_| INVALID)?,
            turns: r.n()?.try_into().map_err(|_| INVALID)?,
            on_top: match r.n()? {
                0 => false,
                1 => true,
                _ => return Err(INVALID),
            },
            locked: match r.n()? {
                0 => false,
                1 => true,
                _ => return Err(INVALID),
            },
        };
        if !p.valid() {
            return Err(INVALID);
        }
        let (width, height) = (r.n()?, r.n()?);
        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n > 0 && *n <= 128 * 1024 * 1024)
            .ok_or(INVALID)?;
        let source = r.take(length)?;
        let marks = r.n()? as usize;
        if marks > 4096 {
            return Err(INVALID);
        }
        let mut objects = Vec::new();
        for _ in 0..marks {
            objects.push(r.mark()?);
        }
        let edits = Edits::from_current(objects).ok_or(INVALID)?;
        let document = ImageDocument::from_selection(
            source,
            width,
            height,
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            &edits,
        )?;
        pins.push(Pin {
            placement: p,
            document: Arc::new(document),
        });
    }
    if !r.0.is_empty() {
        return Err(INVALID);
    }
    Ok(pins)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_objects_source_and_signed_placement_roundtrip_without_old_undo() {
        let mut e = Edits::default();
        e.push(Redaction {
            bounds: Rect {
                x: 1,
                y: 2,
                width: 4,
                height: 3,
            },
            color: [0, 0, 0, 255],
        });
        e.push(Pixelation {
            bounds: Rect {
                x: 3,
                y: 3,
                width: 3,
                height: 4,
            },
        });
        e.push(Stroke {
            points: vec![Point { x: 1., y: 1. }, Point { x: 8., y: 7. }],
            width: 3.,
            color: [10, 20, 30, 255],
        });
        e.push(Shape {
            kind: ShapeKind::Arrow { filled: true },
            start: Point { x: 2., y: 3. },
            end: Point { x: 7., y: 8. },
            width: 2.,
            color: [10, 20, 30, 255],
        });
        e.push(TextAnnotation {
            origin: (1, 1),
            text: "test".into(),
            font: "font".into(),
            size: 22,
            color: [2, 3, 4, 255],
            mask: GlyphMask {
                width: 2,
                height: 2,
                coverage: vec![0, 64, 128, 255].into(),
            },
        });
        let doc = Arc::new(
            ImageDocument::from_selection(
                &[13, 47, 91, 128].repeat(100),
                10,
                10,
                Rect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                },
                &e,
            )
            .unwrap(),
        );
        let placement = Placement {
            x: -1800,
            y: -20,
            zoom: 63,
            opacity: 55,
            turns: 3,
            on_top: false,
            locked: true,
        };
        let bytes = encode(&[Pin {
            placement,
            document: doc.clone(),
        }])
        .unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded[0].placement, placement);
        assert_eq!(decoded[0].document.source(), doc.source());
        assert_eq!(decoded[0].document.edits().marks(), e.marks());
        assert!(!decoded[0].document.edits().can_undo());
        let mut edited = decoded[0].document.edits().clone();
        assert!(edited.remove(0).is_some());
        edited.undo();
        assert_eq!(edited.marks(), e.marks());
        let mut corrupt = bytes.clone();
        corrupt[50] ^= 1;
        assert!(decode(&corrupt).is_err());
        for (at, value) in [
            (20, 0u32),
            (8, 65),
            (40, u32::MAX),
            (500, f32::NAN.to_bits()),
        ] {
            let mut forged = bytes[..bytes.len() - 8].to_vec();
            forged[at..at + 4].copy_from_slice(&value.to_le_bytes());
            forged.extend_from_slice(&checksum(&forged).to_le_bytes());
            assert!(decode(&forged).is_err());
        }
        let mut future = bytes[..bytes.len() - 8].to_vec();
        future[7] = b'2';
        future.extend_from_slice(&checksum(&future).to_le_bytes());
        assert!(decode(&future).is_err());
        for cut in [0, 8, 12, bytes.len() - 1] {
            assert!(decode(&bytes[..cut]).is_err());
        }
        let mut extra = bytes[..bytes.len() - 8].to_vec();
        extra.push(0);
        extra.extend_from_slice(&checksum(&extra).to_le_bytes());
        assert!(decode(&extra).is_err());
        assert!(decode(&encode(&[]).unwrap()).unwrap().is_empty());
    }
}
