// run-rustfix
// edition:2024
// compile-flags: --test

#[derive(Debug, PartialEq)]
struct ReadError;

struct Font;
impl Font {
    fn table(&self) -> Result<i32, ReadError> { Ok(5) }
}
fn read() -> Result<Font, ReadError> { Ok(Font) }

#[test]
fn font_tables() -> Result<(), ReadError> {
    let mut before = Vec::new();
    assert!({ before.push(1); true } && read().unwrap().table().unwrap() > 0);
    assert_ne!(read().unwrap().table().unwrap(), { before.push(2); 6 }, "table mismatch");
    debug_assert_eq!(read().unwrap().table().unwrap().abs(), 5);
    assert_eq!(before, vec![1, 2]);
    Ok(())
}

#[test]
fn optional_glyphs() {
    let glyphs = [".notdef", "space"];
    let mut names = glyphs.iter();
    assert!(names.next().expect("missing first glyph").starts_with('.'));
    assert_eq!(names.next().unwrap().len(), "space".len());
}

#[test]
fn buffered_sources() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = [0x61];
    assert!(std::str::from_utf8(&bytes).unwrap().starts_with('a'));
    assert_ne!(std::str::from_utf8(&bytes).expect("UTF-8 glyph name").len(), 0);
    assert_eq!(bytes.first().expect("buffer was empty").count_ones(), 3);
    Ok(())
}

#[test]
fn guard_lives_through_assertion() -> Result<(), ReadError> {
    struct Reader;
    impl Reader { fn read(&self) -> Result<i32, ReadError> { Ok(5) } }
    let reader = std::cell::RefCell::new(Reader);
    assert_eq!(reader.borrow().read().unwrap(), {
        assert!(reader.try_borrow_mut().is_err());
        5
    });
    Ok(())
}
