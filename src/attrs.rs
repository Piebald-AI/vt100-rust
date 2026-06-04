use crate::term::BufWrite as _;

/// Represents a foreground or background color for cells.
#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
pub enum Color {
    /// The default terminal color.
    #[default]
    Default,

    /// An indexed terminal color.
    Idx(u8),

    /// An RGB terminal color. The parameters are (red, green, blue).
    Rgb(u8, u8, u8),
}

/// Represents the underline style for cells.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum UnderlineStyle {
    /// No underline.
    #[default]
    None,

    /// A single underline.
    Single,

    /// A double underline.
    Double,

    /// A curly underline.
    Curly,

    /// A dotted underline.
    Dotted,

    /// A dashed underline.
    Dashed,
}

const TEXT_MODE_INTENSITY: u16 = 0b0000_0000_0000_0011;
const TEXT_MODE_BOLD: u16 = 0b0000_0000_0000_0001;
const TEXT_MODE_DIM: u16 = 0b0000_0000_0000_0010;
const TEXT_MODE_ITALIC: u16 = 0b0000_0000_0000_0100;
const TEXT_MODE_INVERSE: u16 = 0b0000_0000_0000_1000;
const TEXT_MODE_BLINK: u16 = 0b0000_0000_0001_0000;
const TEXT_MODE_INVISIBLE: u16 = 0b0000_0000_0010_0000;
const TEXT_MODE_STRIKETHROUGH: u16 = 0b0000_0000_0100_0000;
const TEXT_MODE_OVERLINE: u16 = 0b0000_0000_1000_0000;
const TEXT_MODE_UNDERLINE_STYLE: u16 = 0b0000_0111_0000_0000;
const TEXT_MODE_UNDERLINE_STYLE_SHIFT: u16 = 8;

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Attrs {
    pub fgcolor: Color,
    pub bgcolor: Color,
    pub underline_color: Color,
    pub mode: u16,
}

impl Attrs {
    pub fn bold(&self) -> bool {
        self.mode & TEXT_MODE_BOLD != 0
    }

    pub fn dim(&self) -> bool {
        self.mode & TEXT_MODE_DIM != 0
    }

    fn intensity(&self) -> u16 {
        self.mode & TEXT_MODE_INTENSITY
    }

    pub fn set_bold(&mut self) {
        self.mode &= !TEXT_MODE_INTENSITY;
        self.mode |= TEXT_MODE_BOLD;
    }

    pub fn set_dim(&mut self) {
        self.mode &= !TEXT_MODE_INTENSITY;
        self.mode |= TEXT_MODE_DIM;
    }

    pub fn set_normal_intensity(&mut self) {
        self.mode &= !TEXT_MODE_INTENSITY;
    }

    pub fn italic(&self) -> bool {
        self.mode & TEXT_MODE_ITALIC != 0
    }

    pub fn set_italic(&mut self, italic: bool) {
        if italic {
            self.mode |= TEXT_MODE_ITALIC;
        } else {
            self.mode &= !TEXT_MODE_ITALIC;
        }
    }

    pub fn underline(&self) -> bool {
        self.underline_style() != UnderlineStyle::None
    }

    pub fn underline_style(&self) -> UnderlineStyle {
        match (self.mode & TEXT_MODE_UNDERLINE_STYLE)
            >> TEXT_MODE_UNDERLINE_STYLE_SHIFT
        {
            0 => UnderlineStyle::None,
            1 => UnderlineStyle::Single,
            2 => UnderlineStyle::Double,
            3 => UnderlineStyle::Curly,
            4 => UnderlineStyle::Dotted,
            5 => UnderlineStyle::Dashed,
            _ => unreachable!(),
        }
    }

    pub fn set_underline_style(&mut self, underline_style: UnderlineStyle) {
        self.mode &= !TEXT_MODE_UNDERLINE_STYLE;
        self.mode |= match underline_style {
            UnderlineStyle::None => 0,
            UnderlineStyle::Single => 1,
            UnderlineStyle::Double => 2,
            UnderlineStyle::Curly => 3,
            UnderlineStyle::Dotted => 4,
            UnderlineStyle::Dashed => 5,
        } << TEXT_MODE_UNDERLINE_STYLE_SHIFT;
        if underline_style == UnderlineStyle::None {
            self.underline_color = Color::Default;
        }
    }

    pub fn underline_color(&self) -> Color {
        self.underline_color
    }

    pub fn inverse(&self) -> bool {
        self.mode & TEXT_MODE_INVERSE != 0
    }

    pub fn set_inverse(&mut self, inverse: bool) {
        if inverse {
            self.mode |= TEXT_MODE_INVERSE;
        } else {
            self.mode &= !TEXT_MODE_INVERSE;
        }
    }

    pub fn blink(&self) -> bool {
        self.mode & TEXT_MODE_BLINK != 0
    }

    pub fn set_blink(&mut self, blink: bool) {
        if blink {
            self.mode |= TEXT_MODE_BLINK;
        } else {
            self.mode &= !TEXT_MODE_BLINK;
        }
    }

    pub fn invisible(&self) -> bool {
        self.mode & TEXT_MODE_INVISIBLE != 0
    }

    pub fn set_invisible(&mut self, invisible: bool) {
        if invisible {
            self.mode |= TEXT_MODE_INVISIBLE;
        } else {
            self.mode &= !TEXT_MODE_INVISIBLE;
        }
    }

    pub fn strikethrough(&self) -> bool {
        self.mode & TEXT_MODE_STRIKETHROUGH != 0
    }

    pub fn set_strikethrough(&mut self, strikethrough: bool) {
        if strikethrough {
            self.mode |= TEXT_MODE_STRIKETHROUGH;
        } else {
            self.mode &= !TEXT_MODE_STRIKETHROUGH;
        }
    }

    pub fn overline(&self) -> bool {
        self.mode & TEXT_MODE_OVERLINE != 0
    }

    pub fn set_overline(&mut self, overline: bool) {
        if overline {
            self.mode |= TEXT_MODE_OVERLINE;
        } else {
            self.mode &= !TEXT_MODE_OVERLINE;
        }
    }

    pub fn write_escape_code_diff(
        &self,
        contents: &mut Vec<u8>,
        other: &Self,
    ) {
        self.write_escape_code_diff_inner(contents, other, None);
    }

    /// Writes the SGR difference between two attribute sets using a
    /// serialization context.
    pub fn write_escape_code_diff_with_context(
        &self,
        contents: &mut Vec<u8>,
        other: &Self,
        context: &crate::SerializeContext<'_>,
    ) {
        self.write_escape_code_diff_inner(contents, other, Some(context));
    }

    fn write_escape_code_diff_inner(
        &self,
        contents: &mut Vec<u8>,
        other: &Self,
        context: Option<&crate::SerializeContext<'_>>,
    ) {
        if self != other && self == &Self::default() && context.is_none() {
            crate::term::ClearAttrs.write_buf(contents);
            return;
        }

        let attrs = crate::term::Attrs::default();

        let self_fg = resolve_color(
            self.fgcolor,
            crate::ColorRole::Foreground,
            context,
        );
        let other_fg = resolve_previous_color(
            other.fgcolor,
            crate::ColorRole::Foreground,
            context,
        );
        let attrs = if self_fg == other_fg {
            attrs
        } else {
            attrs.fgcolor(self_fg)
        };
        let self_bg = resolve_color(
            self.bgcolor,
            crate::ColorRole::Background,
            context,
        );
        let other_bg = resolve_previous_color(
            other.bgcolor,
            crate::ColorRole::Background,
            context,
        );
        let attrs = if self_bg == other_bg {
            attrs
        } else {
            attrs.bgcolor(self_bg)
        };
        let attrs = if self.intensity() == other.intensity() {
            attrs
        } else {
            attrs.intensity(match self.intensity() {
                0 => crate::term::Intensity::Normal,
                TEXT_MODE_BOLD => crate::term::Intensity::Bold,
                TEXT_MODE_DIM => crate::term::Intensity::Dim,
                _ => unreachable!(),
            })
        };
        let attrs = if self.italic() == other.italic() {
            attrs
        } else {
            attrs.italic(self.italic())
        };
        let attrs = if self.underline_style() == other.underline_style() {
            attrs
        } else {
            attrs.underline_style(self.underline_style())
        };
        let self_underline = resolve_color(
            self.underline_color,
            crate::ColorRole::Underline,
            context,
        );
        let other_underline = resolve_previous_color(
            other.underline_color,
            crate::ColorRole::Underline,
            context,
        );
        let attrs = if self_underline == other_underline {
            attrs
        } else {
            attrs.underline_color(self_underline)
        };
        let attrs = if self.inverse() == other.inverse() {
            attrs
        } else {
            attrs.inverse(self.inverse())
        };
        let attrs = if self.blink() == other.blink() {
            attrs
        } else {
            attrs.blink(self.blink())
        };
        let attrs = if self.invisible() == other.invisible() {
            attrs
        } else {
            attrs.invisible(self.invisible())
        };
        let attrs = if self.strikethrough() == other.strikethrough() {
            attrs
        } else {
            attrs.strikethrough(self.strikethrough())
        };
        let attrs = if self.overline() == other.overline() {
            attrs
        } else {
            attrs.overline(self.overline())
        };

        attrs.write_buf(contents);
    }
}

fn resolve_previous_color(
    color: Color,
    role: crate::ColorRole,
    context: Option<&crate::SerializeContext<'_>>,
) -> Color {
    if context.is_some() && color == Color::Default {
        return Color::Default;
    }
    resolve_color(color, role, context)
}

fn resolve_color(
    color: Color,
    role: crate::ColorRole,
    context: Option<&crate::SerializeContext<'_>>,
) -> Color {
    let Some(context) = context else {
        return color;
    };
    if context.color_mode == crate::SerializeColorMode::PreserveSemantic {
        return color;
    }
    match context.palette.resolve_color(color, role) {
        crate::ResolvedColor::Default => Color::Default,
        crate::ResolvedColor::Indexed(index) => Color::Idx(index),
        crate::ResolvedColor::Rgb(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
    }
}
