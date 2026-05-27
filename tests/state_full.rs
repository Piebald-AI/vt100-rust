mod helpers;

fn full_line_count(screen: &vt100::Screen) -> u16 {
    let contents = screen.contents_full();
    if contents.is_empty() {
        screen.size().0
    } else {
        contents.matches('\n').count().try_into().unwrap_or(0) + 1
    }
}

#[test]
fn state_formatted_full_restores_text_and_formatting() {
    let mut parser = vt100::Parser::new(3, 80, 100);
    parser.process(
        b"\x1b[31mred\r\n\x1b[32;1mgreen bold\r\n\x1b[44mblue bg\r\n\x1b[mplain",
    );

    let state = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(full_line_count(parser.screen()), 80, 0);
    replay.process(&state);

    assert_eq!(replay.screen().contents(), parser.screen().contents_full());
    assert_eq!(
        replay.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
    assert_eq!(
        replay.screen().cell(1, 0).unwrap().fgcolor(),
        vt100::Color::Idx(2)
    );
    assert!(replay.screen().cell(1, 0).unwrap().bold());
    assert_eq!(
        replay.screen().cell(2, 0).unwrap().bgcolor(),
        vt100::Color::Idx(4)
    );
}

#[test]
fn state_formatted_full_restores_cursor_position_with_scrollback_offset() {
    let mut parser = vt100::Parser::new(3, 80, 100);
    parser.process(b"1\r\n2\r\n3\r\n4\r\n5\x1b[2;6H");
    let original_cursor = parser.screen().cursor_position();

    let state = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(5, 80, 0);
    replay.process(&state);

    assert_eq!(original_cursor, (1, 5));
    assert_eq!(replay.screen().cursor_position(), (3, 5));
}

#[test]
fn state_formatted_full_restores_cursor_visibility() {
    let mut parser = vt100::Parser::new(3, 80, 100);
    parser.process(b"hello\x1b[?25l");

    let state = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(3, 80, 0);
    replay.process(&state);

    assert!(parser.screen().hide_cursor());
    assert!(replay.screen().hide_cursor());
}

#[test]
fn state_formatted_full_restores_input_modes() {
    let mut parser = vt100::Parser::new(3, 80, 100);
    parser.process(b"\x1b=\x1b[?1h\x1b[?2004h\x1b[?1002h\x1b[?1006h");

    let state = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(3, 80, 0);
    replay.process(&state);

    assert!(replay.screen().application_keypad());
    assert!(replay.screen().application_cursor());
    assert!(replay.screen().bracketed_paste());
    assert_eq!(
        replay.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::ButtonMotion
    );
    assert_eq!(
        replay.screen().mouse_protocol_encoding(),
        vt100::MouseProtocolEncoding::Sgr
    );
}

#[test]
fn state_formatted_full_does_not_emit_terminal_queries() {
    let mut parser = vt100::Parser::new(3, 80, 100);
    parser.process(b"\x1b[31mhello\r\nworld\x1b[?25l\x1b[?2004h");

    let state = parser.screen().state_formatted_full();

    assert!(!state.windows(b"\x1b]11;?".len()).any(|w| w == b"\x1b]11;?"));
    assert!(!state.windows(b"\x1b[6n".len()).any(|w| w == b"\x1b[6n"));
    assert!(!state.windows(b"\x1b[c".len()).any(|w| w == b"\x1b[c"));
    assert!(!state.windows(b"\x1b[0c".len()).any(|w| w == b"\x1b[0c"));
    assert!(!state.windows(b"\x1b[>c".len()).any(|w| w == b"\x1b[>c"));
}

#[test]
fn contents_formatted_full_remains_history_only() {
    let mut parser = vt100::Parser::new(3, 80, 100);
    parser.process(b"1\r\n2\r\n3\r\n4\r\n5\x1b[2;6H\x1b[?25l\x1b[?2004h");

    let formatted = parser.screen().contents_formatted_full();
    let mut replay = vt100::Parser::new(5, 80, 0);
    replay.process(&formatted);

    assert_eq!(replay.screen().contents(), parser.screen().contents_full());
    assert_ne!(replay.screen().cursor_position(), (3, 5));
    assert!(!replay.screen().hide_cursor());
    assert!(!replay.screen().bracketed_paste());
}
