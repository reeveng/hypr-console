use console_core_epub_files::{Epub, ReadError};

#[test]
fn an_epub_provides_its_reading_order_and_chapter_bytes() -> Result<(), ReadError> {
  let epub = Epub::read(include_bytes!("a-short-book.epub"))?;

  assert_eq!(epub.publication.title.as_deref(), Some("Loomings"));
  assert_eq!(
    epub.publication.chapters,
    ["OEBPS/one.xhtml", "OEBPS/two.xhtml"]
  );

  let chapter = epub.extract("OEBPS/one.xhtml").map_err(ReadError::Zip)?;
  assert!(chapter.is_some_and(|bytes| bytes.starts_with(b"<html><body><h1>Loomings</h1>")));
  let missing = epub
    .extract("OEBPS/missing.xhtml")
    .map_err(ReadError::Zip)?;

  assert_eq!(missing, None);

  Ok(())
}
