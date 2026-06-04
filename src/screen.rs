use crate::term::BufWrite as _;
use unicode_width::UnicodeWidthChar as _;

const MODE_APPLICATION_KEYPAD: u16 = 0b0000_0000_0000_0001;
const MODE_APPLICATION_CURSOR: u16 = 0b0000_0000_0000_0010;
const MODE_HIDE_CURSOR: u16 = 0b0000_0000_0000_0100;
const MODE_ALTERNATE_SCREEN: u16 = 0b0000_0000_0000_1000;
const MODE_BRACKETED_PASTE: u16 = 0b0000_0000_0001_0000;
const MODE_INSERT: u16 = 0b0000_0000_0010_0000;
const MODE_WRAPAROUND: u16 = 0b0000_0000_0100_0000;
const MODE_REVERSE_WRAPAROUND: u16 = 0b0000_0000_1000_0000;
const MODE_SEND_FOCUS: u16 = 0b0000_0001_0000_0000;

/// Controls how colors are emitted by replay serializers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerializeColorMode {
    /// Preserve terminal semantic colors such as indexed palette references.
    PreserveSemantic,
    /// Resolve dynamic palette overrides to truecolor where possible.
    ResolvePaletteToRgb,
}

/// Options for replay serialization.
#[derive(Clone, Debug)]
pub struct SerializeOptions {
    /// Include main-grid scrollback history before visible screen rows.
    pub include_scrollback: bool,
    /// Limit the number of scrollback rows emitted.
    pub scrollback_limit: Option<usize>,
    /// Include the alternate screen when it is active.
    pub include_alt_screen: bool,
    /// Include terminal input modes and scroll region state.
    pub include_modes: bool,
    /// Include cursor position and visibility.
    pub include_cursor: bool,
    /// Include title state.
    pub include_title: bool,
    /// Emit OSC palette setup for dynamic palette overrides.
    pub include_palette_setup: bool,
    /// Controls color serialization semantics.
    pub color_mode: SerializeColorMode,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            include_scrollback: true,
            scrollback_limit: None,
            include_alt_screen: true,
            include_modes: true,
            include_cursor: true,
            include_title: false,
            include_palette_setup: true,
            color_mode: SerializeColorMode::PreserveSemantic,
        }
    }
}

/// Serialization context used by lower-level formatting helpers.
pub struct SerializeContext<'a> {
    /// Dynamic palette state.
    pub palette: &'a crate::palette::Palette,
    /// Color serialization mode.
    pub color_mode: SerializeColorMode,
}

/// The xterm mouse handling mode currently in use.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum MouseProtocolMode {
    /// Mouse handling is disabled.
    #[default]
    None,

    /// Mouse button events should be reported on button press. Also known as
    /// X10 mouse mode.
    Press,

    /// Mouse button events should be reported on button press and release.
    /// Also known as VT200 mouse mode.
    PressRelease,

    // Highlight,
    /// Mouse button events should be reported on button press and release, as
    /// well as when the mouse moves between cells while a button is held
    /// down.
    ButtonMotion,

    /// Mouse button events should be reported on button press and release,
    /// and mouse motion events should be reported when the mouse moves
    /// between cells regardless of whether a button is held down or not.
    AnyMotion,
    // DecLocator,
}

/// The encoding to use for the enabled [`MouseProtocolMode`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum MouseProtocolEncoding {
    /// Default single-printable-byte encoding.
    #[default]
    Default,

    /// UTF-8-based encoding.
    Utf8,

    /// SGR-like encoding.
    Sgr,
    // Urxvt,
}

/// Represents the overall terminal state.
#[derive(Clone, Debug)]
pub struct Screen {
    grid: crate::grid::Grid,
    alternate_grid: crate::grid::Grid,

    attrs: crate::attrs::Attrs,
    saved_attrs: crate::attrs::Attrs,

    modes: u16,
    mouse_protocol_mode: MouseProtocolMode,
    mouse_protocol_encoding: MouseProtocolEncoding,

    window_title: Option<Vec<u8>>,
    window_icon_name: Option<Vec<u8>>,

    palette: crate::palette::Palette,

    hyperlinks: Vec<crate::Hyperlink>,
    active_hyperlink: Option<crate::HyperlinkId>,
}

impl Screen {
    pub(crate) fn new(
        size: crate::grid::Size,
        scrollback_len: usize,
    ) -> Self {
        let mut grid = crate::grid::Grid::new(size, scrollback_len);
        grid.allocate_rows();
        Self {
            grid,
            alternate_grid: crate::grid::Grid::new(size, 0),

            attrs: crate::attrs::Attrs::default(),
            saved_attrs: crate::attrs::Attrs::default(),

            modes: MODE_WRAPAROUND,
            mouse_protocol_mode: MouseProtocolMode::default(),
            mouse_protocol_encoding: MouseProtocolEncoding::default(),

            window_title: None,
            window_icon_name: None,

            palette: crate::palette::Palette::default(),

            hyperlinks: Vec::new(),
            active_hyperlink: None,
        }
    }

    pub(crate) fn set_window_title(&mut self, title: &[u8]) {
        self.window_title = Some(title.to_vec());
    }

    pub(crate) fn set_window_icon_name(&mut self, icon_name: &[u8]) {
        self.window_icon_name = Some(icon_name.to_vec());
    }

    /// Returns the current window title.
    #[must_use]
    pub fn window_title(&self) -> Option<&[u8]> {
        self.window_title.as_deref()
    }

    /// Returns the current window icon name.
    #[must_use]
    pub fn window_icon_name(&self) -> Option<&[u8]> {
        self.window_icon_name.as_deref()
    }

    pub(crate) fn set_hyperlink(&mut self, params: &[u8], uri: &[u8]) {
        if uri.is_empty() {
            if params.is_empty() {
                self.active_hyperlink = None;
            }
            return;
        }

        let id = self
            .hyperlinks
            .iter()
            .position(|link| link.params() == params && link.uri() == uri)
            .map_or_else(
                || {
                    let id = crate::HyperlinkId(
                        self.hyperlinks.len().try_into().unwrap_or(u32::MAX),
                    );
                    self.hyperlinks.push(crate::Hyperlink::new(
                        params.to_vec(),
                        uri.to_vec(),
                    ));
                    id
                },
                |idx| crate::HyperlinkId(idx.try_into().unwrap_or(u32::MAX)),
            );
        self.active_hyperlink = Some(id);
    }

    /// Returns hyperlink metadata for a hyperlink identifier.
    #[must_use]
    pub fn hyperlink(
        &self,
        id: crate::HyperlinkId,
    ) -> Option<&crate::Hyperlink> {
        self.hyperlinks.get(usize::try_from(id.0).ok()?)
    }

    /// Returns hyperlink metadata for the cell at the given location, if any.
    #[must_use]
    pub fn cell_hyperlink(
        &self,
        row: u16,
        col: u16,
    ) -> Option<&crate::Hyperlink> {
        self.cell(row, col)
            .and_then(|cell| cell.hyperlink_id())
            .and_then(|id| self.hyperlink(id))
    }

    /// Returns metadata for the currently active OSC 8 hyperlink, if any.
    #[must_use]
    pub fn active_hyperlink(&self) -> Option<&crate::Hyperlink> {
        self.active_hyperlink.and_then(|id| self.hyperlink(id))
    }

    /// Resizes the terminal.
    pub fn set_size(&mut self, rows: u16, cols: u16) {
        self.grid.set_size(crate::grid::Size { rows, cols });
        self.alternate_grid
            .set_size(crate::grid::Size { rows, cols });
    }

    /// Returns the current size of the terminal.
    ///
    /// The return value will be (rows, cols).
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        let size = self.grid().size();
        (size.rows, size.cols)
    }

    /// Sets the maximum number of rows retained in scrollback history.
    pub fn set_scrollback_len(&mut self, len: usize) {
        self.grid.set_scrollback_len(len);
    }

    /// Returns the configured scrollback history limit.
    #[must_use]
    pub fn scrollback_len(&self) -> usize {
        self.grid.scrollback_len()
    }

    /// Returns the number of rows currently retained in scrollback history.
    #[must_use]
    pub fn scrollback_rows_len(&self) -> usize {
        self.grid.scrollback_rows_len()
    }

    /// Returns the tracked dynamic palette state.
    #[must_use]
    pub fn palette(&self) -> &crate::palette::Palette {
        &self.palette
    }

    pub(crate) fn palette_mut(&mut self) -> &mut crate::palette::Palette {
        &mut self.palette
    }

    /// Scrolls to the given position in the scrollback.
    ///
    /// This position indicates the offset from the top of the screen, and
    /// should be `0` to put the normal screen in view.
    ///
    /// This affects the return values of methods called on the screen: for
    /// instance, `screen.cell(0, 0)` will return the top left corner of the
    /// screen after taking the scrollback offset into account.
    ///
    /// The value given will be clamped to the actual size of the scrollback.
    pub fn set_scrollback(&mut self, rows: usize) {
        self.grid_mut().set_scrollback(rows);
    }

    /// Returns the current position in the scrollback.
    ///
    /// This position indicates the offset from the top of the screen, and is
    /// `0` when the normal screen is in view.
    #[must_use]
    pub fn scrollback(&self) -> usize {
        self.grid().scrollback()
    }

    /// Returns the text contents of the terminal.
    ///
    /// This will not include any formatting information, and will be in plain
    /// text format.
    #[must_use]
    pub fn contents(&self) -> String {
        let mut contents = String::new();
        self.write_contents(&mut contents);
        contents
    }

    fn write_contents(&self, contents: &mut String) {
        self.grid().write_contents(contents);
    }

    /// Returns the text contents of the terminal by row, restricted to the
    /// given subset of columns.
    ///
    /// This will not include any formatting information, and will be in plain
    /// text format.
    ///
    /// Newlines will not be included.
    pub fn rows(
        &self,
        start: u16,
        width: u16,
    ) -> impl Iterator<Item = String> + '_ {
        self.grid().visible_rows().map(move |row| {
            let mut contents = String::new();
            row.write_contents(&mut contents, start, width, false);
            contents
        })
    }

    /// Returns the text contents of the terminal logically between two cells.
    /// This will include the remainder of the starting row after `start_col`,
    /// followed by the entire contents of the rows between `start_row` and
    /// `end_row`, followed by the beginning of the `end_row` up until
    /// `end_col`. This is useful for things like determining the contents of
    /// a clipboard selection.
    #[must_use]
    pub fn contents_between(
        &self,
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
    ) -> String {
        match start_row.cmp(&end_row) {
            std::cmp::Ordering::Less => {
                let (_, cols) = self.size();
                let mut contents = String::new();
                for (i, row) in self
                    .grid()
                    .visible_rows()
                    .enumerate()
                    .skip(usize::from(start_row))
                    .take(usize::from(end_row) - usize::from(start_row) + 1)
                {
                    if i == usize::from(start_row) {
                        row.write_contents(
                            &mut contents,
                            start_col,
                            cols - start_col,
                            false,
                        );
                        if !row.wrapped() {
                            contents.push('\n');
                        }
                    } else if i == usize::from(end_row) {
                        row.write_contents(&mut contents, 0, end_col, false);
                    } else {
                        row.write_contents(&mut contents, 0, cols, false);
                        if !row.wrapped() {
                            contents.push('\n');
                        }
                    }
                }
                contents
            }
            std::cmp::Ordering::Equal => {
                if start_col < end_col {
                    self.rows(start_col, end_col - start_col)
                        .nth(usize::from(start_row))
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
            std::cmp::Ordering::Greater => String::new(),
        }
    }

    /// Return escape codes sufficient to reproduce the entire contents of the
    /// current terminal state. This is a convenience wrapper around
    /// [`contents_formatted`](Self::contents_formatted) and
    /// [`input_mode_formatted`](Self::input_mode_formatted).
    #[must_use]
    pub fn state_formatted(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_contents_formatted(&mut contents);
        self.write_active_hyperlink_formatted(&mut contents);
        self.write_input_mode_formatted(&mut contents);
        contents
    }

    /// Return escape codes sufficient to turn the terminal state of the
    /// screen `prev` into the current terminal state. This is a convenience
    /// wrapper around [`contents_diff`](Self::contents_diff) and
    /// [`input_mode_diff`](Self::input_mode_diff).
    #[must_use]
    pub fn state_diff(&self, prev: &Self) -> Vec<u8> {
        let mut contents = vec![];
        if prev.active_hyperlink.is_some() {
            crate::row::write_hyperlink_end(&mut contents);
        }
        self.write_contents_diff(&mut contents, prev);
        self.write_active_hyperlink_formatted(&mut contents);
        self.write_input_mode_diff(&mut contents, prev);
        contents
    }

    /// Returns the formatted visible contents of the terminal.
    ///
    /// Formatting information will be included inline as terminal escape
    /// codes. The result will be suitable for feeding directly to a raw
    /// terminal parser, and will result in the same visual output.
    #[must_use]
    pub fn contents_formatted(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_contents_formatted(&mut contents);
        contents
    }

    fn write_contents_formatted(&self, contents: &mut Vec<u8>) {
        crate::term::HideCursor::new(self.hide_cursor()).write_buf(contents);
        let prev_attrs = self
            .grid()
            .write_contents_formatted(contents, &self.hyperlinks);
        self.attrs.write_escape_code_diff(contents, &prev_attrs);
    }

    /// Returns the formatted visible contents of the terminal by row,
    /// restricted to the given subset of columns.
    ///
    /// Formatting information will be included inline as terminal escape
    /// codes. The result will be suitable for feeding directly to a raw
    /// terminal parser, and will result in the same visual output.
    ///
    /// You are responsible for positioning the cursor before printing each
    /// row, and the final cursor position after displaying each row is
    /// unspecified.
    // the unwraps in this method shouldn't be reachable
    #[allow(clippy::missing_panics_doc)]
    pub fn rows_formatted(
        &self,
        start: u16,
        width: u16,
    ) -> impl Iterator<Item = Vec<u8>> + '_ {
        let mut wrapping = false;
        self.grid().visible_rows().enumerate().map(move |(i, row)| {
            // number of rows in a grid is stored in a u16 (see Size), so
            // visible_rows can never return enough rows to overflow here
            let i = i.try_into().unwrap();
            let mut contents = vec![];
            let (_, _, mut prev_hyperlink_id) = row.write_contents_formatted(
                &mut contents,
                start,
                width,
                i,
                wrapping,
                None,
                None,
                None,
                &self.hyperlinks,
            );
            crate::row::close_hyperlink(
                &mut contents,
                &mut prev_hyperlink_id,
            );
            if start == 0 && width == self.grid.size().cols {
                wrapping = row.wrapped();
            }
            contents
        })
    }

    /// Returns the plain text contents of the full terminal buffer,
    /// including all scrollback history and the current screen.
    ///
    /// This is like [`contents`](Self::contents) but includes scrollback
    /// lines that have scrolled off the top of the visible viewport.
    ///
    /// Always returns the main grid's contents, even when the alternate
    /// screen is active, since the alternate screen has no meaningful
    /// scrollback history.
    #[must_use]
    pub fn contents_full(&self) -> String {
        let mut contents = String::new();
        self.grid.write_contents_full(&mut contents);
        contents
    }

    /// Returns the formatted contents of the full terminal buffer,
    /// including all scrollback history and the current screen.
    ///
    /// Formatting information (colors, bold, underline, etc.) will be
    /// included inline as SGR escape codes. Lines are separated by `\r\n`.
    /// No cursor positioning escape codes are emitted.
    ///
    /// This is like [`contents_formatted`](Self::contents_formatted) but
    /// includes scrollback lines that have scrolled off the top of the
    /// visible viewport, and uses line-based output instead of absolute
    /// cursor positioning.
    ///
    /// Always returns the main grid's contents, even when the alternate
    /// screen is active, since the alternate screen has no meaningful
    /// scrollback history.
    #[must_use]
    pub fn contents_formatted_full(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.grid
            .write_contents_formatted_full(&mut contents, &self.hyperlinks);
        contents
    }

    /// Serializes terminal contents according to explicit replay options.
    #[must_use]
    pub fn serialize_replay(&self, options: SerializeOptions) -> Vec<u8> {
        let mut contents = vec![];
        self.write_serialize_replay(&mut contents, &options);
        contents
    }

    /// Serializes the main terminal buffer for persistent replay.
    #[must_use]
    pub fn serialize_persistent_replay(
        &self,
        scrollback_limit: Option<usize>,
    ) -> Vec<u8> {
        self.serialize_replay(SerializeOptions {
            scrollback_limit,
            include_alt_screen: false,
            ..SerializeOptions::default()
        })
    }

    /// Serializes a visually stable snapshot with dynamic palette colors
    /// resolved to truecolor where possible.
    #[must_use]
    pub fn serialize_frozen_snapshot(&self) -> Vec<u8> {
        self.serialize_replay(SerializeOptions {
            include_scrollback: true,
            scrollback_limit: None,
            include_alt_screen: true,
            include_modes: false,
            include_cursor: false,
            include_title: false,
            include_palette_setup: false,
            color_mode: SerializeColorMode::ResolvePaletteToRgb,
        })
    }

    fn write_serialize_replay(
        &self,
        contents: &mut Vec<u8>,
        options: &SerializeOptions,
    ) {
        if options.include_cursor {
            crate::term::HideCursor::new(self.hide_cursor())
                .write_buf(contents);
        }
        if options.include_title {
            if let Some(title) = self.window_title() {
                contents.extend_from_slice(b"\x1b]2;");
                contents.extend_from_slice(title);
                contents.extend_from_slice(b"\x1b\\");
            }
            if let Some(icon_name) = self.window_icon_name() {
                contents.extend_from_slice(b"\x1b]1;");
                contents.extend_from_slice(icon_name);
                contents.extend_from_slice(b"\x1b\\");
            }
        }
        if options.include_palette_setup
            && options.color_mode == SerializeColorMode::PreserveSemantic
        {
            self.palette.write_osc_setup(contents);
        }

        let context = SerializeContext {
            palette: &self.palette,
            color_mode: options.color_mode,
        };

        if self.alternate_screen() && options.include_alt_screen {
            contents.extend_from_slice(b"\x1b[?1049h");
            let prev_attrs = self.alternate_grid.write_replay_contents(
                contents,
                false,
                None,
                &self.hyperlinks,
                &context,
            );
            self.attrs.write_escape_code_diff_with_context(
                contents,
                &prev_attrs,
                &context,
            );
            if options.include_modes {
                self.write_scroll_region_formatted(contents);
                if self.origin_mode() {
                    contents.extend_from_slice(b"\x1b[?6h");
                }
            }
            if options.include_cursor {
                self.alternate_grid
                    .write_cursor_position_formatted_with_row_offset(
                        contents,
                        0,
                        self.origin_mode(),
                        None,
                        Some(self.attrs),
                    );
            }
        } else {
            let prev_attrs = self.grid.write_replay_contents(
                contents,
                options.include_scrollback,
                options.scrollback_limit,
                &self.hyperlinks,
                &context,
            );
            self.attrs.write_escape_code_diff_with_context(
                contents,
                &prev_attrs,
                &context,
            );
            if options.include_modes {
                self.write_scroll_region_formatted(contents);
                if self.origin_mode() {
                    contents.extend_from_slice(b"\x1b[?6h");
                }
            }
            if options.include_cursor {
                self.grid.write_cursor_position_formatted_with_row_offset(
                    contents,
                    0,
                    self.origin_mode(),
                    None,
                    Some(self.attrs),
                );
            }
        }

        if options.include_cursor || options.include_modes {
            self.write_active_hyperlink_formatted(contents);
            self.write_input_mode_formatted(contents);
        }
    }

    /// Returns escape codes suitable for replaying the full terminal buffer
    /// and restoring the current terminal state.
    ///
    /// This includes active terminal contents, restores active drawing
    /// attributes, cursor visibility, cursor position, and input modes. No
    /// terminal query sequences are emitted.
    ///
    /// When the alternate screen is inactive, this serializes main-grid
    /// scrollback followed by the current main-grid screen contents. If the
    /// cursor row plus scrollback length cannot fit in `u16`, cursor position
    /// restoration is omitted.
    ///
    /// When the alternate screen is active, this enters the alternate screen
    /// and serializes only its visible viewport, since the alternate screen
    /// has no meaningful scrollback history.
    #[must_use]
    pub fn state_formatted_full(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_state_formatted_full(&mut contents);
        contents
    }

    /// Writes escape codes suitable for replaying the full terminal buffer
    /// and restoring the current terminal state into `contents`.
    pub fn write_state_formatted_full(&self, contents: &mut Vec<u8>) {
        crate::term::HideCursor::new(self.hide_cursor()).write_buf(contents);

        if self.alternate_screen() {
            contents.extend_from_slice(b"\x1b[?1049h");
            let prev_attrs = self
                .alternate_grid
                .write_contents_formatted(contents, &self.hyperlinks);
            self.attrs.write_escape_code_diff(contents, &prev_attrs);
            self.write_scroll_region_formatted(contents);
            if self.origin_mode() {
                contents.extend_from_slice(b"\x1b[?6h");
            }
            self.alternate_grid
                .write_cursor_position_formatted_with_row_offset(
                    contents,
                    0,
                    self.origin_mode(),
                    None,
                    Some(self.attrs),
                );
            self.attrs.write_escape_code_diff(contents, &prev_attrs);
        } else {
            let prev_attrs = self
                .grid
                .write_contents_formatted_full(contents, &self.hyperlinks);
            self.attrs.write_escape_code_diff(contents, &prev_attrs);

            self.write_scroll_region_formatted(contents);
            if self.origin_mode() {
                contents.extend_from_slice(b"\x1b[?6h");
            }

            if let Ok(row_offset) = self.grid.scrollback_rows_len().try_into()
            {
                self.grid.write_cursor_position_formatted_with_row_offset(
                    contents,
                    row_offset,
                    self.origin_mode(),
                    None,
                    Some(self.attrs),
                );
                self.attrs.write_escape_code_diff(contents, &prev_attrs);
            }
        }

        self.write_active_hyperlink_formatted(contents);
        self.write_input_mode_formatted(contents);
    }

    fn write_active_hyperlink_formatted(&self, contents: &mut Vec<u8>) {
        if let Some(link) = self.active_hyperlink() {
            crate::row::write_hyperlink_start(contents, link);
        }
    }

    /// Returns the plain text contents of the full terminal buffer by row.
    /// Includes scrollback lines followed by current screen lines.
    ///
    /// This is like [`rows`](Self::rows) but includes scrollback lines
    /// that have scrolled off the top of the visible viewport.
    ///
    /// Always returns the main grid's rows, even when the alternate
    /// screen is active.
    ///
    /// Newlines will not be included.
    pub fn rows_full(
        &self,
        start: u16,
        width: u16,
    ) -> impl Iterator<Item = String> + '_ {
        self.grid.all_rows().map(move |row| {
            let mut contents = String::new();
            row.write_contents(&mut contents, start, width, false);
            contents
        })
    }

    /// Returns the formatted contents of the full terminal buffer by row.
    /// Includes scrollback lines followed by current screen lines.
    ///
    /// Each row contains inline SGR formatting codes but no cursor
    /// positioning.
    ///
    /// This is like [`rows_formatted`](Self::rows_formatted) but includes
    /// scrollback lines that have scrolled off the top of the visible
    /// viewport.
    ///
    /// Always returns the main grid's rows, even when the alternate
    /// screen is active.
    pub fn rows_formatted_full(
        &self,
        start: u16,
        width: u16,
    ) -> impl Iterator<Item = Vec<u8>> + '_ {
        let mut prev_attrs = crate::attrs::Attrs::default();
        let mut prev_hyperlink_id = None;
        self.grid.all_rows().map(move |row| {
            let mut contents = vec![];
            (prev_attrs, prev_hyperlink_id) = row
                .write_contents_formatted_inline(
                    &mut contents,
                    start,
                    width,
                    prev_attrs,
                    prev_hyperlink_id,
                    &self.hyperlinks,
                );
            contents
        })
    }

    /// Returns a terminal byte stream sufficient to turn the visible contents
    /// of the screen described by `prev` into the visible contents of the
    /// screen described by `self`.
    ///
    /// The result of rendering `prev.contents_formatted()` followed by
    /// `self.contents_diff(prev)` should be equivalent to the result of
    /// rendering `self.contents_formatted()`. This is primarily useful when
    /// you already have a terminal parser whose state is described by `prev`,
    /// since the diff will likely require less memory and cause less
    /// flickering than redrawing the entire screen contents.
    #[must_use]
    pub fn contents_diff(&self, prev: &Self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_contents_diff(&mut contents, prev);
        contents
    }

    fn write_contents_diff(&self, contents: &mut Vec<u8>, prev: &Self) {
        if self.hide_cursor() != prev.hide_cursor() {
            crate::term::HideCursor::new(self.hide_cursor())
                .write_buf(contents);
        }
        let prev_attrs = self.grid().write_contents_diff(
            contents,
            prev.grid(),
            prev.attrs,
            &self.hyperlinks,
            &prev.hyperlinks,
        );
        self.attrs.write_escape_code_diff(contents, &prev_attrs);
    }

    /// Returns a sequence of terminal byte streams sufficient to turn the
    /// visible contents of the subset of each row from `prev` (as described
    /// by `start` and `width`) into the visible contents of the corresponding
    /// row subset in `self`.
    ///
    /// You are responsible for positioning the cursor before printing each
    /// row, and the final cursor position after displaying each row is
    /// unspecified.
    // the unwraps in this method shouldn't be reachable
    #[allow(clippy::missing_panics_doc)]
    pub fn rows_diff<'a>(
        &'a self,
        prev: &'a Self,
        start: u16,
        width: u16,
    ) -> impl Iterator<Item = Vec<u8>> + 'a {
        self.grid()
            .visible_rows()
            .zip(prev.grid().visible_rows())
            .enumerate()
            .map(move |(i, (row, prev_row))| {
                // number of rows in a grid is stored in a u16 (see Size), so
                // visible_rows can never return enough rows to overflow here
                let i = i.try_into().unwrap();
                let mut contents = vec![];
                let (_, _, mut prev_hyperlink_id) = row.write_contents_diff(
                    &mut contents,
                    prev_row,
                    &self.hyperlinks,
                    &prev.hyperlinks,
                    start,
                    width,
                    i,
                    false,
                    false,
                    crate::grid::Pos { row: i, col: start },
                    crate::attrs::Attrs::default(),
                    None,
                );
                crate::row::close_hyperlink(
                    &mut contents,
                    &mut prev_hyperlink_id,
                );
                contents
            })
    }

    /// Returns terminal escape sequences sufficient to set the current
    /// terminal's input modes.
    ///
    /// Supported modes are:
    /// * application keypad
    /// * application cursor
    /// * bracketed paste
    /// * insert mode
    /// * origin mode
    /// * wraparound mode
    /// * reverse wraparound mode
    /// * send focus mode
    /// * xterm mouse support
    #[must_use]
    pub fn input_mode_formatted(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_input_mode_formatted(&mut contents);
        contents
    }

    fn write_input_mode_formatted(&self, contents: &mut Vec<u8>) {
        crate::term::ApplicationKeypad::new(
            self.mode(MODE_APPLICATION_KEYPAD),
        )
        .write_buf(contents);
        crate::term::ApplicationCursor::new(
            self.mode(MODE_APPLICATION_CURSOR),
        )
        .write_buf(contents);
        crate::term::BracketedPaste::new(self.mode(MODE_BRACKETED_PASTE))
            .write_buf(contents);
        self.write_extended_input_mode_formatted(contents);
        crate::term::MouseProtocolMode::new(
            self.mouse_protocol_mode,
            MouseProtocolMode::None,
        )
        .write_buf(contents);
        crate::term::MouseProtocolEncoding::new(
            self.mouse_protocol_encoding,
            MouseProtocolEncoding::Default,
        )
        .write_buf(contents);
    }

    fn write_extended_input_mode_formatted(&self, contents: &mut Vec<u8>) {
        if self.insert_mode() {
            contents.extend_from_slice(b"\x1b[4h");
        }
        if !self.wraparound_mode() {
            contents.extend_from_slice(b"\x1b[?7l");
        }
        if self.reverse_wraparound_mode() {
            contents.extend_from_slice(b"\x1b[?45h");
        }
        if self.send_focus_mode() {
            contents.extend_from_slice(b"\x1b[?1004h");
        }
    }

    fn write_scroll_region_formatted(&self, contents: &mut Vec<u8>) {
        let (top, bottom) = self.scroll_region();
        if top != 0 || bottom != self.grid().size().rows - 1 {
            contents.extend_from_slice(b"\x1b[");
            crate::term::extend_itoa(contents, top + 1);
            contents.push(b';');
            crate::term::extend_itoa(contents, bottom + 1);
            contents.push(b'r');
        }
    }

    /// Returns terminal escape sequences sufficient to change the previous
    /// terminal's input modes to the input modes enabled in the current
    /// terminal.
    #[must_use]
    pub fn input_mode_diff(&self, prev: &Self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_input_mode_diff(&mut contents, prev);
        contents
    }

    fn write_input_mode_diff(&self, contents: &mut Vec<u8>, prev: &Self) {
        if self.mode(MODE_APPLICATION_KEYPAD)
            != prev.mode(MODE_APPLICATION_KEYPAD)
        {
            crate::term::ApplicationKeypad::new(
                self.mode(MODE_APPLICATION_KEYPAD),
            )
            .write_buf(contents);
        }
        if self.mode(MODE_APPLICATION_CURSOR)
            != prev.mode(MODE_APPLICATION_CURSOR)
        {
            crate::term::ApplicationCursor::new(
                self.mode(MODE_APPLICATION_CURSOR),
            )
            .write_buf(contents);
        }
        if self.mode(MODE_BRACKETED_PASTE) != prev.mode(MODE_BRACKETED_PASTE)
        {
            crate::term::BracketedPaste::new(self.mode(MODE_BRACKETED_PASTE))
                .write_buf(contents);
        }
        if self.insert_mode() != prev.insert_mode() {
            contents.extend_from_slice(if self.insert_mode() {
                b"\x1b[4h"
            } else {
                b"\x1b[4l"
            });
        }
        if self.origin_mode() != prev.origin_mode() {
            contents.extend_from_slice(if self.origin_mode() {
                b"\x1b[?6h"
            } else {
                b"\x1b[?6l"
            });
        }
        if self.wraparound_mode() != prev.wraparound_mode() {
            contents.extend_from_slice(if self.wraparound_mode() {
                b"\x1b[?7h"
            } else {
                b"\x1b[?7l"
            });
        }
        if self.reverse_wraparound_mode() != prev.reverse_wraparound_mode() {
            contents.extend_from_slice(if self.reverse_wraparound_mode() {
                b"\x1b[?45h"
            } else {
                b"\x1b[?45l"
            });
        }
        if self.send_focus_mode() != prev.send_focus_mode() {
            contents.extend_from_slice(if self.send_focus_mode() {
                b"\x1b[?1004h"
            } else {
                b"\x1b[?1004l"
            });
        }
        crate::term::MouseProtocolMode::new(
            self.mouse_protocol_mode,
            prev.mouse_protocol_mode,
        )
        .write_buf(contents);
        crate::term::MouseProtocolEncoding::new(
            self.mouse_protocol_encoding,
            prev.mouse_protocol_encoding,
        )
        .write_buf(contents);
    }

    /// Returns terminal escape sequences sufficient to set the current
    /// terminal's drawing attributes.
    ///
    /// Supported drawing attributes are:
    /// * fgcolor
    /// * bgcolor
    /// * bold
    /// * dim
    /// * italic
    /// * underline
    /// * inverse
    ///
    /// This is not typically necessary, since
    /// [`contents_formatted`](Self::contents_formatted) will leave
    /// the current active drawing attributes in the correct state, but this
    /// can be useful in the case of drawing additional things on top of a
    /// terminal output, since you will need to restore the terminal state
    /// without the terminal contents necessarily being the same.
    #[must_use]
    pub fn attributes_formatted(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_attributes_formatted(&mut contents);
        contents
    }

    fn write_attributes_formatted(&self, contents: &mut Vec<u8>) {
        crate::term::ClearAttrs.write_buf(contents);
        self.attrs.write_escape_code_diff(
            contents,
            &crate::attrs::Attrs::default(),
        );
    }

    /// Returns the current cursor position of the terminal.
    ///
    /// The return value will be (row, col).
    #[must_use]
    pub fn cursor_position(&self) -> (u16, u16) {
        let pos = self.grid().pos();
        (pos.row, pos.col)
    }

    /// Returns terminal escape sequences sufficient to set the current
    /// cursor state of the terminal.
    ///
    /// This is not typically necessary, since
    /// [`contents_formatted`](Self::contents_formatted) will leave
    /// the cursor in the correct state, but this can be useful in the case of
    /// drawing additional things on top of a terminal output, since you will
    /// need to restore the terminal state without the terminal contents
    /// necessarily being the same.
    ///
    /// Note that the bytes returned by this function may alter the active
    /// drawing attributes, because it may require redrawing existing cells in
    /// order to position the cursor correctly (for instance, in the case
    /// where the cursor is past the end of a row). Therefore, you should
    /// ensure to reset the active drawing attributes if necessary after
    /// processing this data, for instance by using
    /// [`attributes_formatted`](Self::attributes_formatted).
    #[must_use]
    pub fn cursor_state_formatted(&self) -> Vec<u8> {
        let mut contents = vec![];
        self.write_cursor_state_formatted(&mut contents);
        contents
    }

    fn write_cursor_state_formatted(&self, contents: &mut Vec<u8>) {
        crate::term::HideCursor::new(self.hide_cursor()).write_buf(contents);
        self.grid()
            .write_cursor_position_formatted(contents, None, None);

        // we don't just call write_attributes_formatted here, because that
        // would still be confusing - consider the case where the user sets
        // their own unrelated drawing attributes (on a different parser
        // instance) and then calls cursor_state_formatted. just documenting
        // it and letting the user handle it on their own is more
        // straightforward.
    }

    /// Returns the [`Cell`](crate::Cell) object at the given location in the
    /// terminal, if it exists.
    #[must_use]
    pub fn cell(&self, row: u16, col: u16) -> Option<&crate::Cell> {
        self.grid().visible_cell(crate::grid::Pos { row, col })
    }

    /// Returns whether the text in row `row` should wrap to the next line.
    #[must_use]
    pub fn row_wrapped(&self, row: u16) -> bool {
        self.grid()
            .visible_row(row)
            .is_some_and(crate::row::Row::wrapped)
    }

    /// Returns whether the alternate screen is currently in use.
    #[must_use]
    pub fn alternate_screen(&self) -> bool {
        self.mode(MODE_ALTERNATE_SCREEN)
    }

    /// Returns whether the terminal should be in application keypad mode.
    #[must_use]
    pub fn application_keypad(&self) -> bool {
        self.mode(MODE_APPLICATION_KEYPAD)
    }

    /// Returns whether the terminal should be in application cursor mode.
    #[must_use]
    pub fn application_cursor(&self) -> bool {
        self.mode(MODE_APPLICATION_CURSOR)
    }

    /// Returns whether the terminal should be in hide cursor mode.
    #[must_use]
    pub fn hide_cursor(&self) -> bool {
        self.mode(MODE_HIDE_CURSOR)
    }

    /// Returns whether the terminal should be in bracketed paste mode.
    #[must_use]
    pub fn bracketed_paste(&self) -> bool {
        self.mode(MODE_BRACKETED_PASTE)
    }

    /// Returns whether insert mode is currently enabled.
    #[must_use]
    pub fn insert_mode(&self) -> bool {
        self.mode(MODE_INSERT)
    }

    /// Returns whether origin mode is currently enabled.
    #[must_use]
    pub fn origin_mode(&self) -> bool {
        self.grid().origin_mode()
    }

    /// Returns whether wraparound mode is currently enabled.
    #[must_use]
    pub fn wraparound_mode(&self) -> bool {
        self.mode(MODE_WRAPAROUND)
    }

    /// Returns whether reverse wraparound mode is currently enabled.
    #[must_use]
    pub fn reverse_wraparound_mode(&self) -> bool {
        self.mode(MODE_REVERSE_WRAPAROUND)
    }

    /// Returns whether send focus mode is currently enabled.
    #[must_use]
    pub fn send_focus_mode(&self) -> bool {
        self.mode(MODE_SEND_FOCUS)
    }

    /// Returns the active scroll region as zero-based inclusive row bounds.
    #[must_use]
    pub fn scroll_region(&self) -> (u16, u16) {
        self.grid().scroll_region()
    }

    /// Returns the currently active [`MouseProtocolMode`].
    #[must_use]
    pub fn mouse_protocol_mode(&self) -> MouseProtocolMode {
        self.mouse_protocol_mode
    }

    /// Returns the currently active [`MouseProtocolEncoding`].
    #[must_use]
    pub fn mouse_protocol_encoding(&self) -> MouseProtocolEncoding {
        self.mouse_protocol_encoding
    }

    /// Returns the currently active foreground color.
    #[must_use]
    pub fn fgcolor(&self) -> crate::Color {
        self.attrs.fgcolor
    }

    /// Returns the currently active background color.
    #[must_use]
    pub fn bgcolor(&self) -> crate::Color {
        self.attrs.bgcolor
    }

    /// Returns whether newly drawn text should be rendered with the bold text
    /// attribute.
    #[must_use]
    pub fn bold(&self) -> bool {
        self.attrs.bold()
    }

    /// Returns whether newly drawn text should be rendered with the dim text
    /// attribute.
    #[must_use]
    pub fn dim(&self) -> bool {
        self.attrs.dim()
    }

    /// Returns whether newly drawn text should be rendered with the italic
    /// text attribute.
    #[must_use]
    pub fn italic(&self) -> bool {
        self.attrs.italic()
    }

    /// Returns whether newly drawn text should be rendered with the
    /// underlined text attribute.
    #[must_use]
    pub fn underline(&self) -> bool {
        self.attrs.underline()
    }

    /// Returns the active underline style.
    #[must_use]
    pub fn underline_style(&self) -> crate::UnderlineStyle {
        self.attrs.underline_style()
    }

    /// Returns the active underline color.
    #[must_use]
    pub fn underline_color(&self) -> crate::Color {
        self.attrs.underline_color()
    }

    /// Returns whether newly drawn text should be rendered with the inverse
    /// text attribute.
    #[must_use]
    pub fn inverse(&self) -> bool {
        self.attrs.inverse()
    }

    /// Returns whether newly drawn text should be rendered with the blinking
    /// text attribute.
    #[must_use]
    pub fn blink(&self) -> bool {
        self.attrs.blink()
    }

    /// Returns whether newly drawn text should be rendered with the invisible
    /// text attribute.
    #[must_use]
    pub fn invisible(&self) -> bool {
        self.attrs.invisible()
    }

    /// Returns whether newly drawn text should be rendered with the
    /// strikethrough text attribute.
    #[must_use]
    pub fn strikethrough(&self) -> bool {
        self.attrs.strikethrough()
    }

    /// Returns whether newly drawn text should be rendered with the overline
    /// text attribute.
    #[must_use]
    pub fn overline(&self) -> bool {
        self.attrs.overline()
    }

    pub(crate) fn grid(&self) -> &crate::grid::Grid {
        if self.mode(MODE_ALTERNATE_SCREEN) {
            &self.alternate_grid
        } else {
            &self.grid
        }
    }

    fn grid_mut(&mut self) -> &mut crate::grid::Grid {
        if self.mode(MODE_ALTERNATE_SCREEN) {
            &mut self.alternate_grid
        } else {
            &mut self.grid
        }
    }

    fn enter_alternate_grid(&mut self) {
        self.grid_mut().set_scrollback(0);
        self.set_mode(MODE_ALTERNATE_SCREEN);
        self.alternate_grid.allocate_rows();
    }

    fn exit_alternate_grid(&mut self) {
        self.clear_mode(MODE_ALTERNATE_SCREEN);
    }

    fn save_cursor(&mut self) {
        self.grid_mut().save_cursor();
        self.saved_attrs = self.attrs;
    }

    fn restore_cursor(&mut self) {
        self.grid_mut().restore_cursor();
        self.attrs = self.saved_attrs;
    }

    fn set_mode(&mut self, mode: u16) {
        self.modes |= mode;
    }

    fn clear_mode(&mut self, mode: u16) {
        self.modes &= !mode;
    }

    fn mode(&self, mode: u16) -> bool {
        self.modes & mode != 0
    }

    fn set_mouse_mode(&mut self, mode: MouseProtocolMode) {
        self.mouse_protocol_mode = mode;
    }

    fn clear_mouse_mode(&mut self, mode: MouseProtocolMode) {
        if self.mouse_protocol_mode == mode {
            self.mouse_protocol_mode = MouseProtocolMode::default();
        }
    }

    fn set_mouse_encoding(&mut self, encoding: MouseProtocolEncoding) {
        self.mouse_protocol_encoding = encoding;
    }

    fn clear_mouse_encoding(&mut self, encoding: MouseProtocolEncoding) {
        if self.mouse_protocol_encoding == encoding {
            self.mouse_protocol_encoding = MouseProtocolEncoding::default();
        }
    }
}

impl Screen {
    pub(crate) fn text(&mut self, c: char) {
        let pos = self.grid().pos();
        let size = self.grid().size();
        let attrs = self.attrs;
        let hyperlink_id = self.active_hyperlink;

        let width = c.width();
        if width.is_none() && (u32::from(c)) < 256 {
            // don't even try to draw control characters
            return;
        }
        let width = width
            .unwrap_or(1)
            .try_into()
            // width() can only return 0, 1, or 2
            .unwrap();

        // it doesn't make any sense to wrap if the last column in a row
        // didn't already have contents. don't try to handle the case where a
        // character wraps because there was only one column left in the
        // previous row - literally everything handles this case differently,
        // and this is tmux behavior (and also the simplest). i'm open to
        // reconsidering this behavior, but only with a really good reason
        // (xterm handles this by introducing the concept of triple width
        // cells, which i really don't want to do).
        let mut wrap = false;
        if pos.col > size.cols - width {
            let last_cell = self
                .grid()
                .drawing_cell(crate::grid::Pos {
                    row: pos.row,
                    col: size.cols - 1,
                })
                // pos.row is valid, since it comes directly from
                // self.grid().pos() which we assume to always have a valid
                // row value. size.cols - 1 is also always a valid column.
                .unwrap();
            if last_cell.has_contents() || last_cell.is_wide_continuation() {
                wrap = true;
            }
        }
        if !self.wraparound_mode() && !wrap && pos.col > size.cols - width {
            self.grid_mut().col_set(size.cols - width);
        } else {
            if !self.wraparound_mode() {
                wrap = false;
                if pos.col > size.cols - width {
                    self.grid_mut().col_set(size.cols - width);
                }
            }
            self.grid_mut().col_wrap(width, wrap);
        }
        let pos = self.grid().pos();

        if width == 0 {
            if pos.col > 0 {
                let mut prev_cell = self
                    .grid_mut()
                    .drawing_cell_mut(crate::grid::Pos {
                        row: pos.row,
                        col: pos.col - 1,
                    })
                    // pos.row is valid, since it comes directly from
                    // self.grid().pos() which we assume to always have a
                    // valid row value. pos.col - 1 is valid because we just
                    // checked for pos.col > 0.
                    .unwrap();
                if prev_cell.is_wide_continuation() {
                    prev_cell = self
                        .grid_mut()
                        .drawing_cell_mut(crate::grid::Pos {
                            row: pos.row,
                            col: pos.col - 2,
                        })
                        // pos.row is valid, since it comes directly from
                        // self.grid().pos() which we assume to always have a
                        // valid row value. we know pos.col - 2 is valid
                        // because the cell at pos.col - 1 is a wide
                        // continuation character, which means there must be
                        // the first half of the wide character before it.
                        .unwrap();
                }
                prev_cell.append(c);
            } else if pos.row > 0 {
                let prev_row = self
                    .grid()
                    .drawing_row(pos.row - 1)
                    // pos.row is valid, since it comes directly from
                    // self.grid().pos() which we assume to always have a
                    // valid row value. pos.row - 1 is valid because we just
                    // checked for pos.row > 0.
                    .unwrap();
                if prev_row.wrapped() {
                    let mut prev_cell = self
                        .grid_mut()
                        .drawing_cell_mut(crate::grid::Pos {
                            row: pos.row - 1,
                            col: size.cols - 1,
                        })
                        // pos.row is valid, since it comes directly from
                        // self.grid().pos() which we assume to always have a
                        // valid row value. pos.row - 1 is valid because we
                        // just checked for pos.row > 0. col of size.cols - 1
                        // is always valid.
                        .unwrap();
                    if prev_cell.is_wide_continuation() {
                        prev_cell = self
                            .grid_mut()
                            .drawing_cell_mut(crate::grid::Pos {
                                row: pos.row - 1,
                                col: size.cols - 2,
                            })
                            // pos.row is valid, since it comes directly from
                            // self.grid().pos() which we assume to always
                            // have a valid row value. pos.row - 1 is valid
                            // because we just checked for pos.row > 0. col of
                            // size.cols - 2 is valid because the cell at
                            // size.cols - 1 is a wide continuation character,
                            // so it must have the first half of the wide
                            // character before it.
                            .unwrap();
                    }
                    prev_cell.append(c);
                }
            }
        } else {
            if self.insert_mode() {
                self.grid_mut().insert_cells(width);
            }

            if self
                .grid()
                .drawing_cell(pos)
                // pos.row is valid because we assume self.grid().pos() to
                // always have a valid row value. pos.col is valid because we
                // called col_wrap() immediately before this, which ensures
                // that self.grid().pos().col has a valid value.
                .unwrap()
                .is_wide_continuation()
            {
                let prev_cell = self
                    .grid_mut()
                    .drawing_cell_mut(crate::grid::Pos {
                        row: pos.row,
                        col: pos.col - 1,
                    })
                    // pos.row is valid because we assume self.grid().pos() to
                    // always have a valid row value. pos.col is valid because
                    // we called col_wrap() immediately before this, which
                    // ensures that self.grid().pos().col has a valid value.
                    // pos.col - 1 is valid because the cell at pos.col is a
                    // wide continuation character, so it must have the first
                    // half of the wide character before it.
                    .unwrap();
                prev_cell.clear(attrs);
            }

            if self
                .grid()
                .drawing_cell(pos)
                // pos.row is valid because we assume self.grid().pos() to
                // always have a valid row value. pos.col is valid because we
                // called col_wrap() immediately before this, which ensures
                // that self.grid().pos().col has a valid value.
                .unwrap()
                .is_wide()
            {
                let next_cell = self
                    .grid_mut()
                    .drawing_cell_mut(crate::grid::Pos {
                        row: pos.row,
                        col: pos.col + 1,
                    })
                    // pos.row is valid because we assume self.grid().pos() to
                    // always have a valid row value. pos.col is valid because
                    // we called col_wrap() immediately before this, which
                    // ensures that self.grid().pos().col has a valid value.
                    // pos.col + 1 is valid because the cell at pos.col is a
                    // wide character, so it must have the second half of the
                    // wide character after it.
                    .unwrap();
                next_cell.set(' ', attrs, None);
            }

            let cell = self
                .grid_mut()
                .drawing_cell_mut(pos)
                // pos.row is valid because we assume self.grid().pos() to
                // always have a valid row value. pos.col is valid because we
                // called col_wrap() immediately before this, which ensures
                // that self.grid().pos().col has a valid value.
                .unwrap();
            cell.set(c, attrs, hyperlink_id);
            self.grid_mut().col_inc(1);
            if width > 1 {
                let pos = self.grid().pos();
                if self
                    .grid()
                    .drawing_cell(pos)
                    // pos.row is valid because we assume self.grid().pos() to
                    // always have a valid row value. pos.col is valid because
                    // we called col_wrap() earlier, which ensures that
                    // self.grid().pos().col has a valid value. this is true
                    // even though we just called col_inc, because this branch
                    // only happens if width > 1, and col_wrap takes width
                    // into account.
                    .unwrap()
                    .is_wide()
                {
                    let next_next_pos = crate::grid::Pos {
                        row: pos.row,
                        col: pos.col + 1,
                    };
                    let next_next_cell = self
                        .grid_mut()
                        .drawing_cell_mut(next_next_pos)
                        // pos.row is valid because we assume
                        // self.grid().pos() to always have a valid row value.
                        // pos.col is valid because we called col_wrap()
                        // earlier, which ensures that self.grid().pos().col
                        // has a valid value. this is true even though we just
                        // called col_inc, because this branch only happens if
                        // width > 1, and col_wrap takes width into account.
                        // pos.col + 1 is valid because the cell at pos.col is
                        // wide, and so it must have the second half of the
                        // wide character after it.
                        .unwrap();
                    next_next_cell.clear(attrs);
                    if next_next_pos.col == size.cols - 1 {
                        self.grid_mut()
                            .drawing_row_mut(pos.row)
                            // we assume self.grid().pos().row is always valid
                            .unwrap()
                            .wrap(false);
                    }
                }
                let next_cell = self
                    .grid_mut()
                    .drawing_cell_mut(pos)
                    // pos.row is valid because we assume self.grid().pos() to
                    // always have a valid row value. pos.col is valid because
                    // we called col_wrap() earlier, which ensures that
                    // self.grid().pos().col has a valid value. this is true
                    // even though we just called col_inc, because this branch
                    // only happens if width > 1, and col_wrap takes width
                    // into account.
                    .unwrap();
                next_cell.clear(crate::attrs::Attrs::default());
                next_cell.set_wide_continuation(true);
                self.grid_mut().col_inc(1);
            }
        }
    }

    // control codes

    pub(crate) fn bs(&mut self) {
        if self.reverse_wraparound_mode() {
            self.grid_mut().reverse_wrap_col_dec(1);
        } else {
            self.grid_mut().col_dec(1);
        }
    }

    pub(crate) fn tab(&mut self) {
        self.grid_mut().col_tab();
    }

    pub(crate) fn lf(&mut self) {
        self.grid_mut().row_inc_scroll(1);
    }

    pub(crate) fn vt(&mut self) {
        self.lf();
    }

    pub(crate) fn ff(&mut self) {
        self.lf();
    }

    pub(crate) fn cr(&mut self) {
        self.grid_mut().col_set(0);
    }

    // escape codes

    // ESC 7
    pub(crate) fn decsc(&mut self) {
        self.save_cursor();
    }

    // ESC 8
    pub(crate) fn decrc(&mut self) {
        self.restore_cursor();
    }

    // ESC =
    pub(crate) fn deckpam(&mut self) {
        self.set_mode(MODE_APPLICATION_KEYPAD);
    }

    // ESC >
    pub(crate) fn deckpnm(&mut self) {
        self.clear_mode(MODE_APPLICATION_KEYPAD);
    }

    // ESC M
    pub(crate) fn ri(&mut self) {
        self.grid_mut().row_dec_scroll(1);
    }

    // ESC c
    pub(crate) fn ris(&mut self) {
        *self = Self::new(self.grid.size(), self.grid.scrollback_len());
    }

    // csi codes

    // CSI @
    pub(crate) fn ich(&mut self, count: u16) {
        self.grid_mut().insert_cells(count);
    }

    // CSI A
    pub(crate) fn cuu(&mut self, offset: u16) {
        self.grid_mut().row_dec_clamp(offset);
    }

    // CSI B
    pub(crate) fn cud(&mut self, offset: u16) {
        self.grid_mut().row_inc_clamp(offset);
    }

    // CSI C
    pub(crate) fn cuf(&mut self, offset: u16) {
        self.grid_mut().col_inc_clamp(offset);
    }

    // CSI D
    pub(crate) fn cub(&mut self, offset: u16) {
        self.grid_mut().col_dec(offset);
    }

    // CSI E
    pub(crate) fn cnl(&mut self, offset: u16) {
        self.grid_mut().col_set(0);
        self.grid_mut().row_inc_clamp(offset);
    }

    // CSI F
    pub(crate) fn cpl(&mut self, offset: u16) {
        self.grid_mut().col_set(0);
        self.grid_mut().row_dec_clamp(offset);
    }

    // CSI G
    pub(crate) fn cha(&mut self, col: u16) {
        self.grid_mut().col_set(col - 1);
    }

    // CSI H
    pub(crate) fn cup(&mut self, (row, col): (u16, u16)) {
        self.grid_mut().set_pos(crate::grid::Pos {
            row: row - 1,
            col: col - 1,
        });
    }

    // CSI J
    pub(crate) fn ed(
        &mut self,
        mode: u16,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        let attrs = self.attrs;
        match mode {
            0 => self.grid_mut().erase_all_forward(attrs),
            1 => self.grid_mut().erase_all_backward(attrs),
            2 => self.grid_mut().erase_all(attrs),
            3 => self.grid_mut().clear_scrollback(),
            _ => unhandled(self),
        }
    }

    // CSI ? J
    pub(crate) fn decsed(
        &mut self,
        mode: u16,
        unhandled: impl FnMut(&mut Self),
    ) {
        self.ed(mode, unhandled);
    }

    // CSI K
    pub(crate) fn el(
        &mut self,
        mode: u16,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        let attrs = self.attrs;
        match mode {
            0 => self.grid_mut().erase_row_forward(attrs),
            1 => self.grid_mut().erase_row_backward(attrs),
            2 => self.grid_mut().erase_row(attrs),
            _ => unhandled(self),
        }
    }

    // CSI ? K
    pub(crate) fn decsel(
        &mut self,
        mode: u16,
        unhandled: impl FnMut(&mut Self),
    ) {
        self.el(mode, unhandled);
    }

    // CSI L
    pub(crate) fn il(&mut self, count: u16) {
        self.grid_mut().insert_lines(count);
    }

    // CSI M
    pub(crate) fn dl(&mut self, count: u16) {
        self.grid_mut().delete_lines(count);
    }

    // CSI P
    pub(crate) fn dch(&mut self, count: u16) {
        self.grid_mut().delete_cells(count);
    }

    // CSI S
    pub(crate) fn su(&mut self, count: u16) {
        self.grid_mut().scroll_up(count);
    }

    // CSI T
    pub(crate) fn sd(&mut self, count: u16) {
        self.grid_mut().scroll_down(count);
    }

    // CSI X
    pub(crate) fn ech(&mut self, count: u16) {
        let attrs = self.attrs;
        self.grid_mut().erase_cells(count, attrs);
    }

    // CSI d
    pub(crate) fn vpa(&mut self, row: u16) {
        self.grid_mut().row_set(row - 1);
    }

    // CSI h
    pub(crate) fn sm(
        &mut self,
        params: &vte::Params,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        for param in params {
            match param {
                [4] => self.set_mode(MODE_INSERT),
                _ => unhandled(self),
            }
        }
    }

    // CSI l
    pub(crate) fn rm(
        &mut self,
        params: &vte::Params,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        for param in params {
            match param {
                [4] => self.clear_mode(MODE_INSERT),
                _ => unhandled(self),
            }
        }
    }

    // CSI ? h
    pub(crate) fn decset(
        &mut self,
        params: &vte::Params,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        for param in params {
            match param {
                [1] => self.set_mode(MODE_APPLICATION_CURSOR),
                [6] => self.grid_mut().set_origin_mode(true),
                [7] => self.set_mode(MODE_WRAPAROUND),
                [9] => self.set_mouse_mode(MouseProtocolMode::Press),
                [25] => self.clear_mode(MODE_HIDE_CURSOR),
                [45] => self.set_mode(MODE_REVERSE_WRAPAROUND),
                [47] => self.enter_alternate_grid(),
                [1000] => {
                    self.set_mouse_mode(MouseProtocolMode::PressRelease);
                }
                [1002] => {
                    self.set_mouse_mode(MouseProtocolMode::ButtonMotion);
                }
                [1003] => self.set_mouse_mode(MouseProtocolMode::AnyMotion),
                [1004] => self.set_mode(MODE_SEND_FOCUS),
                [1005] => {
                    self.set_mouse_encoding(MouseProtocolEncoding::Utf8);
                }
                [1006] => {
                    self.set_mouse_encoding(MouseProtocolEncoding::Sgr);
                }
                [1049] => {
                    self.decsc();
                    self.alternate_grid.clear();
                    self.enter_alternate_grid();
                }
                [2004] => self.set_mode(MODE_BRACKETED_PASTE),
                _ => unhandled(self),
            }
        }
    }

    // CSI ? l
    pub(crate) fn decrst(
        &mut self,
        params: &vte::Params,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        for param in params {
            match param {
                [1] => self.clear_mode(MODE_APPLICATION_CURSOR),
                [6] => self.grid_mut().set_origin_mode(false),
                [7] => self.clear_mode(MODE_WRAPAROUND),
                [9] => self.clear_mouse_mode(MouseProtocolMode::Press),
                [25] => self.set_mode(MODE_HIDE_CURSOR),
                [45] => self.clear_mode(MODE_REVERSE_WRAPAROUND),
                [47] => {
                    self.exit_alternate_grid();
                }
                [1000] => {
                    self.clear_mouse_mode(MouseProtocolMode::PressRelease);
                }
                [1002] => {
                    self.clear_mouse_mode(MouseProtocolMode::ButtonMotion);
                }
                [1003] => {
                    self.clear_mouse_mode(MouseProtocolMode::AnyMotion);
                }
                [1004] => self.clear_mode(MODE_SEND_FOCUS),
                [1005] => {
                    self.clear_mouse_encoding(MouseProtocolEncoding::Utf8);
                }
                [1006] => {
                    self.clear_mouse_encoding(MouseProtocolEncoding::Sgr);
                }
                [1049] => {
                    self.exit_alternate_grid();
                    self.decrc();
                }
                [2004] => self.clear_mode(MODE_BRACKETED_PASTE),
                _ => unhandled(self),
            }
        }
    }

    // CSI m
    pub(crate) fn sgr(
        &mut self,
        params: &vte::Params,
        mut unhandled: impl FnMut(&mut Self),
    ) {
        // XXX really i want to just be able to pass in a default Params
        // instance with a 0 in it, but vte doesn't allow creating new Params
        // instances
        if params.is_empty() {
            self.attrs = crate::attrs::Attrs::default();
            return;
        }

        let mut iter = params.iter();

        macro_rules! next_param {
            () => {
                match iter.next() {
                    Some(n) => n,
                    _ => return,
                }
            };
        }

        macro_rules! to_u8 {
            ($n:expr) => {
                if let Some(n) = u16_to_u8($n) {
                    n
                } else {
                    return;
                }
            };
        }

        macro_rules! next_param_u8 {
            () => {
                if let &[n] = next_param!() {
                    to_u8!(n)
                } else {
                    return;
                }
            };
        }

        loop {
            match next_param!() {
                [0] => self.attrs = crate::attrs::Attrs::default(),
                [1] => self.attrs.set_bold(),
                [2] => self.attrs.set_dim(),
                [3] => self.attrs.set_italic(true),
                [4] | [4, 1] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::Single),
                [4, 0] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::None),
                [4, 2] | [21] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::Double),
                [4, 3] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::Curly),
                [4, 4] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::Dotted),
                [4, 5] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::Dashed),
                [5] => self.attrs.set_blink(true),
                [7] => self.attrs.set_inverse(true),
                [8] => self.attrs.set_invisible(true),
                [9] => self.attrs.set_strikethrough(true),
                [22] => self.attrs.set_normal_intensity(),
                [23] => self.attrs.set_italic(false),
                [24] => self
                    .attrs
                    .set_underline_style(crate::UnderlineStyle::None),
                [25] => self.attrs.set_blink(false),
                [27] => self.attrs.set_inverse(false),
                [28] => self.attrs.set_invisible(false),
                [29] => self.attrs.set_strikethrough(false),
                [n] if (30..=37).contains(n) => {
                    self.attrs.fgcolor = crate::Color::Idx(to_u8!(*n) - 30);
                }
                [38, 2, r, g, b] => {
                    self.attrs.fgcolor =
                        crate::Color::Rgb(to_u8!(*r), to_u8!(*g), to_u8!(*b));
                }
                [38, 5, i] => {
                    self.attrs.fgcolor = crate::Color::Idx(to_u8!(*i));
                }
                [38] => match next_param!() {
                    [2] => {
                        let r = next_param_u8!();
                        let g = next_param_u8!();
                        let b = next_param_u8!();
                        self.attrs.fgcolor = crate::Color::Rgb(r, g, b);
                    }
                    [5] => {
                        self.attrs.fgcolor =
                            crate::Color::Idx(next_param_u8!());
                    }
                    _ => {
                        unhandled(self);
                        return;
                    }
                },
                [39] => {
                    self.attrs.fgcolor = crate::Color::Default;
                }
                [n] if (40..=47).contains(n) => {
                    self.attrs.bgcolor = crate::Color::Idx(to_u8!(*n) - 40);
                }
                [48, 2, r, g, b] => {
                    self.attrs.bgcolor =
                        crate::Color::Rgb(to_u8!(*r), to_u8!(*g), to_u8!(*b));
                }
                [48, 5, i] => {
                    self.attrs.bgcolor = crate::Color::Idx(to_u8!(*i));
                }
                [48] => match next_param!() {
                    [2] => {
                        let r = next_param_u8!();
                        let g = next_param_u8!();
                        let b = next_param_u8!();
                        self.attrs.bgcolor = crate::Color::Rgb(r, g, b);
                    }
                    [5] => {
                        self.attrs.bgcolor =
                            crate::Color::Idx(next_param_u8!());
                    }
                    _ => {
                        unhandled(self);
                        return;
                    }
                },
                [49] => {
                    self.attrs.bgcolor = crate::Color::Default;
                }
                [53] => self.attrs.set_overline(true),
                [55] => self.attrs.set_overline(false),
                [58, 2, r, g, b] => {
                    self.attrs.underline_color =
                        crate::Color::Rgb(to_u8!(*r), to_u8!(*g), to_u8!(*b));
                }
                [58, 2, _, r, g, b] => {
                    self.attrs.underline_color =
                        crate::Color::Rgb(to_u8!(*r), to_u8!(*g), to_u8!(*b));
                }
                [58, 5, i] => {
                    self.attrs.underline_color =
                        crate::Color::Idx(to_u8!(*i));
                }
                [58] => match next_param!() {
                    [2] => {
                        let r = next_param_u8!();
                        let g = next_param_u8!();
                        let b = next_param_u8!();
                        self.attrs.underline_color =
                            crate::Color::Rgb(r, g, b);
                    }
                    [5] => {
                        self.attrs.underline_color =
                            crate::Color::Idx(next_param_u8!());
                    }
                    _ => {
                        unhandled(self);
                        return;
                    }
                },
                [59] => {
                    self.attrs.underline_color = crate::Color::Default;
                }
                [n] if (90..=97).contains(n) => {
                    self.attrs.fgcolor = crate::Color::Idx(to_u8!(*n) - 82);
                }
                [n] if (100..=107).contains(n) => {
                    self.attrs.bgcolor = crate::Color::Idx(to_u8!(*n) - 92);
                }
                _ => unhandled(self),
            }
        }
    }

    // CSI r
    pub(crate) fn decstbm(&mut self, (top, bottom): (u16, u16)) {
        self.grid_mut().set_scroll_region(top - 1, bottom - 1);
    }
}

fn u16_to_u8(i: u16) -> Option<u8> {
    if i > u16::from(u8::MAX) {
        None
    } else {
        // safe because we just ensured that the value fits in a u8
        Some(i.try_into().unwrap())
    }
}
