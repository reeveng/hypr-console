use console_core_never::Never;
use console_core_urls::{DataUrl, DataUrlError, forgiving_base64};

struct Case {
  input: &'static str,
  mime: &'static str,
  body: &'static [u8],
  fragment: Option<&'static str>,
}

#[test]
fn urls_keep_their_mime_bytes_and_fragment() -> Result<(), Box<dyn std::error::Error>> {
  let cases = [
    Case {
      input: "data:,Hello%20World!",
      mime: "text/plain;charset=US-ASCII",
      body: b"Hello World!",
      fragment: None,
    },
    Case {
      input: "\0 d\na\tt\ra:;bAsE64,dGVzdA==#a b<>`é",
      mime: "text/plain;charset=US-ASCII",
      body: b"test",
      fragment: Some("a%20b%3C%3E%60%C3%A9"),
    },
    Case {
      input: "data:text/plain;charset=utf-8,a\tb\nc\rd",
      mime: "text/plain;charset=utf-8",
      body: b"abcd",
      fragment: None,
    },
    Case {
      input: "data:invalid,%+5%-5%gg%00%FF",
      mime: "text/plain;charset=US-ASCII",
      body: b"%+5%-5%gg\0\xff",
      fragment: None,
    },
    Case {
      input: "data:a/b;a=\"q;r\";x=y,hello#",
      mime: "a/b;a=\"q;r\";x=y",
      body: b"hello",
      fragment: Some(""),
    },
    Case {
      input: "data:a/b;a=\"q\\;r\";x=y,",
      mime: "a/b;a=\"q;r\";x=y",
      body: b"",
      fragment: None,
    },
    Case {
      input: "data:a/b;x=\"\",%23%25%09%0a",
      mime: "a/b;x=\"\"",
      body: b"#%\t\n",
      fragment: None,
    },
  ];

  for case in cases {
    let url = DataUrl::process(case.input)?;
    let Ok(mime) = url.mime_type();
    let (body, fragment) = url.decode_to_vec()?;
    let encoded_fragment = fragment.map(|identifier| {
      let Ok(encoded) = identifier.to_percent_encoded();

      encoded
    });

    assert_eq!(mime.to_string(), case.mime, "{}", case.input);
    assert_eq!(body, case.body, "{}", case.input);
    assert_eq!(encoded_fragment.as_deref(), case.fragment, "{}", case.input);
  }

  Ok(())
}

#[test]
fn a_fragment_cannot_supply_the_header_comma() {
  assert!(matches!(
    DataUrl::process("data:#fragment,body"),
    Err(DataUrlError::NoComma)
  ));
  assert!(matches!(
    DataUrl::process("other:,body"),
    Err(DataUrlError::NotADataUrl)
  ));
  assert!(matches!(
    DataUrl::process("data:header"),
    Err(DataUrlError::NoComma)
  ));
}

#[test]
fn base64_can_arrive_in_parts() -> Result<(), Box<dyn std::error::Error>> {
  let mut output = Vec::new();
  let target = &mut output;
  let Ok(mut decoder) = forgiving_base64::Decoder::new(move |chunk| {
    target.extend_from_slice(chunk);

    Ok::<(), Never>(())
  });

  for part in [b"dG".as_slice(), b" Vz\n", b"dA", b"=", b"=\r"] {
    decoder.feed(part)?;
  }

  decoder.finish()?;

  assert_eq!(output, b"test");

  Ok(())
}

#[test]
fn invalid_padding_and_symbols_are_rejected() {
  for invalid in [
    b"A".as_slice(),
    b"A=",
    b"AA=",
    b"AA===",
    b"AAAA=",
    b"AA==A",
    b"AA-_",
  ] {
    assert!(
      matches!(
        forgiving_base64::decode_to_vec(invalid),
        Err(_invalid_base64)
      ),
      "{invalid:?}"
    );
  }
}

#[derive(Debug)]
struct Rejected;

#[test]
fn writer_failures_are_returned_even_for_an_empty_body() -> Result<(), Box<dyn std::error::Error>>
{
  for input in ["data:,", "data:,plain", "data:;base64,dGVzdA=="] {
    let url = DataUrl::process(input)?;
    let result = url.decode(|_bytes| Err(Rejected));

    assert!(matches!(
      result,
      Err(forgiving_base64::DecodeError::WriteError(Rejected))
    ));
  }

  Ok(())
}

#[test]
fn streaming_preserves_plain_spans_and_separates_escaped_bytes()
-> Result<(), Box<dyn std::error::Error>> {
  let url = DataUrl::process("data:,abc%20def%ZZ%00\t#fragment")?;
  let mut chunks = Vec::new();
  let output = &mut chunks;
  let fragment = url.decode(move |bytes| {
    output.push(bytes.to_vec());

    Ok::<(), Never>(())
  })?;
  let encoded_fragment = fragment.map(|identifier| {
    let Ok(encoded) = identifier.to_percent_encoded();

    encoded
  });

  assert_eq!(
    chunks,
    [
      b"abc".to_vec(),
      b" ".to_vec(),
      b"def".to_vec(),
      b"%ZZ".to_vec(),
      b"\0".to_vec()
    ]
  );
  assert_eq!(encoded_fragment.as_deref(), Some("fragment"));

  Ok(())
}
