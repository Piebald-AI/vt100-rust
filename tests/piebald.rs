#[test]
fn preserving_resize_rejects_zero_dimensions() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"unchanged");

    assert!(!parser.set_size_preserving_history(0, 10));
    assert!(!parser.set_size_preserving_history(3, 0));
    assert!(!parser.set_size_preserving_history(3, 1));
    assert_eq!(parser.screen().size(), (3, 10));
    assert_eq!(parser.screen().contents_full(), "unchanged");
}

#[test]
fn preserving_resize_keeps_rows_and_columns() {
    let mut parser = vt100::Parser::new(4, 10, 10);
    parser.process(b"OLDEST\r\nMIDDLE\r\nABCDEFGHIJ");

    assert!(parser.set_size_preserving_history(2, 5));

    assert_eq!(parser.screen().size(), (2, 5));
    assert_eq!(
        parser.screen().contents_full(),
        "OLDEST\nMIDDLE\nABCDEFGHIJ"
    );
}

#[test]
fn preserving_resize_keeps_partial_csi_state() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"\x1b[");

    assert!(parser.set_size_preserving_history(2, 5));
    parser.process(b"31mRED");

    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.contents(), "R");
    assert_eq!(cell.fgcolor(), vt100::Color::Idx(1));
}

#[test]
fn preserving_resize_keeps_partial_osc_state() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"\x1b]2;partial");

    assert!(parser.set_size_preserving_history(2, 5));
    parser.process(b" title\x1b\\");

    assert_eq!(
        parser.screen().window_title(),
        Some(b"partial title".as_slice())
    );
}

#[test]
fn preserving_resize_keeps_partial_utf8_state() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    let bytes = "界".as_bytes();
    parser.process(&bytes[..2]);

    assert!(parser.set_size_preserving_history(2, 5));
    parser.process(&bytes[2..]);

    assert_eq!(parser.screen().contents_full(), "界");
    assert!(parser.screen().cell(0, 0).unwrap().is_wide());
    assert!(parser.screen().cell(0, 1).unwrap().is_wide_continuation());
}

#[test]
fn preserving_resize_keeps_main_and_alternate_screens() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"main\x1b[?1049halt");

    assert!(parser.set_size_preserving_history(2, 5));
    assert_eq!(parser.screen().contents(), "alt");

    parser.process(b"\x1b[?1049l");
    assert_eq!(parser.screen().contents_full(), "main");
}

#[test]
fn preserving_resize_keeps_wide_cells_together() {
    let mut parser = vt100::Parser::new(3, 6, 10);
    parser.process("abc界z".as_bytes());

    assert!(parser.set_size_preserving_history(3, 4));

    assert_eq!(parser.screen().contents_full(), "abc界z");
    assert!(parser.screen().cell(1, 0).unwrap().is_wide());
    assert!(parser.screen().cell(1, 1).unwrap().is_wide_continuation());
}

#[test]
fn preserving_resize_keeps_physical_rows_when_growing() {
    let mut parser = vt100::Parser::new(4, 5, 10);
    parser.process(b"AAAAABBBBB");

    assert!(parser.set_size_preserving_history(4, 10));
    parser.process(b"\rCCCCC\r\nTAIL");

    assert_eq!(parser.screen().contents_full(), "AAAAA\nCCCCC\nTAIL");
}

#[test]
fn preserving_row_growth_keeps_wrapped_rows() {
    let mut parser = vt100::Parser::new(2, 5, 10);
    parser.process(b"abcdef");

    assert!(parser.set_size_preserving_history(4, 5));
    parser.process(b"X");

    assert_eq!(parser.screen().contents_full(), "abcdefX");
}

#[test]
fn preserving_resize_rejoins_rows_split_by_resize() {
    let mut parser = vt100::Parser::new(4, 10, 10);
    parser.process(b"abcdefghij");

    assert!(parser.set_size_preserving_history(4, 5));
    assert!(parser.set_size_preserving_history(4, 10));
    parser.process(b"X");

    assert_eq!(parser.screen().contents_full(), "abcdefghijX");
}

#[test]
fn preserving_resize_clears_pending_wrap_when_line_grows() {
    let mut parser = vt100::Parser::new(3, 5, 10);
    parser.process(b"abcde");

    assert!(parser.set_size_preserving_history(3, 10));
    parser.process(b"X");

    assert_eq!(parser.screen().contents_full(), "abcdeX");
}

#[test]
fn preserving_resize_keeps_pending_wrap_cursor() {
    let mut parser = vt100::Parser::new(4, 6, 10);
    parser.process(b"abcdef");

    assert!(parser.set_size_preserving_history(3, 3));
    parser.process(b"X");

    assert_eq!(parser.screen().contents_full(), "abcdefX");
}

#[test]
fn preserving_resize_handles_repeated_shrink_and_growth() {
    let mut parser = vt100::Parser::new(4, 10, 10);
    parser.process(b"one\r\ntwo\r\nthree\r\nfour");

    assert!(parser.set_size_preserving_history(2, 5));
    assert!(parser.set_size_preserving_history(5, 12));
    assert!(parser.set_size_preserving_history(3, 7));

    assert_eq!(parser.screen().contents_full(), "one\ntwo\nthree\nfour");
}

#[test]
fn preserving_resize_honors_scrollback_limit() {
    let mut parser = vt100::Parser::new(5, 10, 2);
    parser.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");

    assert!(parser.set_size_preserving_history(1, 10));

    assert_eq!(parser.screen().scrollback_rows_len(), 2);
    assert_eq!(parser.screen().contents_full(), "three\nfour\nfive");
}

#[test]
fn preserving_row_shrink_remaps_active_cursor() {
    let mut parser = vt100::Parser::new(4, 10, 10);
    parser.process(b"one\r\ntwo\r\nthree\r\nfour");

    assert!(parser.set_size_preserving_history(2, 10));

    assert_eq!(parser.screen().cursor_position(), (1, 4));
}

#[test]
fn preserving_row_shrink_remaps_saved_cursor() {
    let mut parser = vt100::Parser::new(4, 10, 10);
    parser.process(b"one\r\ntwo\x1b7\r\nthree\r\nfour");

    assert!(parser.set_size_preserving_history(2, 10));
    parser.process(b"\x1b8");
    assert_eq!(parser.screen().cursor_position(), (0, 3));
    parser.process(b"X");

    assert_eq!(parser.screen().contents_full(), "one\ntwo\nthrXe\nfour");
}

#[test]
fn preserving_resize_keeps_cursor_in_blank_columns() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"abc\x1b[3C");

    assert!(parser.set_size_preserving_history(3, 5));

    assert_eq!(parser.screen().cursor_position(), (1, 1));
}

#[test]
fn preserving_resize_keeps_styled_blank_cells() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"\x1b[44m   ");

    assert!(parser.set_size_preserving_history(3, 2));

    assert_eq!(
        parser.screen().cell(1, 0).unwrap().bgcolor(),
        vt100::Color::Idx(4)
    );
}

#[test]
fn preserving_resize_keeps_saved_cursor_state() {
    let mut parser = vt100::Parser::new(4, 10, 10);
    parser.process(b"first\r\nsecond\x1b7\r\nthird");

    assert!(parser.set_size_preserving_history(2, 5));
    parser.process(b"\x1b8X");

    assert_eq!(parser.screen().contents_full(), "first\nsecondX\nthird");
}

#[test]
fn runtime_scrollback_limit_trims_and_clamps() {
    let mut parser = vt100::Parser::new(3, 10, 5);
    parser.process(b"one\ntwo\nthree\nfour\nfive\nsix");
    assert_eq!(parser.screen().scrollback_len(), 5);
    assert!(parser.screen().scrollback_rows_len() <= 5);

    parser.screen_mut().set_scrollback(5);
    parser.set_scrollback_len(2);
    assert_eq!(parser.screen().scrollback_len(), 2);
    assert_eq!(parser.screen().scrollback_rows_len(), 2);
    assert_eq!(parser.screen().scrollback(), 2);

    parser.set_scrollback_len(10);
    assert_eq!(parser.screen().scrollback_len(), 10);
    assert_eq!(parser.screen().scrollback_rows_len(), 2);
}

#[test]
fn csi_3j_clears_scrollback_without_erasing_screen() {
    let mut parser = vt100::Parser::new(3, 10, 10);
    parser.process(b"one\ntwo\nthree\nfour\nfive");
    assert!(parser.screen().scrollback_rows_len() > 0);
    let visible = parser.screen().contents();

    parser.process(b"\x1b[3J");

    assert_eq!(parser.screen().scrollback_rows_len(), 0);
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(parser.screen().contents(), visible);
}

#[test]
fn dynamic_palette_osc_updates_and_resets_state() {
    let mut parser = vt100::Parser::new(3, 20, 0);
    parser.process(b"\x1b]4;1;rgb:ff/80/00\x1b\\");
    parser.process(b"\x1b]10;#eeeeee\x1b\\");
    parser.process(b"\x1b]11;rgb:1111/2222/3333\x1b\\");
    parser.process(b"\x1b]12;rgb:f/0/8\x1b\\");

    assert_eq!(
        parser.screen().palette().indexed(1),
        Some(vt100::RgbColor {
            r: 255,
            g: 128,
            b: 0
        })
    );
    assert_eq!(
        parser.screen().palette().foreground(),
        Some(vt100::RgbColor {
            r: 238,
            g: 238,
            b: 238
        })
    );
    assert_eq!(
        parser.screen().palette().background(),
        Some(vt100::RgbColor {
            r: 17,
            g: 34,
            b: 51
        })
    );
    assert_eq!(
        parser.screen().palette().cursor(),
        Some(vt100::RgbColor {
            r: 255,
            g: 0,
            b: 136
        })
    );

    parser.process(
        b"\x1b]104;1\x1b\\\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\",
    );
    assert_eq!(parser.screen().palette().indexed(1), None);
    assert_eq!(parser.screen().palette().foreground(), None);
    assert_eq!(parser.screen().palette().background(), None);
    assert_eq!(parser.screen().palette().cursor(), None);
}

#[test]
fn sgr_cells_remain_semantic_after_palette_change() {
    let mut parser = vt100::Parser::new(3, 20, 0);
    parser.process(b"\x1b]4;1;rgb:ff/80/00\x1b\\\x1b[31mred");

    assert_eq!(
        parser.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
    assert_eq!(
        parser.screen().palette().resolve_color(
            vt100::Color::Idx(1),
            vt100::ColorRole::Foreground,
        ),
        vt100::ResolvedColor::Rgb(vt100::RgbColor {
            r: 255,
            g: 128,
            b: 0
        })
    );
}

#[test]
fn persistent_replay_emits_palette_setup_before_content() {
    let mut parser = vt100::Parser::new(3, 20, 5);
    parser.process(b"\x1b]4;1;rgb:ff/80/00\x1b\\\x1b[31mred");

    let replay = parser.screen().serialize_persistent_replay(None);
    let palette_pos = replay
        .windows(b"\x1b]4;1;rgb:ff/80/00\x1b\\".len())
        .position(|window| window == b"\x1b]4;1;rgb:ff/80/00\x1b\\")
        .unwrap();
    let text_pos = replay
        .windows(b"red".len())
        .position(|window| window == b"red")
        .unwrap();
    assert!(palette_pos < text_pos);
    assert!(replay
        .windows(b"\x1b[31m".len())
        .any(|window| window == b"\x1b[31m"));
}

#[test]
fn frozen_snapshot_resolves_palette_colors_to_truecolor() {
    let mut parser = vt100::Parser::new(3, 20, 0);
    parser.process(b"\x1b]4;1;rgb:ff/80/00\x1b\\\x1b[31mred");

    let snapshot = parser.screen().serialize_frozen_snapshot();

    assert!(snapshot
        .windows(b"\x1b[38;2;255;128;0m".len())
        .any(|window| { window == b"\x1b[38;2;255;128;0m" }));
    assert!(!snapshot
        .windows(b"\x1b]4;".len())
        .any(|window| window == b"\x1b]4;"));
}

#[test]
fn frozen_snapshot_resolves_default_foreground_and_background() {
    let mut parser = vt100::Parser::new(3, 20, 0);
    parser.process(
        b"\x1b]10;rgb:01/02/03\x1b\\\x1b]11;rgb:04/05/06\x1b\\plain",
    );

    let snapshot = parser.screen().serialize_frozen_snapshot();

    assert!(snapshot
        .windows(b"38;2;1;2;3".len())
        .any(|window| window == b"38;2;1;2;3"));
    assert!(snapshot
        .windows(b"48;2;4;5;6".len())
        .any(|window| window == b"48;2;4;5;6"));
}

#[test]
fn persistent_replay_uses_main_buffer_when_alternate_screen_is_active() {
    let mut parser = vt100::Parser::new(3, 20, 10);
    parser.process(b"main-line\n");
    parser.process(b"\x1b[?1049halt-line");

    let replay = parser.screen().serialize_persistent_replay(None);

    assert!(replay
        .windows(b"main-line".len())
        .any(|window| window == b"main-line"));
    assert!(!replay
        .windows(b"alt-line".len())
        .any(|window| window == b"alt-line"));
    assert!(!replay
        .windows(b"\x1b[?1049h".len())
        .any(|window| window == b"\x1b[?1049h"));
}

#[test]
fn persistent_replay_handles_huge_scrollback_without_u16_offset() {
    let mut parser = vt100::Parser::new(1, 20, 70_000);
    for i in 0..70_010_u32 {
        parser.process(format!("{i}\n").as_bytes());
    }

    let replay = parser.screen().serialize_persistent_replay(None);

    assert!(replay
        .windows(b"70009".len())
        .any(|window| window == b"70009"));
}

#[test]
fn persistent_replay_preserves_wrapped_rows() {
    let mut parser = vt100::Parser::new(2, 5, 10);
    parser.process(b"abcdefghij");

    let replay = parser.screen().serialize_persistent_replay(None);
    assert!(replay
        .windows(b"abcdefghij".len())
        .any(|window| window == b"abcdefghij"));
}
