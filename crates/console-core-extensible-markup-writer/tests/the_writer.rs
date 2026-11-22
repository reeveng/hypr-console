use console_core_extensible_markup_writer::{
  AttributeQuote, Fault, Options, Whitespace, XmlWriter,
};
use std::fmt;

#[test]
fn an_oversized_capacity_hint_still_opens_a_writer() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::with_capacity(u64::MAX, Options::default());

  writer.start_element("empty")?;
  let document = writer.end_document()?;

  assert_eq!(document, "<empty/>\n");

  Ok(())
}

#[test]
fn nesting_attributes_and_text_keep_their_layout() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::new(Options::default());

  writer.start_element("svg")?;
  writer.write_attribute("width", &5)?;
  writer.start_element("text")?;
  writer.write_text("<&")?;

  let document = writer.end_document()?;

  assert_eq!(
    document,
    "<svg width=\"5\">\n    <text>\n        &lt;&\n    </text>\n</svg>\n"
  );

  Ok(())
}

#[test]
fn single_quotes_and_whitespace_have_named_modes() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::new(Options {
    attribute_quote: AttributeQuote::Single,
    ..Options::default()
  });

  writer.start_element("svg")?;
  writer.write_attribute("title", "a'b\"c")?;
  writer.start_element("text")?;
  writer.set_preserve_whitespaces(Whitespace::Preserve)?;
  writer.write_text("x<y")?;
  writer.end_element()?;
  writer.set_preserve_whitespaces(Whitespace::Indent)?;

  let document = writer.end_document()?;

  assert_eq!(
    document,
    "<svg title='a&apos;b\"c'>\n    <text>x&lt;y</text>\n</svg>\n"
  );

  Ok(())
}

#[test]
fn an_invalid_call_order_is_a_fault() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::new(Options::default());

  assert!(matches!(
    writer.write_attribute("x", "value"),
    Err(Fault::AttributeOutsideElement)
  ));
  assert!(matches!(
    writer.write_text("value"),
    Err(Fault::TextOutsideElement)
  ));

  writer.write_declaration()?;

  assert!(matches!(
    writer.write_declaration(),
    Err(Fault::DeclarationAlreadyWritten)
  ));

  Ok(())
}

struct Unformattable;

impl fmt::Display for Unformattable {
  fn fmt(&self, _to: &mut fmt::Formatter<'_>) -> fmt::Result {
    Err(fmt::Error)
  }
}

#[test]
fn a_display_failure_is_returned() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::new(Options::default());

  writer.start_element("svg")?;

  assert!(matches!(
    writer.write_attribute("value", &Unformattable),
    Err(Fault::Formatting(_fault))
  ));

  Ok(())
}

#[test]
fn invalid_raw_utf8_is_returned() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::new(Options::default());

  writer.start_element("svg")?;
  writer.write_attribute_raw("value", |buffer| buffer.push(0xff))?;

  assert!(matches!(
    writer.end_document(),
    Err(Fault::InvalidText(_fault))
  ));

  Ok(())
}

#[test]
fn removing_the_raw_attribute_prefix_is_a_fault() -> Result<(), Fault> {
  let Ok(mut writer) = XmlWriter::new(Options::default());

  writer.start_element("svg")?;

  assert!(matches!(
    writer.write_attribute_raw("value", Vec::clear),
    Err(Fault::InvalidElementName)
  ));

  Ok(())
}
