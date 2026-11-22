/*!
PDF content operators.

This module provides facilities to read and interpret PDF content streams using
high-level types.

```
use console_core_portable_document_syntax::object::Number;
use console_core_portable_document_syntax::content::*;
use console_core_portable_document_syntax::content::ops::*;

let content_stream = b"1 0 0 -1 0 200 cm
0 1.0 0 rg
0 0 m
200 0 l
200 200 l
0 200 l
h
f";

let Ok(mut iter) = TypedIter::new(content_stream);
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::Transform(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::NonStrokeColorDeviceRgb(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::MoveTo(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::LineTo(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::LineTo(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::LineTo(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::ClosePath(_)))));
assert!(matches!(iter.next(), Ok(Some(TypedInstruction::FillPathNonZero(_)))));
```
*/
use alloc::vec::Vec;

#[expect(
  missing_docs,
  reason = "each operator is named for the PDF content operator it reads, and the specification is where those are documented"
)]
pub mod ops;

use crate::ObjectReadMode;
use crate::content::ops::TypedInstruction;
use crate::object;
use crate::object::dict::InlineImageDict;
use crate::object::name::{Name, NameSyntax, skip_name_like};
use crate::object::{Array, Null, Number, Object, Stream};
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use crate::trivia::is_white_space_character;
use crate::util::find_needle;
use console_core_never::Never;
use console_core_number_conversion::index;
use core::array;
use core::fmt::{Debug, Formatter};
use core::ops::Deref;

impl Debug for Operator<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(
      f,
      "{}",
      ({
        let Ok(explicit_value) = self.0.as_str();
        explicit_value
      })
    )
  }
}

#[derive(Clone, PartialEq)]
pub struct Operator<'a>(Name<'a>);

impl Deref for Operator<'_> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.0.as_ref()
  }
}

impl Skippable for Operator<'_> {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    ({
      let Ok(explicit_value) = skip_name_like(r, NameSyntax::Bare);
      explicit_value
    })
    .map(|_| ())
  }
}

impl<'a> Readable<'a> for Operator<'a> {
  fn read(reader: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
    let Ok(start) = reader.fixed_offset();
    let Ok(skipped) = skip_name_like(reader, NameSyntax::Bare);
    skipped?;
    let Ok(end) = reader.fixed_offset();
    let Ok(range_start) = index(start);
    let Ok(range_end) = index(end);
    let Ok(range) = reader.range(range_start..range_end);
    let data = range?;

    match data {
      [] => return None,
      [_, ..] => {}
    }

    let Ok(name) = Name::new(data);
    name.map(Self)
  }
}

const MAX_EI_CANDIDATES: u32 = 256;
const MAX_EI_LOOKAHEAD_STEPS: u32 = 256;

#[derive(Clone)]
enum ContentRead {
  Normal,
  InlineImageLookahead,
}

enum InlineImageBoundary {
  End,
  Begin,
}

enum Lookahead {
  Plausible,
  Rejected,
}

enum InlineImageRead {
  Complete,
  MissingBoundary,
  Invalid,
}

#[derive(Clone)]
pub struct UntypedIter<'a> {
  reader: Reader<'a>,
  stack: Stack<'a>,
  operator: Option<Operator<'a>>,

  read_mode: ContentRead,
}

impl<'a> UntypedIter<'a> {
  pub fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(reader) = Reader::new(data);
    let Ok(stack) = Stack::new();

    Ok(Self {
      reader,
      stack,
      operator: None,
      read_mode: ContentRead::Normal,
    })
  }

  pub fn empty() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(reader) = Reader::new(&[]);
    let Ok(stack) = Stack::new();

    Ok(Self {
      reader,
      stack,
      operator: None,
      read_mode: ContentRead::Normal,
    })
  }

  #[allow(
    clippy::should_implement_trait,
    reason = "next answers a Result as every infallible function here does, which is not the signature Iterator asks for"
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Only normal content scanning enables typed inline-image lookahead"
    )
  )]
  pub fn next(
    &mut self,
  ) -> core::result::Result<Option<Instruction<'_, 'a>>, console_core_never::Never> {
    let Ok(()) = self.stack.clear();
    self.operator = None;
    self.reader.skip_white_spaces_and_comments();
    let Ok(unread) = self.reader.tail();
    let budget = match unread {
      Some(bytes) => bytes,
      None => return Ok(None),
    };

    for _source_byte in budget {
      let Ok(peeked) = self.reader.peek_byte();

      match peeked {
        None => break,
        Some(b'/' | b'.' | b'+' | b'-' | b'0'..=b'9' | b'[' | b'<' | b'(') => {
          let Ok(operand) = self.read_operand();

          match operand {
            OperandRead::Consumed => {}
            OperandRead::Invalid => return Ok(None),
          }
        }
        Some(_operator) => {
          let Ok(scanned) = self.read_operator();
          let operator = match scanned {
            Some(operator) => operator,
            None => return Ok(None),
          };

          self.operator = Some(operator);
          return Ok(self.operator.as_ref().map(|operator_2| Instruction {
            operands: &self.stack,
            operator: operator_2,
          }));
        }
      }

      self.reader.skip_white_spaces_and_comments();
    }

    Ok(None)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "An inline operator starts speculative parsing only outside lookahead mode"
    )
  )]
  fn read_operator(&mut self) -> Result<Option<Operator<'a>>, Never> {
    let operator = match self.reader.read_without_context::<Operator<'_>>() {
      Some(operator) => operator,
      None => {
        warn!("failed to read operator in content stream");
        let Ok(()) = self.reader.jump_to_end();
        return Ok(None);
      }
    };

    match operator.as_ref() {
      b"BI" => {
        let Ok(image) = self.read_inline_image();

        match image {
          InlineImageRead::Complete | InlineImageRead::MissingBoundary => {}
          InlineImageRead::Invalid => return Ok(None),
        }
      }
      _other_operator => {}
    }

    Ok(Some(operator))
  }

  fn read_operand(&mut self) -> Result<OperandRead, Never> {
    match self.reader.read_without_context::<Object<'_>>() {
      Some(operand) => {
        let Ok(pushed) = self.stack.push(operand);

        Ok(match pushed {
          Some(()) => OperandRead::Consumed,
          None => OperandRead::Invalid,
        })
      }
      None => match self.reader.read_without_context::<Operator<'_>>() {
        Some(_discarded) => {
          let Ok(()) = self.stack.clear();
          Ok(OperandRead::Consumed)
        }
        None => Ok(OperandRead::Invalid),
      },
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Only normal content starts the inline-image lookahead, and it parses with lookahead disabled"
    )
  )]
  fn read_inline_image(&mut self) -> Result<InlineImageRead, Never> {
    let inline_dict = match self.reader.read_without_context::<InlineImageDict<'_>>() {
      Some(dict) => dict,
      None => return Ok(InlineImageRead::Invalid),
    };
    let Ok(dict) = inline_dict.get_dict();

    match self.reader.read_white_space() {
      Some(_) => {}
      None => return Ok(InlineImageRead::Invalid),
    }

    let Ok(unread) = self.reader.tail();
    let stream_data = match unread {
      Some(data) => data,
      None => return Ok(InlineImageRead::Invalid),
    };
    let Ok(start) = self.reader.fixed_offset();
    let mut candidates = 0_u32;

    for _image_byte in stream_data {
      let Ok(remaining) = self.reader.tail();
      let tail = match remaining {
        Some(tail) => tail,
        None => return Ok(InlineImageRead::Invalid),
      };
      let Ok(next) = find_needle(tail, b"EI");
      let marker = match next {
        Some(marker) => marker,
        None => break,
      };
      let Ok(position) = index(marker);
      let Ok(advanced) = self.reader.skip_bytes(position);

      match advanced {
        Some(()) => {}
        None => return Ok(InlineImageRead::Invalid),
      }

      let Ok(next_three) = self.reader.peek_bytes(3);
      let delimiter = match next_three {
        Some([_, _, third]) => {
          let Ok(whitespace) = is_white_space_character(*third);
          whitespace
        }
        Some(_) | None => crate::IsWhiteSpaceCharacterAnswer::Yes,
      };

      match delimiter {
        crate::IsWhiteSpaceCharacterAnswer::No => {
          let Ok(skipped) = self.reader.skip_bytes(3);

          match skipped {
            Some(()) => continue,
            None => return Ok(InlineImageRead::Invalid),
          }
        }
        crate::IsWhiteSpaceCharacterAnswer::Yes => {}
      }

      candidates = candidates.saturating_add(1);
      let Ok(candidate) = self.reader.tail();
      let rest = match candidate.and_then(|tail_2| tail_2.get(2..)) {
        Some(tail_3) => tail_3,
        None => return Ok(InlineImageRead::Invalid),
      };

      let Ok(lookahead) = self.check_candidate(rest, candidates);

      match lookahead {
        Lookahead::Plausible => {}
        Lookahead::Rejected => {
          let Ok(skipped) = self.reader.skip_bytes(2);

          match skipped {
            Some(()) => continue,
            None => return Ok(InlineImageRead::Invalid),
          }
        }
      }

      let Ok(end) = self.reader.fixed_offset();
      let Ok(data_end) = index(end.saturating_sub(start));
      let data = match stream_data.get(..data_end) {
        Some(data) => data,
        None => return Ok(InlineImageRead::Invalid),
      };
      let Ok(stream) = Stream::new(data, dict.clone());
      let Ok(pushed) = self.stack.push(Object::Stream(stream));

      match pushed {
        Some(()) => {}
        None => return Ok(InlineImageRead::Invalid),
      }

      let Ok(terminator) = self.reader.skip_bytes(2);

      match terminator {
        Some(()) => {}
        None => return Ok(InlineImageRead::Invalid),
      }

      self.reader.skip_white_spaces();
      return Ok(InlineImageRead::Complete);
    }

    Ok(InlineImageRead::MissingBoundary)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The bounded probe is entered only from normal mode and disables further probes"
    )
  )]
  fn check_candidate(&self, tail: &[u8], candidates: u32) -> Result<Lookahead, Never> {
    match (&self.read_mode, candidates) {
      (ContentRead::Normal, 1..=MAX_EI_CANDIDATES) => assess_image_tail(tail),
      (ContentRead::InlineImageLookahead, _) | (ContentRead::Normal, _) => {
        Ok(Lookahead::Plausible)
      }
    }
  }
}

enum OperandRead {
  Consumed,
  Invalid,
}

struct ImageBoundary {
  position: u64,
  kind: InlineImageBoundary,
}

fn next_image_boundary(bytes: &[u8]) -> Result<Option<ImageBoundary>, Never> {
  let Ok(end) = find_needle(bytes, b"EI");
  let Ok(start) = find_needle(bytes, b"BI");
  let select = match (end, start) {
    (Some(end_2), Some(start_2)) => match end_2.cmp(&start_2) {
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => (end_2, InlineImageBoundary::End),
      core::cmp::Ordering::Greater => (start_2, InlineImageBoundary::Begin),
    },
    (Some(end_3), None) => (end_3, InlineImageBoundary::End),
    (None, Some(start_3)) => (start_3, InlineImageBoundary::Begin),
    (None, None) => return Ok(None),
  };
  let (position, kind) = select;

  Ok(Some(ImageBoundary { position, kind }))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A lookahead parser cannot start another image lookahead and attempts at most 21 typed operators"
  )
)]
fn assess_image_tail(tail: &[u8]) -> Result<Lookahead, Never> {
  let Ok(mut reader) = Reader::new(tail);

  for _ in 0..MAX_EI_LOOKAHEAD_STEPS {
    let Ok(remaining) = reader.tail();
    let bytes = match remaining {
      Some(bytes) => bytes,
      None => return Ok(Lookahead::Rejected),
    };
    let Ok(next) = next_image_boundary(bytes);
    let boundary = match next {
      Some(boundary) => boundary,
      None => break,
    };
    let Ok(marker) = index(boundary.position);
    let Ok(advanced) = reader.skip_bytes(marker);

    match advanced {
      Some(()) => {}
      None => return Ok(Lookahead::Rejected),
    }

    match boundary.kind {
      InlineImageBoundary::End => {
        let Ok(analysis_end) = reader.fixed_offset();
        let Ok(position) = index(analysis_end);
        let analysis = match tail.get(..position) {
          Some(bytes_2) => bytes_2,
          None => return Ok(Lookahead::Rejected),
        };

        match analysis.is_ascii().then_some(()) {
          Some(()) => return read_image_lookahead(tail),
          None => return Ok(Lookahead::Rejected),
        }
      }
      InlineImageBoundary::Begin => {
        let mut probe = reader.clone();
        let Ok(skipped) = probe.skip_bytes(2);

        match skipped {
          Some(()) => {}
          None => return Ok(Lookahead::Rejected),
        }

        match probe.read_without_context::<InlineImageDict<'_>>() {
          Some(_next_image) => return Ok(Lookahead::Plausible),
          None => {}
        }
      }
    }

    let Ok(advanced_2) = reader.read_byte();

    match advanced_2 {
      Some(_) => {}
      None => return Ok(Lookahead::Rejected),
    }
  }

  Ok(Lookahead::Plausible)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "This typed scanner is constructed with InlineImageLookahead so it cannot start another lookahead"
  )
)]
fn read_image_lookahead(tail: &[u8]) -> Result<Lookahead, Never> {
  let Ok(untyped) = UntypedIter::new(tail);
  let Ok(mut iter) = TypedIter::from_untyped(UntypedIter {
    read_mode: ContentRead::InlineImageLookahead,
    ..untyped
  });

  for index in 0_u8..=20 {
    let Ok(instruction) = iter.next();

    match (index, instruction) {
      (_, None) => return Ok(Lookahead::Rejected),
      (20, Some(_))
      | (
        _,
        Some(
          TypedInstruction::NextLineAndShowText(_)
          | TypedInstruction::ShowText(_)
          | TypedInstruction::ShowTexts(_)
          | TypedInstruction::ShowTextWithParameters(_),
        ),
      ) => return Ok(Lookahead::Plausible),
      (_, Some(_other)) => {}
    }
  }

  Ok(Lookahead::Rejected)
}

#[derive(Clone)]
pub struct TypedIter<'a> {
  untyped: UntypedIter<'a>,
}

impl<'a> TypedIter<'a> {
  pub fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(untyped) = UntypedIter::new(data);

    Ok(Self { untyped })
  }

  pub(crate) fn from_untyped(
    untyped: UntypedIter<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { untyped })
  }

  #[allow(
    clippy::should_implement_trait,
    reason = "next answers a Result as every infallible function here does, which is not the signature Iterator asks for"
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The inline-image probe uses this method with its lookahead explicitly disabled"
    )
  )]
  pub fn next(
    &mut self,
  ) -> core::result::Result<Option<TypedInstruction<'_, 'a>>, console_core_never::Never> {
    let Ok(explicit_value) = self.untyped.next();
    let op = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(explicit_value_2) = TypedInstruction::dispatch(&op);

    Ok(match explicit_value_2 {
      Some(op_2) => Some(op_2),

      None => {
        match ::core::primitive::bool::then_some(
          [
            ::core::convert::AsRef::<[_]>::as_ref(b"m"),
            ::core::convert::AsRef::<[_]>::as_ref(b"l"),
            ::core::convert::AsRef::<[_]>::as_ref(b"c"),
            ::core::convert::AsRef::<[_]>::as_ref(b"v"),
            ::core::convert::AsRef::<[_]>::as_ref(b"y"),
            ::core::convert::AsRef::<[_]>::as_ref(b"h"),
            ::core::convert::AsRef::<[_]>::as_ref(b"re"),
          ]
          .contains(&op.operator.0.deref()),
          (),
        ) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => Some(TypedInstruction::Fallback(op.operator)),
        }
      }
    })
  }
}

pub struct Instruction<'b, 'a> {
  pub operands: &'b Stack<'a>,

  pub operator: &'b Operator<'a>,
}

impl<'b, 'a> Instruction<'b, 'a> {
  pub fn operands(
    &self,
  ) -> core::result::Result<OperandIterator<'b, 'a>, console_core_never::Never> {
    let Ok(operand_iterator) = OperandIterator::new(self.operands);

    Ok(operand_iterator)
  }
}

pub struct Stack<'a> {
  data: [Object<'a>; 10],
  len: u8,
}

impl<'a> Default for Stack<'a> {
  fn default() -> Self {
    let Ok(stack) = Self::new();

    stack
  }
}

impl<'a> Stack<'a> {
  pub fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      data: array::from_fn(|_| Object::Null(Null)),
      len: 0,
    })
  }

  fn push(&mut self, operand: Object<'a>) -> Result<Option<()>, console_core_never::Never> {
    let Ok(position) = console_core_number_conversion::index(self.len);
    let target = match self.data.get_mut(position) {
      Some(slot) => slot,
      None => return Ok(None),
    };
    *target = operand;
    self.len = self.len.saturating_add(1);

    Ok(Some(()))
  }

  fn clear(&mut self) -> Result<(), console_core_never::Never> {
    self.len = 0;

    Ok(())
  }

  fn len(&self) -> Result<u8, console_core_never::Never> {
    Ok(self.len)
  }

  fn as_slice(&self) -> Result<&[Object<'a>], console_core_never::Never> {
    let Ok(end) = console_core_number_conversion::index(self.len);

    Ok(match self.data.get(..end) {
      Some(operands) => operands,
      None => &[],
    })
  }

  fn get<'b, T>(&'b self, operand: u8) -> Result<Option<T>, console_core_never::Never>
  where
    T: Operand<'b, 'a>,
  {
    let Ok(slice) = self.as_slice();
    let Ok(position) = console_core_number_conversion::index(operand);

    Ok(slice.get(position).and_then(T::from_object))
  }

  fn get_all<'b, T>(&'b self) -> core::result::Result<Option<Vec<T>>, console_core_never::Never>
  where
    T: Operand<'b, 'a>,
  {
    let mut operands = Vec::new();

    let Ok(slice) = self.as_slice();

    for op in slice {
      let converted = match T::from_object(op) {
        Some(value) => value,
        None => return Ok(None),
      };
      operands.push(converted);
    }

    Ok(Some(operands))
  }
}

impl Debug for Stack<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_list()
      .entries({
        let Ok(explicit_value) = self.as_slice();
        explicit_value
      })
      .finish()
  }
}

impl Clone for Stack<'_> {
  fn clone(&self) -> Self {
    let Ok(mut stack) = Self::new();
    let Ok(operands) = self.as_slice();

    for (target, operand) in stack.data.iter_mut().zip(operands) {
      *target = operand.clone();
    }

    stack.len = self.len;

    stack
  }
}

impl PartialEq for Stack<'_> {
  fn eq(&self, other: &Self) -> bool {
    let Ok(slice) = self.as_slice();
    let Ok(other_slice) = other.as_slice();

    slice == other_slice
  }
}

trait Operand<'b, 'a>: Sized {
  fn from_object(object: &'b Object<'a>) -> Option<Self>;
}

impl<'b, 'a> Operand<'b, 'a> for Number {
  fn from_object(object: &'b Object<'a>) -> Option<Self> {
    match object {
      Object::Number(n) => Some(*n),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => None,
    }
  }
}

impl<'b, 'a> Operand<'b, 'a> for &'b object::String<'a> {
  fn from_object(object: &'b Object<'a>) -> Option<Self> {
    match object {
      Object::String(s) => Some(s),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => None,
    }
  }
}

impl<'b, 'a> Operand<'b, 'a> for &'b Name<'a> {
  fn from_object(object: &'b Object<'a>) -> Option<Self> {
    match object {
      Object::Name(n) => Some(n),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => None,
    }
  }
}

impl<'b, 'a> Operand<'b, 'a> for &'b Array<'a> {
  fn from_object(object: &'b Object<'a>) -> Option<Self> {
    match object {
      Object::Array(a) => Some(a),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Stream(_) => None,
    }
  }
}

impl<'b, 'a> Operand<'b, 'a> for &'b Stream<'a> {
  fn from_object(object: &'b Object<'a>) -> Option<Self> {
    match object {
      Object::Stream(s) => Some(s),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_) => None,
    }
  }
}

impl<'b, 'a> Operand<'b, 'a> for &'b Object<'a> {
  fn from_object(object: &'b Object<'a>) -> Option<Self> {
    Some(object)
  }
}

pub struct OperandIterator<'b, 'a> {
  operands: core::slice::Iter<'b, Object<'a>>,
}

impl<'b, 'a> OperandIterator<'b, 'a> {
  fn new(stack: &'b Stack<'a>) -> Result<Self, console_core_never::Never> {
    let Ok(slice) = stack.as_slice();

    Ok(Self {
      operands: slice.iter(),
    })
  }
}

impl<'b, 'a> Iterator for OperandIterator<'b, 'a> {
  type Item = &'b Object<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    self.operands.next()
  }
}

pub(crate) trait OperatorTrait<'b, 'a>: Sized {
  const OPERATOR: &'static str;

  fn from_stack(stack: &'b Stack<'a>) -> Option<Self>;
}

mod macros {
  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "The generated operator types use these standard stack and typed-instruction conversions"
    )
  )]
  macro_rules! op_impl {
        ($t:ident $(<$($l:lifetime),+>)?, $e:expr, |$stack:ident : $stack_ty:ty| $body:block) => {
            impl<'b, 'a> OperatorTrait<'b, 'a> for $t$(<$($l),+>)? {
                const OPERATOR: &'static str = $e;

                #[inline(always)]
                fn from_stack($stack: $stack_ty) -> Option<Self> {
                    $body.or_else(|| {
                        warn!("failed to convert operands for operator {}", Self::OPERATOR);

                        None
                    })
                }
            }

            impl<'b, 'a> From<$t$(<$($l),+>)?> for TypedInstruction<'b, 'a> {
                fn from(value: $t$(<$($l),+>)?) -> Self {
                    TypedInstruction::$t(value)
                }
            }

            impl<'b, 'a> TryFrom<TypedInstruction<'b, 'a>> for $t$(<$($l),+>)? {
                type Error = ();

                fn try_from(value: TypedInstruction<'b, 'a>) -> core::result::Result<Self, Self::Error> {
                    match value {
                        TypedInstruction::$t(e) => Ok(e),
                        _ => Err(())
                    }
                }
            }
        };
    }

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "An operand-free PDF operator creates the generated unit type"
    )
  )]
  macro_rules! op0 {
    ($operator:ident, $spelling:expr) => {
      crate::content::macros::op_impl!($operator, $spelling, |_stack: &'b Stack<'a>| {
        Some(Self)
      });
    };
  }

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "Variable-length PDF color instructions consume every operand as the expected type"
    )
  )]
  macro_rules! op_all {
    ($operator:ident, $spelling:expr) => {
      crate::content::macros::op_impl!($operator, $spelling, |stack: &'b Stack<'a>| {
        let Ok(entries) = stack.get_all();
        let operands = entries?;
        Some(Self(operands))
      });
    };
  }

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "Fixed-arity PDF instructions decode the last required operands in their declared order"
    )
  )]
  macro_rules! op_fixed {
    (
      $operator:ident $(<$($l:lifetime),+>)?,
      $spelling:expr,
      $($index:literal),+
    ) => {
      crate::content::macros::op_impl!($operator$(<$($l),+>)?, $spelling, |stack: &'b Stack<'a>| {
        const ARITY: u8 = 0 $(+ { let _ = $index; 1 })+;
        let Ok(count) = stack.len();
        let start = count.saturating_sub(ARITY);

        Some(Self($({
          let Ok(entry) = stack.get(start.saturating_add($index));
          entry?
        }),+))
      });
    };
  }

  pub(crate) use op_all;
  pub(crate) use op_fixed;
  pub(crate) use op_impl;
  pub(crate) use op0;
}
