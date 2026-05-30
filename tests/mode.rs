mod helpers;

#[test]
fn modes() {
    helpers::fixture("modes");
}

#[test]
fn alternate_buffer() {
    helpers::fixture("alternate_buffer");
}

#[test]
fn parses_piebald_snapshot_modes() {
    let mut parser = vt100::Parser::new(3, 5, 0);

    assert!(!parser.screen().insert_mode());
    assert!(!parser.screen().origin_mode());
    assert!(parser.screen().wraparound_mode());
    assert!(!parser.screen().reverse_wraparound_mode());
    assert!(!parser.screen().send_focus_mode());

    parser.process(b"\x1b[4h\x1b[?6h\x1b[?7l\x1b[?45h\x1b[?1004h");

    assert!(parser.screen().insert_mode());
    assert!(parser.screen().origin_mode());
    assert!(!parser.screen().wraparound_mode());
    assert!(parser.screen().reverse_wraparound_mode());
    assert!(parser.screen().send_focus_mode());

    parser.process(b"\x1b[4l\x1b[?6l\x1b[?7h\x1b[?45l\x1b[?1004l");

    assert!(!parser.screen().insert_mode());
    assert!(!parser.screen().origin_mode());
    assert!(parser.screen().wraparound_mode());
    assert!(!parser.screen().reverse_wraparound_mode());
    assert!(!parser.screen().send_focus_mode());
}

#[test]
fn insert_mode_shifts_cells_right() {
    let mut parser = vt100::Parser::new(1, 5, 0);
    parser.process(b"abcde\x1b[1;3H\x1b[4hX");

    assert_eq!(parser.screen().contents(), "abXcd");
}

#[test]
fn wraparound_mode_defaults_on_and_can_be_disabled() {
    let mut parser = vt100::Parser::new(2, 3, 0);
    parser.process(b"abc");

    assert_eq!(parser.screen().cursor_position(), (0, 3));

    parser.process(b"d");
    assert_eq!(parser.screen().contents(), "abcd");
    assert!(parser.screen().row_wrapped(0));

    let mut parser = vt100::Parser::new(2, 3, 0);
    parser.process(b"\x1b[?7labcd");

    assert_eq!(parser.screen().contents(), "abd");
    assert_eq!(parser.screen().cursor_position(), (0, 3));
}

#[test]
fn reverse_wraparound_moves_backspace_to_previous_line() {
    let mut parser = vt100::Parser::new(2, 3, 0);
    parser.process(b"\x1b[2;1H\x1b[?45h\x08");

    assert_eq!(parser.screen().cursor_position(), (0, 2));
}
