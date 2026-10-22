//! Language negotiation shared by presentation adapters, independent of copy IDs.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    ZhCn,
}

impl Locale {
    /// Unsupported languages return `None` so the next OS preference can win.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let tag = tag.replace('_', "-").to_ascii_lowercase();
        let mut parts = tag.split('-');
        match parts.next()? {
            "en" => Some(Self::En),
            "zh" if !parts.any(|part| matches!(part, "hant" | "tw" | "hk" | "mo")) => {
                Some(Self::ZhCn)
            }
            _ => None,
        }
    }

    pub fn detect(preferred: &[String]) -> Option<Self> {
        preferred.iter().find_map(|tag| Self::from_tag(tag))
    }
}
