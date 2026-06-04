//! Dynamic terminal palette state.

/// A concrete RGB color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgbColor {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// The role a color is being resolved for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorRole {
    /// Foreground text color.
    Foreground,
    /// Background cell color.
    Background,
    /// Underline color.
    Underline,
    /// Cursor color.
    Cursor,
}

/// A color after applying dynamic palette overrides where possible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedColor {
    /// The terminal default color.
    Default,
    /// An unresolved indexed terminal color.
    Indexed(u8),
    /// A resolved RGB color.
    Rgb(RgbColor),
}

/// Dynamic xterm palette overrides tracked from OSC color sequences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    indexed: [Option<RgbColor>; 256],
    foreground: Option<RgbColor>,
    background: Option<RgbColor>,
    cursor: Option<RgbColor>,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            indexed: [None; 256],
            foreground: None,
            background: None,
            cursor: None,
        }
    }
}

impl Palette {
    /// Returns the RGB override for an indexed palette entry.
    #[must_use]
    pub fn indexed(&self, index: u8) -> Option<RgbColor> {
        self.indexed[usize::from(index)]
    }

    /// Returns the default foreground override.
    #[must_use]
    pub fn foreground(&self) -> Option<RgbColor> {
        self.foreground
    }

    /// Returns the default background override.
    #[must_use]
    pub fn background(&self) -> Option<RgbColor> {
        self.background
    }

    /// Returns the cursor color override.
    #[must_use]
    pub fn cursor(&self) -> Option<RgbColor> {
        self.cursor
    }

    pub(crate) fn set_indexed(&mut self, index: u8, color: RgbColor) {
        self.indexed[usize::from(index)] = Some(color);
    }

    pub(crate) fn reset_indexed(&mut self, index: Option<u8>) {
        if let Some(index) = index {
            self.indexed[usize::from(index)] = None;
        } else {
            self.indexed = [None; 256];
        }
    }

    pub(crate) fn set_foreground(&mut self, color: RgbColor) {
        self.foreground = Some(color);
    }

    pub(crate) fn reset_foreground(&mut self) {
        self.foreground = None;
    }

    pub(crate) fn set_background(&mut self, color: RgbColor) {
        self.background = Some(color);
    }

    pub(crate) fn reset_background(&mut self) {
        self.background = None;
    }

    pub(crate) fn set_cursor(&mut self, color: RgbColor) {
        self.cursor = Some(color);
    }

    pub(crate) fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    /// Returns true when no dynamic palette override is set.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.foreground.is_none()
            && self.background.is_none()
            && self.cursor.is_none()
            && self.indexed.iter().all(Option::is_none)
    }

    /// Resolves a semantic terminal color through dynamic palette overrides.
    #[must_use]
    pub fn resolve_color(
        &self,
        color: crate::Color,
        role: ColorRole,
    ) -> ResolvedColor {
        match color {
            crate::Color::Rgb(r, g, b) => {
                ResolvedColor::Rgb(RgbColor { r, g, b })
            }
            crate::Color::Idx(index) => self
                .indexed(index)
                .map_or(ResolvedColor::Indexed(index), ResolvedColor::Rgb),
            crate::Color::Default => match role {
                ColorRole::Foreground | ColorRole::Underline => self
                    .foreground
                    .map_or(ResolvedColor::Default, ResolvedColor::Rgb),
                ColorRole::Background => self
                    .background
                    .map_or(ResolvedColor::Default, ResolvedColor::Rgb),
                ColorRole::Cursor => self
                    .cursor
                    .map_or(ResolvedColor::Default, ResolvedColor::Rgb),
            },
        }
    }

    pub(crate) fn write_osc_setup(&self, contents: &mut Vec<u8>) {
        for (index, color) in self.indexed.iter().enumerate() {
            if let Some(color) = color {
                write_osc_color(contents, b"4", Some(index), *color);
            }
        }
        if let Some(color) = self.foreground {
            write_osc_color(contents, b"10", None, color);
        }
        if let Some(color) = self.background {
            write_osc_color(contents, b"11", None, color);
        }
        if let Some(color) = self.cursor {
            write_osc_color(contents, b"12", None, color);
        }
    }
}

fn write_osc_color(
    contents: &mut Vec<u8>,
    code: &[u8],
    index: Option<usize>,
    color: RgbColor,
) {
    contents.extend_from_slice(b"\x1b]");
    contents.extend_from_slice(code);
    contents.push(b';');
    if let Some(index) = index {
        crate::term::extend_itoa(contents, index);
        contents.push(b';');
    }
    contents.extend_from_slice(b"rgb:");
    write_hex_byte(contents, color.r);
    contents.push(b'/');
    write_hex_byte(contents, color.g);
    contents.push(b'/');
    write_hex_byte(contents, color.b);
    contents.extend_from_slice(b"\x1b\\");
}

fn write_hex_byte(contents: &mut Vec<u8>, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    contents.push(HEX[usize::from(byte >> 4)]);
    contents.push(HEX[usize::from(byte & 0x0f)]);
}

pub(crate) fn parse_color(bytes: &[u8]) -> Option<RgbColor> {
    if let Some(hex) = bytes.strip_prefix(b"#") {
        if hex.len() == 6 {
            return Some(RgbColor {
                r: parse_hex_byte(&hex[0..2])?,
                g: parse_hex_byte(&hex[2..4])?,
                b: parse_hex_byte(&hex[4..6])?,
            });
        }
        return None;
    }

    let rest = bytes.strip_prefix(b"rgb:")?;
    let mut parts = rest.split(|b| *b == b'/');
    let r = parse_x_color_component(parts.next()?)?;
    let g = parse_x_color_component(parts.next()?)?;
    let b = parse_x_color_component(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(RgbColor { r, g, b })
}

fn parse_hex_byte(bytes: &[u8]) -> Option<u8> {
    if bytes.len() != 2 {
        return None;
    }
    Some(hex_value(bytes[0])? << 4 | hex_value(bytes[1])?)
}

fn parse_x_color_component(bytes: &[u8]) -> Option<u8> {
    if bytes.is_empty() || bytes.len() > 4 {
        return None;
    }
    let mut value: u16 = 0;
    for byte in bytes {
        value = value
            .checked_mul(16)?
            .checked_add(u16::from(hex_value(*byte)?))?;
    }
    let max = (1_u32 << (bytes.len() * 4)) - 1;
    Some(((u32::from(value) * 255 + (max / 2)) / max) as u8)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
