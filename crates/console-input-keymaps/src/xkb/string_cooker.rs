use crate::xkb::code::Code;
use crate::xkb::code::display_text;
use crate::xkb::code_map::CodeMap;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::FromBytes;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use hashbrown::HashMap;
use hashbrown::hash_map::Entry;
use std::iter;
use std::ops::ControlFlow;
use std::ops::Range;

#[derive(Default)]
pub(crate) struct StringCooker {
  cache: HashMap<Interned, Interned>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum StringCookerError {
  OctalOverflow,
  UnknownSequence(u8),
  UnterminatedUnicodeEscape(CodeSlice<'static>),
  UnopenedUnicodeEscape,
  InvalidUnicodeEscapeRepresentation(CodeSlice<'static>),
  InvalidUnicodeCodepoint(u32),
}

impl std::fmt::Display for StringCookerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      StringCookerError::OctalOverflow => f.write_str("The octal literal overflows u8"),
      StringCookerError::UnknownSequence(byte) => {
        write!(f, "Unknown escape sequence {:?}", char::from(*byte))
      }
      StringCookerError::UnterminatedUnicodeEscape(written) => {
        let Ok(text) = display_text(written);
        write!(f, "Unterminated unicode escape sequence {:?}", text)
      }
      StringCookerError::UnopenedUnicodeEscape => {
        f.write_str("Unicode escape sequence doesn't start with a `{`")
      }
      StringCookerError::InvalidUnicodeEscapeRepresentation(written) => {
        let Ok(text) = display_text(written);
        write!(
          f,
          "Unicode escape sequence contains invalid representation {:?}",
          text
        )
      }
      StringCookerError::InvalidUnicodeCodepoint(point) => {
        write!(f, "Code point is not valid: U+{point:X}")
      }
    }
  }
}

impl std::error::Error for StringCookerError {}

type ReportedError = (DiagnosticKind, Spanned<StringCookerError>);

enum Piece {
  Byte(u8),
  Char(char),
  Invalid(ReportedError),
}

struct Step {
  piece: Piece,
  next: u32,
}

struct Cooked {
  at: u32,
  bytes: Vec<u8>,
  faults: Vec<ReportedError>,
}

struct Cooking<'a> {
  uncooked: &'a CodeSlice<'static>,
  lo: SpanUnit,
  length: u32,
}

impl StringCooker {
  pub(crate) fn cook(
    &mut self,
    map: &mut CodeMap,
    diagnostics: &mut DiagnosticSink,
    interner: &mut Interner,
    interned: Spanned<Interned>,
  ) -> Result<Interned, Never> {
    let string = interned.val;

    Ok(match self.cache.entry(string) {
      Entry::Occupied(known) => *known.get(),
      Entry::Vacant(first) => {
        let Ok(uncooked) = interner.get(string);
        let cooked = match ::core::primitive::bool::then_some(uncooked.contains(&b'\\'), ()) {
          ::core::option::Option::None => string,
          ::core::option::Option::Some(()) => {
            let Ok(unescaped) = unescaped(map, diagnostics, interner, interned);
            unescaped
          }
        };

        *first.insert(cooked)
      }
    })
  }
}

fn unescaped(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink,
  interner: &mut Interner,
  interned: Spanned<Interned>,
) -> Result<Interned, Never> {
  let Ok(uncooked) = interner.get(interned.val);
  let Ok(length) = fitted::<_, u32>(uncooked.len());
  let cooking = Cooking {
    uncooked,
    lo: interned.span.lo.saturating_add(1),
    length,
  };
  let Ok(walked) = cooking.walk();

  for (kind, fault) in walked.faults {
    let Ok(()) = diagnostics.push(map, kind, fault);
  }

  let Ok(code) = Code::owning(walked.bytes);
  let Ok(slice) = code.to_slice();
  interner.intern(&slice)
}

impl Cooked {
  fn with(mut self, step: Step) -> Result<Self, Never> {
    match step.piece {
      Piece::Byte(byte) => self.bytes.push(byte),
      Piece::Char(character) => {
        let mut encoded = [0; 4];
        self
          .bytes
          .extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
      }
      Piece::Invalid(fault) => self.faults.push(fault),
    }

    self.at = step.next;
    Ok(self)
  }
}

impl Cooking<'_> {
  fn walk(&self) -> Result<Cooked, Never> {
    let start = Cooked {
      at: 0,
      bytes: Vec::with_capacity(self.uncooked.len()),
      faults: Vec::new(),
    };
    let walked = self
      .uncooked
      .iter()
      .try_fold(start, |so_far, _one_byte_at_least| {
        let Ok(stepped) = self.step(so_far.at);

        match stepped {
          Some(step) => {
            let Ok(next) = so_far.with(step);
            ControlFlow::Continue(next)
          }
          None => ControlFlow::Break(so_far),
        }
      });

    Ok(match walked {
      ControlFlow::Break(cooked) | ControlFlow::Continue(cooked) => cooked,
    })
  }

  fn rest(&self, at: u32) -> Result<&[u8], Never> {
    let Ok(position) = index(at);

    Ok(match self.uncooked.get(position..) {
      Some(rest) => rest,
      None => &[],
    })
  }

  fn span(&self, of: Range<u32>) -> Result<Span, Never> {
    Ok(Span {
      lo: self.lo.saturating_add(of.start),
      hi: self.lo.saturating_add(of.end),
    })
  }

  fn written(&self, of: Range<u32>) -> Result<CodeSlice<'static>, Never> {
    let Ok(part) = self.uncooked.slice(of);
    part.to_owned()
  }

  fn fault(
    &self,
    kind: DiagnosticKind,
    error: StringCookerError,
    of: Range<u32>,
  ) -> Result<Piece, Never> {
    let Ok(span) = self.span(of);
    Ok(Piece::Invalid((kind, error.spanned2(span))))
  }

  fn step(&self, at: u32) -> Result<Option<Step>, Never> {
    let Ok(rest) = self.rest(at);

    Ok(match rest {
      [] => None,
      [b'\\', escape, after @ ..] => {
        let Ok(step) = self.escape(at.saturating_add(1), *escape, after);
        Some(step)
      }
      [byte, ..] => Some(Step {
        piece: Piece::Byte(*byte),
        next: at.saturating_add(1),
      }),
    })
  }

  fn escape(&self, at: u32, escape: u8, after: &[u8]) -> Result<Step, Never> {
    let next = at.saturating_add(1);
    let plain = |byte| Step {
      piece: Piece::Byte(byte),
      next,
    };

    Ok(match escape {
      b'\\' => plain(b'\\'),
      b'n' => plain(b'\n'),
      b't' => plain(b'\t'),
      b'r' => plain(b'\r'),
      b'b' => plain(b'\x08'),
      b'f' => plain(b'\x0c'),
      b'v' => plain(b'\x0b'),
      b'e' => plain(b'\x1b'),
      b'"' => plain(b'"'),
      b'0'..=b'7' => {
        let Ok(step) = self.octal(at, escape, after);
        step
      }
      b'u' => {
        let Ok(step) = self.unicode(at);
        step
      }
      _ => {
        let Ok(piece) = self.fault(
          DiagnosticKind::UnknownEscapeSequence,
          StringCookerError::UnknownSequence(escape),
          at..next,
        );
        Step { piece, next }
      }
    })
  }

  fn octal(&self, at: u32, first: u8, after: &[u8]) -> Result<Step, Never> {
    let more = after
      .iter()
      .copied()
      .take(2)
      .take_while(|byte| matches!(byte, b'0'..=b'7'));
    let (value, digits) =
      iter::once(first)
        .chain(more)
        .fold((0_u32, 0_u32), |(value, digits), digit| {
          (
            value
              .wrapping_mul(8)
              .wrapping_add(u32::from(digit.wrapping_sub(b'0'))),
            digits.saturating_add(1),
          )
        });
    let next = at.saturating_add(digits);
    let piece = match u8::try_from(value) {
      Ok(byte) => Piece::Byte(byte),
      Err(_past_a_byte) => {
        let Ok(fault) = self.fault(
          DiagnosticKind::OctalStringEscapeOverflow,
          StringCookerError::OctalOverflow,
          at..next,
        );
        fault
      }
    };

    Ok(Step { piece, next })
  }

  fn unicode(&self, at: u32) -> Result<Step, Never> {
    let opening = at.saturating_add(1);
    let Ok(after) = self.rest(opening);
    let found = (opening..).zip(after).find(|(_, byte)| **byte == b'}');

    Ok(match found {
      None => {
        let escape = at.saturating_sub(1)..self.length;
        let Ok(written) = self.written(escape.clone());
        let Ok(piece) = self.fault(
          DiagnosticKind::UnterminatedUnicodeEscape,
          StringCookerError::UnterminatedUnicodeEscape(written),
          escape,
        );
        Step {
          piece,
          next: self.length,
        }
      }
      Some((closing, _)) => {
        let Ok(piece) = self.braced(after, opening..closing);
        Step {
          piece,
          next: closing.saturating_add(1),
        }
      }
    })
  }

  fn braced(&self, after: &[u8], between: Range<u32>) -> Result<Piece, Never> {
    let opening = between.start;

    match after.first() {
      Some(b'{') => {}
      Some(_) | None => {
        return self.fault(
          DiagnosticKind::UnopenedUnicodeEscape,
          StringCookerError::UnopenedUnicodeEscape,
          opening..opening.saturating_add(1),
        );
      }
    }

    let digits = opening.saturating_add(1)..between.end;
    let Ok(written) = self.written(digits.clone());
    let codepoint = match u32::from_bytes_hex(&written) {
      Some(codepoint) => codepoint,
      None => {
        return self.fault(
          DiagnosticKind::InvalidUnicodeEscapeRepresentation,
          StringCookerError::InvalidUnicodeEscapeRepresentation(written),
          digits,
        );
      }
    };

    match char::from_u32(codepoint) {
      Some(character) => Ok(Piece::Char(character)),
      None => self.fault(
        DiagnosticKind::InvalidUnicodeCodepoint,
        StringCookerError::InvalidUnicodeCodepoint(codepoint),
        digits,
      ),
    }
  }
}
