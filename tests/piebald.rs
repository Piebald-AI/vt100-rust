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
