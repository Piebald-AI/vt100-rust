use crate::term::BufWrite as _;

pub(crate) fn write_hyperlink_start(
    contents: &mut Vec<u8>,
    hyperlink: &crate::Hyperlink,
) {
    contents.extend_from_slice(b"\x1b]8;");
    contents.extend_from_slice(hyperlink.params());
    contents.push(b';');
    contents.extend_from_slice(hyperlink.uri());
    contents.extend_from_slice(b"\x1b\\");
}

pub(crate) fn write_hyperlink_end(contents: &mut Vec<u8>) {
    contents.extend_from_slice(b"\x1b]8;;\x1b\\");
}

pub(crate) fn write_hyperlink_diff(
    contents: &mut Vec<u8>,
    prev_hyperlink_id: &mut Option<crate::HyperlinkId>,
    next_hyperlink_id: Option<crate::HyperlinkId>,
    hyperlinks: &[crate::Hyperlink],
) {
    if hyperlink_eq(
        *prev_hyperlink_id,
        next_hyperlink_id,
        hyperlinks,
        hyperlinks,
    ) {
        *prev_hyperlink_id = next_hyperlink_id;
        return;
    }

    if prev_hyperlink_id.is_some() {
        write_hyperlink_end(contents);
    }
    if let Some(id) = next_hyperlink_id {
        if let Some(link) =
            hyperlinks.get(usize::try_from(id.0).ok().unwrap_or(usize::MAX))
        {
            write_hyperlink_start(contents, link);
            *prev_hyperlink_id = Some(id);
            return;
        }
    }
    *prev_hyperlink_id = None;
}

pub(crate) fn close_hyperlink(
    contents: &mut Vec<u8>,
    prev_hyperlink_id: &mut Option<crate::HyperlinkId>,
) {
    if prev_hyperlink_id.take().is_some() {
        write_hyperlink_end(contents);
    }
}

pub(crate) fn hyperlink_eq(
    a: Option<crate::HyperlinkId>,
    b: Option<crate::HyperlinkId>,
    a_hyperlinks: &[crate::Hyperlink],
    b_hyperlinks: &[crate::Hyperlink],
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let a = a_hyperlinks
                .get(usize::try_from(a.0).ok().unwrap_or(usize::MAX));
            let b = b_hyperlinks
                .get(usize::try_from(b.0).ok().unwrap_or(usize::MAX));
            a.is_some() && a == b
        }
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    cells: Vec<crate::Cell>,
    wrapped: bool,
    resize_wrapped: bool,
}

impl Row {
    pub fn new(cols: u16) -> Self {
        Self {
            cells: vec![crate::Cell::new(); usize::from(cols)],
            wrapped: false,
            resize_wrapped: false,
        }
    }

    fn cols(&self) -> u16 {
        self.cells
            .len()
            .try_into()
            // we limit the number of cols to a u16 (see Size)
            .unwrap()
    }

    pub fn clear(&mut self, attrs: crate::attrs::Attrs) {
        for cell in &mut self.cells {
            cell.clear(attrs);
        }
        self.wrapped = false;
        self.resize_wrapped = false;
    }

    fn cells(&self) -> impl Iterator<Item = &crate::Cell> {
        self.cells.iter()
    }

    pub(crate) fn cell_slice(&self) -> &[crate::Cell] {
        &self.cells
    }

    pub(crate) fn from_cells(
        mut cells: Vec<crate::Cell>,
        cols: u16,
        wrapped: bool,
    ) -> Self {
        cells.resize(usize::from(cols), crate::Cell::new());
        Self {
            cells,
            wrapped,
            resize_wrapped: wrapped,
        }
    }

    pub(crate) fn meaningful_len(&self) -> usize {
        let empty = crate::Cell::new();
        self.cells
            .iter()
            .rposition(|cell| cell != &empty)
            .map_or(0, |index| index + 1)
    }

    pub fn get(&self, col: u16) -> Option<&crate::Cell> {
        self.cells.get(usize::from(col))
    }

    pub fn get_mut(&mut self, col: u16) -> Option<&mut crate::Cell> {
        self.cells.get_mut(usize::from(col))
    }

    pub fn insert(&mut self, i: u16, cell: crate::Cell) {
        self.cells.insert(usize::from(i), cell);
        self.wrapped = false;
        self.resize_wrapped = false;
    }

    pub fn remove(&mut self, i: u16) {
        self.clear_wide(i);
        self.cells.remove(usize::from(i));
        self.wrapped = false;
        self.resize_wrapped = false;
    }

    pub fn erase(&mut self, i: u16, attrs: crate::attrs::Attrs) {
        let wide = self.cells[usize::from(i)].is_wide();
        self.clear_wide(i);
        self.cells[usize::from(i)].clear(attrs);
        if i == self.cols() - if wide { 2 } else { 1 } {
            self.wrapped = false;
            self.resize_wrapped = false;
        }
    }

    pub fn truncate(&mut self, len: u16) {
        self.cells.truncate(usize::from(len));
        self.wrapped = false;
        self.resize_wrapped = false;
        let last_cell = &mut self.cells[usize::from(len) - 1];
        if last_cell.is_wide() {
            last_cell.clear(*last_cell.attrs());
        }
    }

    pub fn resize(&mut self, len: u16, cell: crate::Cell) {
        self.cells.resize(usize::from(len), cell);
        self.wrapped = false;
        self.resize_wrapped = false;
    }

    pub fn wrap(&mut self, wrap: bool) {
        self.wrapped = wrap;
    }

    pub fn wrapped(&self) -> bool {
        self.wrapped
    }

    pub(crate) fn resize_wrapped(&self) -> bool {
        self.resize_wrapped
    }

    pub fn clear_wide(&mut self, col: u16) {
        let cell = &self.cells[usize::from(col)];
        let other = if cell.is_wide() {
            &mut self.cells[usize::from(col + 1)]
        } else if cell.is_wide_continuation() {
            &mut self.cells[usize::from(col - 1)]
        } else {
            return;
        };
        other.clear(*other.attrs());
    }

    pub fn write_contents(
        &self,
        contents: &mut String,
        start: u16,
        width: u16,
        wrapping: bool,
    ) {
        let mut prev_was_wide = false;

        let mut prev_col = start;
        for (col, cell) in self
            .cells()
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            if prev_was_wide {
                prev_was_wide = false;
                continue;
            }
            prev_was_wide = cell.is_wide();

            // we limit the number of cols to a u16 (see Size)
            let col: u16 = col.try_into().unwrap();
            if cell.has_contents() {
                for _ in 0..(col - prev_col) {
                    contents.push(' ');
                }
                prev_col += col - prev_col;

                contents.push_str(cell.contents());
                prev_col += if cell.is_wide() { 2 } else { 1 };
            }
        }
        if prev_col == start && wrapping {
            contents.push('\n');
        }
    }

    pub fn write_contents_formatted(
        &self,
        contents: &mut Vec<u8>,
        start: u16,
        width: u16,
        row: u16,
        wrapping: bool,
        prev_pos: Option<crate::grid::Pos>,
        prev_attrs: Option<crate::attrs::Attrs>,
        prev_hyperlink_id: Option<crate::HyperlinkId>,
        hyperlinks: &[crate::Hyperlink],
    ) -> (
        crate::grid::Pos,
        crate::attrs::Attrs,
        Option<crate::HyperlinkId>,
    ) {
        let mut prev_was_wide = false;
        let default_cell = crate::Cell::new();

        let mut prev_pos = prev_pos.unwrap_or_else(|| {
            if wrapping {
                crate::grid::Pos {
                    row: row - 1,
                    col: self.cols(),
                }
            } else {
                crate::grid::Pos { row, col: start }
            }
        });
        let mut prev_attrs = prev_attrs.unwrap_or_default();
        let mut prev_hyperlink_id = prev_hyperlink_id;

        let first_cell = &self.cells[usize::from(start)];
        if wrapping && first_cell == &default_cell {
            let default_attrs = default_cell.attrs();
            if &prev_attrs != default_attrs {
                default_attrs.write_escape_code_diff(contents, &prev_attrs);
                prev_attrs = *default_attrs;
            }
            contents.push(b' ');
            crate::term::Backspace.write_buf(contents);
            crate::term::EraseChar::new(1).write_buf(contents);
            prev_pos = crate::grid::Pos { row, col: 0 };
        }

        let mut erase: Option<(u16, &crate::attrs::Attrs)> = None;
        for (col, cell) in self
            .cells()
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            if prev_was_wide {
                prev_was_wide = false;
                continue;
            }
            prev_was_wide = cell.is_wide();

            // we limit the number of cols to a u16 (see Size)
            let col: u16 = col.try_into().unwrap();
            let pos = crate::grid::Pos { row, col };

            if let Some((prev_col, attrs)) = erase {
                if cell.has_contents() || cell.attrs() != attrs {
                    let new_pos = crate::grid::Pos { row, col: prev_col };
                    if wrapping
                        && prev_pos.row + 1 == new_pos.row
                        && prev_pos.col >= self.cols()
                    {
                        if new_pos.col > 0 {
                            contents.extend(
                                " ".repeat(usize::from(new_pos.col))
                                    .as_bytes(),
                            );
                        } else {
                            contents.extend(b" ");
                            crate::term::Backspace.write_buf(contents);
                        }
                    } else {
                        crate::term::MoveFromTo::new(prev_pos, new_pos)
                            .write_buf(contents);
                    }
                    prev_pos = new_pos;
                    close_hyperlink(contents, &mut prev_hyperlink_id);
                    if &prev_attrs != attrs {
                        attrs.write_escape_code_diff(contents, &prev_attrs);
                        prev_attrs = *attrs;
                    }
                    crate::term::EraseChar::new(pos.col - prev_col)
                        .write_buf(contents);
                    erase = None;
                }
            }

            if cell != &default_cell {
                let attrs = cell.attrs();
                if cell.has_contents() {
                    if pos != prev_pos {
                        if !wrapping
                            || prev_pos.row + 1 != pos.row
                            || prev_pos.col
                                < self.cols() - u16::from(cell.is_wide())
                            || pos.col != 0
                        {
                            crate::term::MoveFromTo::new(prev_pos, pos)
                                .write_buf(contents);
                        }
                        prev_pos = pos;
                    }

                    if &prev_attrs != attrs {
                        attrs.write_escape_code_diff(contents, &prev_attrs);
                        prev_attrs = *attrs;
                    }
                    write_hyperlink_diff(
                        contents,
                        &mut prev_hyperlink_id,
                        cell.hyperlink_id(),
                        hyperlinks,
                    );

                    prev_pos.col += if cell.is_wide() { 2 } else { 1 };
                    let cell_contents = cell.contents();
                    contents.extend(cell_contents.as_bytes());
                } else if erase.is_none() {
                    erase = Some((pos.col, attrs));
                }
            }
        }
        if let Some((prev_col, attrs)) = erase {
            let new_pos = crate::grid::Pos { row, col: prev_col };
            if wrapping
                && prev_pos.row + 1 == new_pos.row
                && prev_pos.col >= self.cols()
            {
                if new_pos.col > 0 {
                    contents.extend(
                        " ".repeat(usize::from(new_pos.col)).as_bytes(),
                    );
                } else {
                    contents.extend(b" ");
                    crate::term::Backspace.write_buf(contents);
                }
            } else {
                crate::term::MoveFromTo::new(prev_pos, new_pos)
                    .write_buf(contents);
            }
            prev_pos = new_pos;
            close_hyperlink(contents, &mut prev_hyperlink_id);
            if &prev_attrs != attrs {
                attrs.write_escape_code_diff(contents, &prev_attrs);
                prev_attrs = *attrs;
            }
            crate::term::ClearRowForward.write_buf(contents);
        }

        (prev_pos, prev_attrs, prev_hyperlink_id)
    }

    /// Write this row's formatted cell contents without cursor positioning.
    /// Only emits SGR attribute changes and cell text. Cells with
    /// non-default attributes but no text content are rendered as spaces
    /// with the appropriate attributes.
    /// Returns the active attributes after this row.
    pub fn write_contents_formatted_inline(
        &self,
        contents: &mut Vec<u8>,
        start: u16,
        width: u16,
        mut prev_attrs: crate::attrs::Attrs,
        mut prev_hyperlink_id: Option<crate::HyperlinkId>,
        hyperlinks: &[crate::Hyperlink],
    ) -> (crate::attrs::Attrs, Option<crate::HyperlinkId>) {
        let default_cell = crate::Cell::new();
        let mut prev_was_wide = false;

        // First, find the last column that has non-default content so we
        // can avoid trailing whitespace/formatting (matching the behavior
        // of write_contents which omits trailing empty cells).
        let mut last_non_default: Option<u16> = None;
        for (col, cell) in self
            .cells()
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            let col: u16 = col.try_into().unwrap();
            if cell != &default_cell {
                last_non_default = Some(col);
            }
        }

        let Some(end_col) = last_non_default else {
            return (prev_attrs, prev_hyperlink_id);
        };

        let mut prev_col = start;
        for (col, cell) in self
            .cells()
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            if prev_was_wide {
                prev_was_wide = false;
                continue;
            }
            prev_was_wide = cell.is_wide();

            let col: u16 = col.try_into().unwrap();
            if col > end_col {
                break;
            }

            if cell.has_contents() {
                // Fill any gap with spaces (for cells between prev_col
                // and this col that had no content)
                if prev_col < col {
                    close_hyperlink(contents, &mut prev_hyperlink_id);
                    for _ in prev_col..col {
                        contents.push(b' ');
                    }
                }
                prev_col = col;

                let attrs = cell.attrs();
                if &prev_attrs != attrs {
                    attrs.write_escape_code_diff(contents, &prev_attrs);
                    prev_attrs = *attrs;
                }
                write_hyperlink_diff(
                    contents,
                    &mut prev_hyperlink_id,
                    cell.hyperlink_id(),
                    hyperlinks,
                );
                contents.extend(cell.contents().as_bytes());
                prev_col += if cell.is_wide() { 2 } else { 1 };
            } else if cell != &default_cell {
                // Cell has non-default attrs but no text content (e.g.
                // background color). Emit a space with those attrs.
                if prev_col < col {
                    close_hyperlink(contents, &mut prev_hyperlink_id);
                    for _ in prev_col..col {
                        contents.push(b' ');
                    }
                }
                prev_col = col;

                let attrs = cell.attrs();
                if &prev_attrs != attrs {
                    attrs.write_escape_code_diff(contents, &prev_attrs);
                    prev_attrs = *attrs;
                }
                write_hyperlink_diff(
                    contents,
                    &mut prev_hyperlink_id,
                    cell.hyperlink_id(),
                    hyperlinks,
                );
                contents.push(b' ');
                prev_col += 1;
            }
        }

        (prev_attrs, prev_hyperlink_id)
    }

    pub fn write_contents_formatted_inline_with_context(
        &self,
        contents: &mut Vec<u8>,
        start: u16,
        width: u16,
        mut prev_attrs: crate::attrs::Attrs,
        mut prev_hyperlink_id: Option<crate::HyperlinkId>,
        hyperlinks: &[crate::Hyperlink],
        context: &crate::SerializeContext<'_>,
    ) -> (crate::attrs::Attrs, Option<crate::HyperlinkId>) {
        let default_cell = crate::Cell::new();
        let include_text_cells = context.color_mode
            == crate::SerializeColorMode::ResolvePaletteToRgb;
        let mut prev_was_wide = false;
        let mut last_non_default: Option<u16> = None;
        for (col, cell) in self
            .cells()
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            let col: u16 = col.try_into().unwrap();
            if cell != &default_cell
                || (include_text_cells && cell.has_contents())
            {
                last_non_default = Some(col);
            }
        }

        let Some(end_col) = last_non_default else {
            return (prev_attrs, prev_hyperlink_id);
        };

        let mut prev_col = start;
        for (col, cell) in self
            .cells()
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            if prev_was_wide {
                prev_was_wide = false;
                continue;
            }
            prev_was_wide = cell.is_wide();

            let col: u16 = col.try_into().unwrap();
            if col > end_col {
                break;
            }

            if cell.has_contents() {
                if prev_col < col {
                    close_hyperlink(contents, &mut prev_hyperlink_id);
                    for _ in prev_col..col {
                        contents.push(b' ');
                    }
                }
                prev_col = col;

                let attrs = cell.attrs();
                if &prev_attrs != attrs || include_text_cells {
                    attrs.write_escape_code_diff_with_context(
                        contents,
                        &prev_attrs,
                        context,
                    );
                    prev_attrs = *attrs;
                }
                write_hyperlink_diff(
                    contents,
                    &mut prev_hyperlink_id,
                    cell.hyperlink_id(),
                    hyperlinks,
                );
                contents.extend(cell.contents().as_bytes());
                prev_col += if cell.is_wide() { 2 } else { 1 };
            } else if cell != &default_cell {
                if prev_col < col {
                    close_hyperlink(contents, &mut prev_hyperlink_id);
                    for _ in prev_col..col {
                        contents.push(b' ');
                    }
                }
                prev_col = col;

                let attrs = cell.attrs();
                if &prev_attrs != attrs || include_text_cells {
                    attrs.write_escape_code_diff_with_context(
                        contents,
                        &prev_attrs,
                        context,
                    );
                    prev_attrs = *attrs;
                }
                write_hyperlink_diff(
                    contents,
                    &mut prev_hyperlink_id,
                    cell.hyperlink_id(),
                    hyperlinks,
                );
                contents.push(b' ');
                prev_col += 1;
            }
        }

        (prev_attrs, prev_hyperlink_id)
    }

    // while it's true that most of the logic in this is identical to
    // write_contents_formatted, i can't figure out how to break out the
    // common parts without making things noticeably slower.
    pub fn write_contents_diff(
        &self,
        contents: &mut Vec<u8>,
        prev: &Self,
        self_hyperlinks: &[crate::Hyperlink],
        prev_hyperlinks: &[crate::Hyperlink],
        start: u16,
        width: u16,
        row: u16,
        wrapping: bool,
        prev_wrapping: bool,
        mut prev_pos: crate::grid::Pos,
        mut prev_attrs: crate::attrs::Attrs,
        mut prev_hyperlink_id: Option<crate::HyperlinkId>,
    ) -> (
        crate::grid::Pos,
        crate::attrs::Attrs,
        Option<crate::HyperlinkId>,
    ) {
        let mut prev_was_wide = false;

        let first_cell = &self.cells[usize::from(start)];
        let prev_first_cell = &prev.cells[usize::from(start)];
        if wrapping
            && !prev_wrapping
            && first_cell == prev_first_cell
            && prev_pos.row + 1 == row
            && prev_pos.col
                >= self.cols() - u16::from(prev_first_cell.is_wide())
        {
            let first_cell_attrs = first_cell.attrs();
            if &prev_attrs != first_cell_attrs {
                first_cell_attrs
                    .write_escape_code_diff(contents, &prev_attrs);
                prev_attrs = *first_cell_attrs;
            }
            write_hyperlink_diff(
                contents,
                &mut prev_hyperlink_id,
                first_cell.hyperlink_id(),
                self_hyperlinks,
            );
            let mut cell_contents = prev_first_cell.contents();
            let need_erase = if cell_contents.is_empty() {
                cell_contents = " ";
                true
            } else {
                false
            };
            contents.extend(cell_contents.as_bytes());
            crate::term::Backspace.write_buf(contents);
            if prev_first_cell.is_wide() {
                crate::term::Backspace.write_buf(contents);
            }
            if need_erase {
                crate::term::EraseChar::new(1).write_buf(contents);
            }
            prev_pos = crate::grid::Pos { row, col: 0 };
        }

        let mut erase: Option<(u16, &crate::attrs::Attrs)> = None;
        for (col, (cell, prev_cell)) in self
            .cells()
            .zip(prev.cells())
            .enumerate()
            .skip(usize::from(start))
            .take(usize::from(width))
        {
            if prev_was_wide {
                prev_was_wide = false;
                continue;
            }
            prev_was_wide = cell.is_wide();

            // we limit the number of cols to a u16 (see Size)
            let col: u16 = col.try_into().unwrap();
            let pos = crate::grid::Pos { row, col };

            if let Some((prev_col, attrs)) = erase {
                if cell.has_contents() || cell.attrs() != attrs {
                    let new_pos = crate::grid::Pos { row, col: prev_col };
                    if wrapping
                        && prev_pos.row + 1 == new_pos.row
                        && prev_pos.col >= self.cols()
                    {
                        if new_pos.col > 0 {
                            contents.extend(
                                " ".repeat(usize::from(new_pos.col))
                                    .as_bytes(),
                            );
                        } else {
                            contents.extend(b" ");
                            crate::term::Backspace.write_buf(contents);
                        }
                    } else {
                        crate::term::MoveFromTo::new(prev_pos, new_pos)
                            .write_buf(contents);
                    }
                    prev_pos = new_pos;
                    close_hyperlink(contents, &mut prev_hyperlink_id);
                    if &prev_attrs != attrs {
                        attrs.write_escape_code_diff(contents, &prev_attrs);
                        prev_attrs = *attrs;
                    }
                    crate::term::EraseChar::new(pos.col - prev_col)
                        .write_buf(contents);
                    erase = None;
                }
            }

            if cell != prev_cell
                || !hyperlink_eq(
                    cell.hyperlink_id(),
                    prev_cell.hyperlink_id(),
                    self_hyperlinks,
                    prev_hyperlinks,
                )
            {
                let attrs = cell.attrs();
                if cell.has_contents() {
                    if pos != prev_pos {
                        if !wrapping
                            || prev_pos.row + 1 != pos.row
                            || prev_pos.col
                                < self.cols() - u16::from(cell.is_wide())
                            || pos.col != 0
                        {
                            crate::term::MoveFromTo::new(prev_pos, pos)
                                .write_buf(contents);
                        }
                        prev_pos = pos;
                    }

                    if &prev_attrs != attrs {
                        attrs.write_escape_code_diff(contents, &prev_attrs);
                        prev_attrs = *attrs;
                    }
                    write_hyperlink_diff(
                        contents,
                        &mut prev_hyperlink_id,
                        cell.hyperlink_id(),
                        self_hyperlinks,
                    );

                    prev_pos.col += if cell.is_wide() { 2 } else { 1 };
                    contents.extend(cell.contents().as_bytes());
                } else if erase.is_none() {
                    erase = Some((pos.col, attrs));
                }
            }
        }
        if let Some((prev_col, attrs)) = erase {
            let new_pos = crate::grid::Pos { row, col: prev_col };
            if wrapping
                && prev_pos.row + 1 == new_pos.row
                && prev_pos.col >= self.cols()
            {
                if new_pos.col > 0 {
                    contents.extend(
                        " ".repeat(usize::from(new_pos.col)).as_bytes(),
                    );
                } else {
                    contents.extend(b" ");
                    crate::term::Backspace.write_buf(contents);
                }
            } else {
                crate::term::MoveFromTo::new(prev_pos, new_pos)
                    .write_buf(contents);
            }
            prev_pos = new_pos;
            close_hyperlink(contents, &mut prev_hyperlink_id);
            if &prev_attrs != attrs {
                attrs.write_escape_code_diff(contents, &prev_attrs);
                prev_attrs = *attrs;
            }
            crate::term::ClearRowForward.write_buf(contents);
        }

        // if this row is going from wrapped to not wrapped, we need to erase
        // and redraw the last character to break wrapping. if this row is
        // wrapped, we need to redraw the last character without erasing it to
        // position the cursor after the end of the line correctly so that
        // drawing the next line can just start writing and be wrapped.
        if (!self.wrapped && prev.wrapped) || (!prev.wrapped && self.wrapped)
        {
            let end_pos = if self.cells[usize::from(self.cols() - 1)]
                .is_wide_continuation()
            {
                crate::grid::Pos {
                    row,
                    col: self.cols() - 2,
                }
            } else {
                crate::grid::Pos {
                    row,
                    col: self.cols() - 1,
                }
            };
            crate::term::MoveFromTo::new(prev_pos, end_pos)
                .write_buf(contents);
            prev_pos = end_pos;
            if !self.wrapped {
                close_hyperlink(contents, &mut prev_hyperlink_id);
                crate::term::EraseChar::new(1).write_buf(contents);
            }
            let end_cell = &self.cells[usize::from(end_pos.col)];
            if end_cell.has_contents() {
                let attrs = end_cell.attrs();
                if &prev_attrs != attrs {
                    attrs.write_escape_code_diff(contents, &prev_attrs);
                    prev_attrs = *attrs;
                }
                write_hyperlink_diff(
                    contents,
                    &mut prev_hyperlink_id,
                    end_cell.hyperlink_id(),
                    self_hyperlinks,
                );
                contents.extend(end_cell.contents().as_bytes());
                prev_pos.col += if end_cell.is_wide() { 2 } else { 1 };
            }
        }

        (prev_pos, prev_attrs, prev_hyperlink_id)
    }
}
