mod helpers;

#[test]
fn ascii() {
    helpers::fixture("ascii");
}

#[test]
fn utf8() {
    helpers::fixture("utf8");
}

#[test]
fn newlines() {
    helpers::fixture("newlines");
}

#[test]
fn wide() {
    helpers::fixture("wide");
}

#[test]
fn combining() {
    helpers::fixture("combining");
}

#[test]
fn wrap() {
    helpers::fixture("wrap");
}

#[test]
fn wrap_weird() {
    helpers::fixture("wrap_weird");
}

#[test]
fn combining_cell_retains_legacy_payload_capacity() {
    let mut parser = vt100::Parser::new(3, 80, 0);
    parser.process(
        "e\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}"
            .as_bytes(),
    );

    assert_eq!(
        parser.screen().cell(0, 0).unwrap().contents(),
        "e\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}\u{0301}",
    );
}
