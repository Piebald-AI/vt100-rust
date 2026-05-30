mod helpers;

#[test]
fn colors() {
    helpers::fixture("colors");
}

#[test]
fn attrs() {
    helpers::fixture("attrs");
}

#[test]
fn attributes_formatted() {
    let mut parser = vt100::Parser::default();
    assert_eq!(parser.screen().attributes_formatted(), b"\x1b[m");
    parser.process(b"\x1b[32mfoo\x1b[41mbar\x1b[33mbaz");
    assert_eq!(parser.screen().attributes_formatted(), b"\x1b[m\x1b[33;41m");
    parser.process(b"\x1b[1m\x1b[39m");
    assert_eq!(parser.screen().attributes_formatted(), b"\x1b[m\x1b[41;1m");
    parser.process(b"\x1b[m");
    assert_eq!(parser.screen().attributes_formatted(), b"\x1b[m");
}

#[test]
fn rich_text_attrs_active_and_cells() {
    let mut parser = vt100::Parser::new(3, 80, 0);
    parser.process(
        b"\x1b[5mB\x1b[25m\x1b[8mI\x1b[28m\x1b[9mS\x1b[29m\x1b[53mO\x1b[55m",
    );

    assert!(parser.screen().cell(0, 0).unwrap().blink());
    assert!(parser.screen().cell(0, 1).unwrap().invisible());
    assert!(parser.screen().cell(0, 2).unwrap().strikethrough());
    assert!(parser.screen().cell(0, 3).unwrap().overline());
    assert!(!parser.screen().blink());
    assert!(!parser.screen().invisible());
    assert!(!parser.screen().strikethrough());
    assert!(!parser.screen().overline());
}

#[test]
fn underline_style_and_color_attrs() {
    let mut parser = vt100::Parser::new(3, 80, 0);
    parser.process(
        b"\x1b[4mA\x1b[24m\x1b[4:2mB\x1b[24m\x1b[4:3mC\x1b[24m\x1b[4:4mD\x1b[24m\x1b[4:5mE\x1b[24m",
    );

    assert!(parser.screen().cell(0, 0).unwrap().underline());
    assert_eq!(
        parser.screen().cell(0, 0).unwrap().underline_style(),
        vt100::UnderlineStyle::Single,
    );
    assert_eq!(
        parser.screen().cell(0, 1).unwrap().underline_style(),
        vt100::UnderlineStyle::Double,
    );
    assert_eq!(
        parser.screen().cell(0, 2).unwrap().underline_style(),
        vt100::UnderlineStyle::Curly,
    );
    assert_eq!(
        parser.screen().cell(0, 3).unwrap().underline_style(),
        vt100::UnderlineStyle::Dotted,
    );
    assert_eq!(
        parser.screen().cell(0, 4).unwrap().underline_style(),
        vt100::UnderlineStyle::Dashed,
    );

    parser.process(b"\r\n\x1b[4:3;58:5:42mX\x1b[58:2::255:128:64mY\x1b[59mZ");
    assert_eq!(
        parser.screen().cell(1, 0).unwrap().underline_color(),
        vt100::Color::Idx(42),
    );
    assert_eq!(
        parser.screen().cell(1, 1).unwrap().underline_color(),
        vt100::Color::Rgb(255, 128, 64),
    );
    assert_eq!(
        parser.screen().cell(1, 2).unwrap().underline_color(),
        vt100::Color::Default,
    );
}

#[test]
fn resetting_underline_style_clears_underline_color() {
    let mut parser = vt100::Parser::new(3, 80, 0);

    parser.process(b"\x1b[4:3;58:5:42mX\x1b[24mY\x1b[4mZ");
    assert_eq!(
        parser.screen().cell(0, 0).unwrap().underline_color(),
        vt100::Color::Idx(42)
    );
    assert_eq!(
        parser.screen().cell(0, 1).unwrap().underline_style(),
        vt100::UnderlineStyle::None
    );
    assert_eq!(
        parser.screen().cell(0, 1).unwrap().underline_color(),
        vt100::Color::Default
    );
    assert_eq!(
        parser.screen().cell(0, 2).unwrap().underline_style(),
        vt100::UnderlineStyle::Single
    );
    assert_eq!(
        parser.screen().cell(0, 2).unwrap().underline_color(),
        vt100::Color::Default
    );

    parser.process(b"\r\n\x1b[4:3;58:5:42mX\x1b[4:0mY\x1b[4mZ");
    assert_eq!(
        parser.screen().cell(1, 0).unwrap().underline_color(),
        vt100::Color::Idx(42)
    );
    assert_eq!(
        parser.screen().cell(1, 1).unwrap().underline_style(),
        vt100::UnderlineStyle::None
    );
    assert_eq!(
        parser.screen().cell(1, 1).unwrap().underline_color(),
        vt100::Color::Default
    );
    assert_eq!(
        parser.screen().cell(1, 2).unwrap().underline_style(),
        vt100::UnderlineStyle::Single
    );
    assert_eq!(
        parser.screen().cell(1, 2).unwrap().underline_color(),
        vt100::Color::Default
    );
}

#[test]
fn rich_text_attrs_reset_behavior() {
    let mut parser = vt100::Parser::new(3, 80, 0);
    parser.process(b"\x1b[1;3;4:3;5;7;8;9;53;58:5:42m");
    assert!(parser.screen().bold());
    assert!(parser.screen().italic());
    assert_eq!(
        parser.screen().underline_style(),
        vt100::UnderlineStyle::Curly
    );
    assert_eq!(parser.screen().underline_color(), vt100::Color::Idx(42));
    assert!(parser.screen().blink());
    assert!(parser.screen().inverse());
    assert!(parser.screen().invisible());
    assert!(parser.screen().strikethrough());
    assert!(parser.screen().overline());

    parser.process(b"\x1b[22;23;24;25;27;28;29;55;59m");
    assert!(!parser.screen().bold());
    assert!(!parser.screen().dim());
    assert!(!parser.screen().italic());
    assert_eq!(
        parser.screen().underline_style(),
        vt100::UnderlineStyle::None
    );
    assert_eq!(parser.screen().underline_color(), vt100::Color::Default);
    assert!(!parser.screen().blink());
    assert!(!parser.screen().inverse());
    assert!(!parser.screen().invisible());
    assert!(!parser.screen().strikethrough());
    assert!(!parser.screen().overline());

    parser.process(b"\x1b[1;3;4:5;5;7;8;9;53;58:2::1:2:3m\x1b[0m");
    assert!(!parser.screen().bold());
    assert!(!parser.screen().italic());
    assert_eq!(
        parser.screen().underline_style(),
        vt100::UnderlineStyle::None
    );
    assert_eq!(parser.screen().underline_color(), vt100::Color::Default);
    assert!(!parser.screen().blink());
    assert!(!parser.screen().inverse());
    assert!(!parser.screen().invisible());
    assert!(!parser.screen().strikethrough());
    assert!(!parser.screen().overline());
}

#[test]
fn rich_text_attrs_formatted_full_round_trip() {
    let input = b"\x1b[5mblink\x1b[25m \x1b[8minvis\x1b[28m \
                  \x1b[9mstrike\x1b[29m \x1b[53mover\x1b[55m \
                  \x1b[4:3mcurly\x1b[24m \
                  \x1b[4:4;58:2::255:128:64mdotted-color\x1b[0m \
                  \x1b[4:5;58:5:42mdashed-index";
    let mut parser = vt100::Parser::new(5, 120, 0);
    parser.process(input);

    let formatted = parser.screen().state_formatted_full();
    let mut replay = vt100::Parser::new(5, 120, 0);
    replay.process(&formatted);

    for row in 0..5 {
        for col in 0..120 {
            let expected = parser.screen().cell(row, col).unwrap();
            let got = replay.screen().cell(row, col).unwrap();
            assert_eq!(got.contents(), expected.contents());
            assert_eq!(got.fgcolor(), expected.fgcolor());
            assert_eq!(got.bgcolor(), expected.bgcolor());
            assert_eq!(got.bold(), expected.bold());
            assert_eq!(got.dim(), expected.dim());
            assert_eq!(got.italic(), expected.italic());
            assert_eq!(got.underline_style(), expected.underline_style());
            assert_eq!(got.underline_color(), expected.underline_color());
            assert_eq!(got.inverse(), expected.inverse());
            assert_eq!(got.blink(), expected.blink());
            assert_eq!(got.invisible(), expected.invisible());
            assert_eq!(got.strikethrough(), expected.strikethrough());
            assert_eq!(got.overline(), expected.overline());
        }
    }

    assert_eq!(
        replay.screen().underline_style(),
        parser.screen().underline_style()
    );
    assert_eq!(
        replay.screen().underline_color(),
        parser.screen().underline_color()
    );
    assert_eq!(replay.screen().blink(), parser.screen().blink());
    assert_eq!(replay.screen().invisible(), parser.screen().invisible());
    assert_eq!(
        replay.screen().strikethrough(),
        parser.screen().strikethrough()
    );
    assert_eq!(replay.screen().overline(), parser.screen().overline());
}
