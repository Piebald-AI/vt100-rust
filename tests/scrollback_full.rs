mod helpers;

#[test]
fn basic_scrollback_plain_text() {
    // Create a 3-row, 80-col terminal with scrollback=100
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write 10 lines (7 will scroll into scrollback, 3 on screen)
    parser.process(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9\r\n10");

    // contents() shows only the visible screen
    assert_eq!(parser.screen().contents(), "8\n9\n10");

    // contents_full() shows all lines including scrollback
    assert_eq!(
        parser.screen().contents_full(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10"
    );
}

#[test]
fn basic_scrollback_many_lines() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write 50 lines
    let input: String = (1..=50)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\r\n");
    parser.process(input.as_bytes());

    let expected: String = (1..=50)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(parser.screen().contents_full(), expected);
    assert_eq!(parser.screen().contents(), "48\n49\n50");
}

#[test]
fn formatting_preservation() {
    // Create a terminal with scrollback
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write lines with different colors
    // Line 1: red text
    parser.process(b"\x1b[31mred line\r\n");
    // Line 2: green text
    parser.process(b"\x1b[32mgreen line\r\n");
    // Line 3: blue text
    parser.process(b"\x1b[34mblue line\r\n");
    // Line 4: bold text
    parser.process(b"\x1b[1mbold line\r\n");
    // Line 5: normal
    parser.process(b"\x1b[mnormal");

    // Verify plain text full contents
    assert_eq!(
        parser.screen().contents_full(),
        "red line\ngreen line\nblue line\nbold line\nnormal"
    );

    // Get formatted full output
    let formatted = parser.screen().contents_formatted_full();

    // Feed it into a new parser (with enough rows)
    let mut new_parser = vt100::Parser::new(5, 80, 0);
    new_parser.process(&formatted);

    // Verify the text matches
    assert_eq!(new_parser.screen().contents(), parser.screen().contents_full());

    // Verify formatting on specific cells
    // Row 0 (red line): should be red (color idx 1)
    assert_eq!(
        new_parser.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
    // Row 1 (green line): should be green (color idx 2)
    assert_eq!(
        new_parser.screen().cell(1, 0).unwrap().fgcolor(),
        vt100::Color::Idx(2)
    );
    // Row 2 (blue line): should be blue (color idx 4)
    assert_eq!(
        new_parser.screen().cell(2, 0).unwrap().fgcolor(),
        vt100::Color::Idx(4)
    );
    // Row 3 (bold line): should be bold
    assert!(new_parser.screen().cell(3, 0).unwrap().bold());
    // Row 4 (normal): should not be bold, default color
    assert!(!new_parser.screen().cell(4, 0).unwrap().bold());
    assert_eq!(
        new_parser.screen().cell(4, 0).unwrap().fgcolor(),
        vt100::Color::Default
    );
}

#[test]
fn no_scrollback() {
    // With scrollback=0, contents_full() should match contents()
    let mut parser = vt100::Parser::new(24, 80, 0);
    parser.process(b"hello world\r\nline two\r\nline three");

    assert_eq!(parser.screen().contents_full(), parser.screen().contents());

    // For formatted, the text should be equivalent when re-parsed
    let formatted_full = parser.screen().contents_formatted_full();
    let mut new_parser = vt100::Parser::new(24, 80, 0);
    new_parser.process(&formatted_full);
    assert_eq!(new_parser.screen().contents(), parser.screen().contents());
}

#[test]
fn wrapped_lines() {
    // Create narrow terminal
    let mut parser = vt100::Parser::new(3, 10, 100);

    // Write a line longer than terminal width (will wrap)
    parser.process(b"0123456789abcde\r\nshort");

    // The full plain text should show the wrapped content correctly
    let full = parser.screen().contents_full();
    // "0123456789" wraps to "abcde" then "short" on next line
    assert!(full.contains("0123456789"));
    assert!(full.contains("abcde"));
    assert!(full.contains("short"));

    // The wrapped line shouldn't have a newline between its parts
    // but non-wrapped lines should
    let lines: Vec<&str> = full.split('\n').collect();
    assert_eq!(lines[0], "0123456789abcde");
    assert_eq!(lines[1], "short");
}

#[test]
fn empty_terminal() {
    let parser = vt100::Parser::new(24, 80, 100);

    // Empty terminal should return empty string (trailing newlines trimmed)
    assert_eq!(parser.screen().contents_full(), "");

    // Formatted output - all rows are empty/default, so the only output
    // should be the clear attrs prefix plus \r\n between the 24 empty rows.
    // When round-tripped, it should produce an equivalent empty terminal.
    let formatted = parser.screen().contents_formatted_full();
    let mut new_parser = vt100::Parser::new(24, 80, 0);
    new_parser.process(&formatted);
    assert_eq!(new_parser.screen().contents(), "");
}

#[test]
fn round_trip_test() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write enough lines to have scrollback
    parser.process(b"\x1b[31mline 1\r\n\x1b[32mline 2\r\n\x1b[34mline 3\r\n\x1b[33mline 4\r\n\x1b[35mline 5\r\n\x1b[36mline 6");

    let plain_full = parser.screen().contents_full();
    let formatted_full = parser.screen().contents_formatted_full();

    // Feed the formatted output into a new parser with enough rows
    let total_lines = plain_full.matches('\n').count() + 1;
    let mut new_parser =
        vt100::Parser::new(total_lines.try_into().unwrap(), 80, 0);
    new_parser.process(&formatted_full);

    // Verify plain text matches
    assert_eq!(new_parser.screen().contents(), plain_full);
}

#[test]
fn alternate_screen() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write some lines to the main screen (with scrollback)
    parser.process(b"main1\r\nmain2\r\nmain3\r\nmain4\r\nmain5");

    let main_full = parser.screen().contents_full();
    assert_eq!(main_full, "main1\nmain2\nmain3\nmain4\nmain5");

    // Enter alternate screen
    parser.process(b"\x1b[?1049h");
    assert!(parser.screen().alternate_screen());

    // Write to alternate screen
    parser.process(b"alt1\r\nalt2\r\nalt3");

    // contents_full() should still return main grid's history
    assert_eq!(parser.screen().contents_full(), main_full);

    // contents_formatted_full() should also return main grid's formatted
    let formatted = parser.screen().contents_formatted_full();
    let total_lines = main_full.matches('\n').count() + 1;
    let mut new_parser =
        vt100::Parser::new(total_lines.try_into().unwrap(), 80, 0);
    new_parser.process(&formatted);
    assert_eq!(new_parser.screen().contents(), main_full);

    // Exit alternate screen
    parser.process(b"\x1b[?1049l");
    assert!(!parser.screen().alternate_screen());
}

#[test]
fn rows_full_plain() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    parser.process(b"line1\r\nline2\r\nline3\r\nline4\r\nline5");

    let rows: Vec<String> = parser.screen().rows_full(0, 80).collect();

    // Should have 5 rows (2 scrollback + 3 screen)
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], "line1");
    assert_eq!(rows[1], "line2");
    assert_eq!(rows[2], "line3");
    assert_eq!(rows[3], "line4");
    assert_eq!(rows[4], "line5");
}

#[test]
fn rows_formatted_full_basic() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write colored lines
    parser.process(b"\x1b[31mred\r\n\x1b[32mgreen\r\n\x1b[34mblue\r\n\x1b[33myellow\r\n\x1b[mnormal");

    let rows: Vec<Vec<u8>> =
        parser.screen().rows_formatted_full(0, 80).collect();

    // Should have 5 rows
    assert_eq!(rows.len(), 5);

    // First row should contain red SGR code and "red"
    assert!(rows[0].len() > 0);
    let row0_str = String::from_utf8_lossy(&rows[0]);
    assert!(row0_str.contains("red"));
}

#[test]
fn scrollback_overflow_capped() {
    // Scrollback capped at the configured limit
    let mut parser = vt100::Parser::new(3, 80, 5);

    // Write 20 lines
    let input: String = (1..=20)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\r\n");
    parser.process(input.as_bytes());

    // Only the last 5 scrollback + 3 screen = 8 lines should be available
    let full = parser.screen().contents_full();
    let lines: Vec<&str> = full.split('\n').collect();
    assert_eq!(lines.len(), 8); // 5 scrollback + 3 screen
    assert_eq!(lines[0], "13");
    assert_eq!(lines[7], "20");
}

#[test]
fn formatted_full_with_background_colors() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    // Write text with background colors
    parser.process(b"\x1b[41m red bg \x1b[m\r\n\x1b[42m green bg \x1b[m\r\nno bg\r\nmore\r\nstuff");

    let formatted = parser.screen().contents_formatted_full();

    // Round-trip test
    let plain = parser.screen().contents_full();
    let total_lines = plain.matches('\n').count() + 1;
    let mut new_parser =
        vt100::Parser::new(total_lines.try_into().unwrap(), 80, 0);
    new_parser.process(&formatted);

    assert_eq!(new_parser.screen().contents(), plain);

    // Check that background colors are preserved
    assert_eq!(
        new_parser.screen().cell(0, 1).unwrap().bgcolor(),
        vt100::Color::Idx(1)
    );
    assert_eq!(
        new_parser.screen().cell(1, 1).unwrap().bgcolor(),
        vt100::Color::Idx(2)
    );
}

#[test]
fn rows_full_with_alternate_screen() {
    let mut parser = vt100::Parser::new(3, 80, 100);

    parser.process(b"main1\r\nmain2\r\nmain3\r\nmain4\r\nmain5");

    // Enter alternate screen
    parser.process(b"\x1b[?1049h");
    parser.process(b"alt content");

    // rows_full should return main grid rows
    let rows: Vec<String> = parser.screen().rows_full(0, 80).collect();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], "main1");
    assert_eq!(rows[4], "main5");

    // rows_formatted_full should also return main grid rows
    let formatted_rows: Vec<Vec<u8>> =
        parser.screen().rows_formatted_full(0, 80).collect();
    assert_eq!(formatted_rows.len(), 5);
}

#[test]
fn wide_characters() {
    let mut parser = vt100::Parser::new(3, 20, 100);

    // Write wide characters (CJK)
    parser.process("你好世界\r\nhello\r\nnext\r\nmore".as_bytes());

    let full = parser.screen().contents_full();
    assert!(full.contains("你好世界"));
    assert!(full.contains("hello"));

    // Round-trip test
    let formatted = parser.screen().contents_formatted_full();
    let total_lines = full.matches('\n').count() + 1;
    let mut new_parser =
        vt100::Parser::new(total_lines.try_into().unwrap(), 20, 0);
    new_parser.process(&formatted);
    assert_eq!(new_parser.screen().contents(), full);
}
