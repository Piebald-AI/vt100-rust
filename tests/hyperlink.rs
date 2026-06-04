fn process(input: &[u8]) -> vt100::Parser {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(input);
    parser
}

#[test]
fn parsing_and_cell_metadata() {
    let input = b"\x1b]8;;https://example.com\x07link\x1b]8;;\x07 plain";
    let parser = process(input);
    let screen = parser.screen();

    assert!(screen.active_hyperlink().is_none());
    let link = screen.cell_hyperlink(0, 0).unwrap();
    assert_eq!(link.params(), b"");
    assert_eq!(link.uri(), b"https://example.com");
    assert_eq!(
        screen.cell(0, 3).unwrap().hyperlink_id(),
        screen.cell(0, 0).unwrap().hyperlink_id()
    );
    assert!(screen.cell_hyperlink(0, 4).is_none());

    let parser =
        process(b"\x1b]8;id=docs;https://example.com/docs\x1b\\docs");
    let link = parser.screen().cell_hyperlink(0, 0).unwrap();
    assert_eq!(link.params(), b"id=docs");
    assert_eq!(link.uri(), b"https://example.com/docs");
}

#[test]
fn link_switching_replaces_active_link() {
    let parser = process(
        b"\x1b]8;id=a;https://a.example\x1b\\A\
          \x1b]8;id=b;https://b.example\x1b\\B\
          \x1b]8;;\x1b\\C",
    );
    let screen = parser.screen();

    let a = screen.cell_hyperlink(0, 0).unwrap();
    assert_eq!(a.params(), b"id=a");
    assert_eq!(a.uri(), b"https://a.example");

    let b = screen.cell_hyperlink(0, 1).unwrap();
    assert_eq!(b.params(), b"id=b");
    assert_eq!(b.uri(), b"https://b.example");

    assert!(screen.cell_hyperlink(0, 2).is_none());
}

#[test]
fn state_formatted_full_round_trips_hyperlinks() {
    let parser = process(b"hi \x1b]8;id=docs;https://example.com/docs\x1b\\docs\x1b]8;;\x1b\\!");
    let state = parser.screen().state_formatted_full();

    assert!(state.windows(3).any(|window| window == b"\x1b]8"));
    assert!(state.windows(3).any(|window| window == b"8;;"));
    assert!(!state.windows(5).any(|window| window == b"\x1b[c"));

    let mut replay = vt100::Parser::new(3, 10, 10);
    replay.process(&state);

    assert_eq!(replay.screen().contents(), parser.screen().contents());
    let link = replay.screen().cell_hyperlink(0, 3).unwrap();
    assert_eq!(link.params(), b"id=docs");
    assert_eq!(link.uri(), b"https://example.com/docs");
    assert!(replay.screen().cell_hyperlink(0, 7).is_none());
}

#[test]
fn contents_formatted_full_preserves_scrollback_hyperlinks() {
    let mut parser = vt100::Parser::new(2, 8, 10);
    parser.process(
        b"\x1b]8;;https://one.example\x1b\\one\x1b]8;;\x1b\\\r\ntwo\r\nthree",
    );

    let formatted = parser.screen().contents_formatted_full();
    assert!(formatted.windows(3).any(|window| window == b"\x1b]8"));

    let mut replay = vt100::Parser::new(4, 8, 10);
    replay.process(&formatted);
    let link = replay.screen().cell_hyperlink(0, 0).unwrap();
    assert_eq!(link.uri(), b"https://one.example");
}

#[test]
fn alternate_screen_state_round_trips_hyperlinks() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(
        b"main\x1b[?1049h\x1b]8;;https://alt.example\x1b\\alt\x1b]8;;\x1b\\",
    );

    let state = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(3, 10, 10);
    replay.process(&state);

    assert!(replay.screen().alternate_screen());
    let link = replay.screen().cell_hyperlink(0, 0).unwrap();
    assert_eq!(link.uri(), b"https://alt.example");
}

#[test]
fn erased_cells_do_not_retain_hyperlinks() {
    let mut parser =
        process(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
    parser.process(b"\r\x1b[K");

    for col in 0..4 {
        assert!(parser.screen().cell_hyperlink(0, col).is_none());
    }
}

#[test]
fn contents_diff_preserves_hyperlink_only_changes() {
    let mut prev = vt100::Parser::new(1, 10, 10);
    prev.process(b"link");

    let mut next = vt100::Parser::new(1, 10, 10);
    next.process(b"\x1b]8;;https://x\x1b\\link\x1b]8;;\x1b\\");

    let diff = next.screen().contents_diff(prev.screen());
    assert!(diff.windows(3).any(|window| window == b"\x1b]8"));

    let mut replay = vt100::Parser::new(1, 10, 10);
    replay.process(&prev.screen().contents_formatted());
    replay.process(&diff);

    let link = replay.screen().cell_hyperlink(0, 0).unwrap();
    assert_eq!(link.uri(), b"https://x");
}

#[test]
fn contents_diff_closes_hyperlinks_before_erasing() {
    let mut prev = vt100::Parser::new(1, 10, 10);
    prev.process(b"\x1b]8;;https://x\x1b\\link\x1b]8;;\x1b\\");

    let mut next = vt100::Parser::new(1, 10, 10);
    next.process(b"plain");

    let diff = next.screen().contents_diff(prev.screen());
    let mut replay = vt100::Parser::new(1, 10, 10);
    replay.process(&prev.screen().contents_formatted());
    replay.process(&diff);

    assert_eq!(replay.screen().contents(), "plain");
    for col in 0..5 {
        assert!(replay.screen().cell_hyperlink(0, col).is_none());
    }
}

#[test]
fn formatted_full_does_not_link_gap_filler_spaces() {
    let mut parser = vt100::Parser::new(1, 4, 10);
    parser.process(b"\x1b]8;;https://x\x1b\\A\x1b]8;;\x1b\\\x1b[1;3HB");

    let state = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(1, 4, 10);
    replay.process(&state);

    assert_eq!(replay.screen().contents(), "A B");
    assert_eq!(
        replay.screen().cell_hyperlink(0, 0).unwrap().uri(),
        b"https://x"
    );
    assert!(replay.screen().cell_hyperlink(0, 1).is_none());
    assert!(replay.screen().cell_hyperlink(0, 2).is_none());
}

#[test]
fn contents_diff_compares_resolved_hyperlink_metadata() {
    let mut prev = vt100::Parser::new(1, 10, 10);
    prev.process(b"\x1b]8;;https://a\x1b\\link\x1b]8;;\x1b\\");

    let mut next = vt100::Parser::new(1, 10, 10);
    next.process(b"\x1b]8;;https://b\x1b\\link\x1b]8;;\x1b\\");

    let diff = next.screen().contents_diff(prev.screen());
    assert!(diff.windows(3).any(|window| window == b"\x1b]8"));

    let mut replay = vt100::Parser::new(1, 10, 10);
    replay.process(&prev.screen().contents_formatted());
    replay.process(&diff);

    let link = replay.screen().cell_hyperlink(0, 0).unwrap();
    assert_eq!(link.uri(), b"https://b");
}

#[test]
fn state_formatted_full_restores_active_hyperlink() {
    let mut original = vt100::Parser::new(1, 10, 10);
    original.process(b"\x1b]8;;https://x\x1b\\ab");

    let state = original.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(1, 10, 10);
    replay.process(&state);

    original.process(b"c");
    replay.process(b"c");

    assert_eq!(original.screen().contents(), replay.screen().contents());
    assert_eq!(
        original.screen().cell_hyperlink(0, 2).unwrap().uri(),
        b"https://x"
    );
    assert_eq!(
        replay.screen().cell_hyperlink(0, 2).unwrap().uri(),
        b"https://x"
    );
}

#[test]
fn state_diff_restores_active_hyperlink() {
    let prev = vt100::Parser::new(1, 10, 10);
    let mut next = vt100::Parser::new(1, 10, 10);
    next.process(b"\x1b]8;;https://x\x1b\\ab");

    let diff = next.screen().state_diff(prev.screen());
    let mut replay = vt100::Parser::new(1, 10, 10);
    replay.process(&diff);
    replay.process(b"c");

    assert_eq!(
        replay.screen().cell_hyperlink(0, 2).unwrap().uri(),
        b"https://x"
    );
}

#[test]
fn state_diff_closes_previous_active_hyperlink_before_content_changes() {
    let mut prev = vt100::Parser::new(1, 10, 10);
    prev.process(b"\x1b]8;;https://x\x1b\\");

    let mut next = vt100::Parser::new(1, 10, 10);
    next.process(
        b"\x1b]8;;https://x\x1b\\\x1b]8;;\x1b\\A\x1b]8;;https://x\x1b\\",
    );

    let diff = next.screen().state_diff(prev.screen());

    let mut replay = vt100::Parser::new(1, 10, 10);
    replay.process(b"\x1b]8;;https://x\x1b\\");
    replay.process(&diff);

    assert!(next.screen().cell_hyperlink(0, 0).is_none());
    assert!(replay.screen().cell_hyperlink(0, 0).is_none());
    assert_eq!(
        replay.screen().active_hyperlink().unwrap().uri(),
        b"https://x"
    );
}
