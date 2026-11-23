use console_core_postscript_tokens::{
  Array, NameKind, Number, Object, Remaining, ScanError, Scanner, decoding,
};
use std::collections::VecDeque;

type Failure = Box<dyn std::error::Error>;

fn names(program: &[u8]) -> Result<Vec<(NameKind, Vec<u8>)>, ScanError> {
  let Ok(mut scanner) = Scanner::new(program);
  let mut found = Vec::new();

  for _object in program {
    match scanner.remaining() {
      Ok(Remaining::Absent) => return Ok(found),
      Ok(Remaining::Objects) => {}
    }

    let name = scanner.parse_name()?;
    let Ok(kind) = name.kind();
    let decoded = name.decode()?;

    found.push((kind, decoded));
  }

  Ok(found)
}

fn array(object: Object<'_>) -> Result<Array<'_>, ScanError> {
  match object {
    Object::Array(found) => Ok(found),
    Object::Number(_) | Object::Name(_) | Object::String(_) => Err(ScanError::TypeCheck),
  }
}

fn number(program: &[u8]) -> Result<Number, ScanError> {
  let Ok(mut scanner) = Scanner::new(program);

  scanner.parse_number()
}

fn string(program: &[u8]) -> Result<Vec<u8>, ScanError> {
  let Ok(mut scanner) = Scanner::new(program);
  let found = scanner.parse_string()?;

  found.decode()
}

#[test]
fn a_cmap_is_read_as_the_objects_it_is_written_in() -> Result<(), Failure> {
  let program = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CMapName /Test-H def\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\nendcmap";
  let Ok(mut scanner) = Scanner::new(program.as_slice());
  let mut said = Vec::new();

  for _object in program {
    match scanner.remaining() {
      Ok(Remaining::Absent) => break,
      Ok(Remaining::Objects) => {}
    }

    let object = scanner.parse_object()?;
    let word = match object {
      Object::Number(found) => format!("{found:?}"),
      Object::Name(found) => {
        let text = found.as_str()?;

        format!("{:?} {text}", found.kind())
      }
      Object::String(found) => {
        let bytes = found.decode()?;

        format!("<{bytes:02X?}>")
      }
      Object::Array(_found) => String::from("[]"),
    };

    said.push(word);
  }

  assert_eq!(
    said,
    [
      "Ok(Literal) CIDInit",
      "Ok(Literal) ProcSet",
      "Ok(Executable) findresource",
      "Ok(Executable) begin",
      "Integer(12)",
      "Ok(Executable) dict",
      "Ok(Executable) begin",
      "Ok(Executable) begincmap",
      "Ok(Literal) CMapName",
      "Ok(Literal) Test-H",
      "Ok(Executable) def",
      "Integer(1)",
      "Ok(Executable) begincodespacerange",
      "<[00]>",
      "<[FF]>",
      "Ok(Executable) endcodespacerange",
      "Ok(Executable) endcmap",
    ]
  );

  Ok(())
}

#[test]
fn an_array_is_scanned_the_way_its_program_was() -> Result<(), Failure> {
  let Ok(mut scanner) = Scanner::new(b"[123 /abc (x]z) [<41>] % a ]\n 4] 5");
  let object = scanner.parse_object()?;
  let outer = array(object)?;
  let Ok(mut inside) = outer.objects();

  assert_eq!(inside.parse_number(), Ok(Number::Integer(123)));

  let name = inside.parse_name()?;
  let text = inside.parse_string()?;

  assert_eq!(name.as_str(), Ok("abc"));
  assert_eq!(text.decode(), Ok(b"x]z".to_vec()));
  assert!(matches!(inside.parse_object(), Ok(Object::Array(_))));
  assert_eq!(inside.parse_number(), Ok(Number::Integer(4)));
  assert_eq!(inside.remaining(), Ok(Remaining::Absent));
  assert_eq!(scanner.parse_number(), Ok(Number::Integer(5)));

  Ok(())
}

#[test]
fn comments_and_dictionary_marks_are_stepped_over() -> Result<(), Failure> {
  let Ok(mut scanner) = Scanner::new(b"% a comment\n<< 42 % another\n>> /Name");

  assert_eq!(scanner.parse_number(), Ok(Number::Integer(42)));

  let name = scanner.parse_name()?;

  assert_eq!(name.as_str(), Ok("Name"));
  assert_eq!(scanner.remaining(), Ok(Remaining::Absent));

  Ok(())
}

#[test]
fn a_program_of_nested_dictionaries_ends_rather_than_running_the_stack_out() -> Result<(), Failure>
{
  let mut program = b"<<".repeat(200_000);

  program.extend_from_slice(b" 7");

  assert_eq!(number(&program), Ok(Number::Integer(7)));
  assert_eq!(number(b"<<<<>>"), Err(ScanError::Ended));

  Ok(())
}

#[test]
fn an_object_of_another_kind_is_a_typecheck() -> Result<(), Failure> {
  let Ok(mut scanner) = Scanner::new(b"42 ");

  assert_eq!(scanner.parse_name(), Err(ScanError::TypeCheck));
  assert_eq!(number(b"   % nothing\n"), Err(ScanError::Ended));
  assert_eq!(number(b"{ 1 }"), Err(ScanError::Unsupported));
  assert_eq!(number(b"]"), Err(ScanError::SyntaxError));

  Ok(())
}

#[test]
fn numbers_are_integers_reals_and_integers_in_a_base() -> Result<(), Failure> {
  assert_eq!(number(b"123 "), Ok(Number::Integer(123)));
  assert_eq!(number(b"-98"), Ok(Number::Integer(-98)));
  assert_eq!(number(b"+17/"), Ok(Number::Integer(17)));
  assert_eq!(number(b"-.002"), Ok(Number::Real(-0.002)));
  assert_eq!(number(b"34.5"), Ok(Number::Real(34.5)));
  assert_eq!(number(b"123.6e10"), Ok(Number::Real(123.6e10)));
  assert_eq!(number(b"1.0E-5"), Ok(Number::Real(1.0E-5)));
  assert_eq!(number(b"1E6"), Ok(Number::Real(1E6)));
  assert_eq!(number(b"-1."), Ok(Number::Real(-1.0)));
  assert_eq!(number(b"8#1777"), Ok(Number::Integer(0o1777)));
  assert_eq!(number(b"16#FFFE"), Ok(Number::Integer(0xFFFE)));
  assert_eq!(number(b"2#1000"), Ok(Number::Integer(0b1000)));
  assert_eq!(Number::Real(-2.9).as_i32(), Ok(-2));

  Ok(())
}

#[test]
fn a_token_that_starts_as_a_number_and_is_not_one_is_a_fault() -> Result<(), Failure> {
  assert_eq!(number(b"1a"), Err(ScanError::SyntaxError));
  assert_eq!(number(b"+abc"), Err(ScanError::SyntaxError));
  assert_eq!(number(b".notdef"), Err(ScanError::SyntaxError));
  assert_eq!(number(b"37#1"), Err(ScanError::SyntaxError));
  assert_eq!(number(b"-8#7"), Err(ScanError::SyntaxError));
  assert_eq!(number(b"2147483648"), Err(ScanError::SyntaxError));

  Ok(())
}

#[test]
fn a_name_is_literal_or_executable_and_its_escapes_are_bytes() -> Result<(), Failure> {
  assert_eq!(
    names(b"/Name1 beginbfchar def/name /"),
    Ok(vec![
      (NameKind::Literal, b"Name1".to_vec()),
      (NameKind::Executable, b"beginbfchar".to_vec()),
      (NameKind::Executable, b"def".to_vec()),
      (NameKind::Literal, b"name".to_vec()),
      (NameKind::Literal, Vec::new()),
    ])
  );
  assert_eq!(
    names(b"/lime#20Green /paired#28#29parentheses"),
    Ok(vec![
      (NameKind::Literal, b"lime Green".to_vec()),
      (NameKind::Literal, b"paired()parentheses".to_vec()),
    ])
  );
  assert_eq!(
    names(b"/A;Name_With-Various***Characters?"),
    Ok(vec![(
      NameKind::Literal,
      b"A;Name_With-Various***Characters?".to_vec()
    )])
  );
  assert_eq!(names(b"/broken#4"), Err(ScanError::SyntaxError));
  assert_eq!(names(b"/broken#4G"), Err(ScanError::SyntaxError));

  Ok(())
}

#[test]
fn a_literal_string_says_its_bytes_and_escapes_the_rest() -> Result<(), Failure> {
  assert_eq!(string(b"()"), Ok(Vec::new()));
  assert_eq!(string(b"(Hi (()) there)"), Ok(b"Hi (()) there".to_vec()));
  assert_eq!(
    string(b"(\\n\\r\\t\\b\\f\\\\\\(\\)\\q)"),
    Ok(b"\n\r\t\x08\x0c\\()q".to_vec())
  );
  assert_eq!(string(b"(\\053\\36\\3x)"), Ok(b"+\x1e\x03x".to_vec()));
  assert_eq!(string(b"(\\0539)"), Ok(b"+9".to_vec()));
  assert_eq!(string(b"(\\400.)"), Ok(b".".to_vec()));
  assert_eq!(string(b"(Hi \\\r\nthere)"), Ok(b"Hi there".to_vec()));
  assert_eq!(string(b"(a\rb\r\nc\n\nd)"), Ok(b"a\nb\nc\nd".to_vec()));
  assert_eq!(string(b"(never closed"), Err(ScanError::SyntaxError));
  assert_eq!(
    string(b"(ends on a backslash\\"),
    Err(ScanError::SyntaxError)
  );

  Ok(())
}

#[test]
fn a_hexadecimal_string_is_two_digits_a_byte() -> Result<(), Failure> {
  assert_eq!(string(b"<48656C6C6F>"), Ok(b"Hello".to_vec()));
  assert_eq!(string(b"<48 65\n6c 6C 6f>"), Ok(b"Hello".to_vec()));
  assert_eq!(string(b"<ABC>"), Ok(vec![0xAB, 0xC0]));
  assert_eq!(string(b"<>"), Ok(Vec::new()));
  assert_eq!(string(b"<4G>"), Err(ScanError::SyntaxError));
  assert_eq!(string(b"<41"), Err(ScanError::SyntaxError));

  Ok(())
}

#[test]
fn an_ascii85_string_is_five_digits_for_four_bytes() -> Result<(), Failure> {
  assert_eq!(string(b"<~87cURDZ~>"), Ok(b"Hello".to_vec()));
  assert_eq!(string(b"<~87cU RDZ~>"), Ok(b"Hello".to_vec()));
  assert_eq!(string(b"<~~>"), Ok(Vec::new()));
  assert_eq!(string(b"<~z~>"), Ok(vec![0, 0, 0, 0]));
  assert_eq!(string(b"<~87~>").map(|bytes| bytes.len()), Ok(1));
  assert_eq!(string(b"<~8~>"), Err(ScanError::SyntaxError));
  assert_eq!(string(b"<~uuuuu~>"), Err(ScanError::SyntaxError));
  assert_eq!(string(b"<~87~c~>"), Err(ScanError::SyntaxError));

  Ok(())
}

#[test]
fn a_stream_is_decoded_into_whatever_its_reader_keeps_bytes_in() -> Result<(), Failure> {
  let mut stream = VecDeque::new();
  let mut text = Vec::new();

  decoding::ascii85(b"87cU RDZ~>", &mut stream)?;
  decoding::hexadecimal(b"48 69", &mut text)?;

  assert_eq!(stream, b"Hello");
  assert_eq!(text, b"Hi");

  Ok(())
}
