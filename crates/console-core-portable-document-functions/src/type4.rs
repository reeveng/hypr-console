//! A calculator program can only visit each selected operator once. Each
//! operation and procedure frame uses at least one byte of the source, so
//! evaluation has room for two steps per byte and one final completion step.

use crate::{AsBoolAnswer, Clamper, Values};
use console_core_portable_document_syntax::content;
use console_core_portable_document_syntax::object::Number;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::reader::Reader;
use console_core_portable_document_syntax::reader::{ReaderContext, ReaderExt};
use std::cmp::Ordering;
use std::num::FpCategory;
use std::ops::Rem;
use std::slice;

#[derive(Debug)]
pub(crate) struct Type4 {
  program: Vec<PostScriptOp>,
  clamper: Clamper,
  max_steps: u64,
}

impl Type4 {
  pub(crate) fn new(
    stream: &Stream<'_>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(dict) = stream.dict();
    let Ok(limits) = Clamper::new(dict);
    let clamper = match limits {
      Some(value) => value,
      None => return Ok(None),
    };
    let decoded = match stream.decoded() {
      Ok(value) => value,
      Err(_undecodable) => return Ok(None),
    };
    let Ok(parsed) = parse_procedure(&decoded);
    let program = match parsed {
      Some(operators) => operators,
      None => return Ok(None),
    };
    let Ok(max_steps) = evaluation_steps(&decoded);

    Ok(Some(Self {
      clamper,
      program,
      max_steps,
    }))
  }

  pub(crate) fn eval(
    &self,
    mut input: Values,
  ) -> core::result::Result<Option<Values>, console_core_never::Never> {
    let Ok(()) = self.clamper.clamp_input(&mut input);

    let Ok(mut arg_stack) = InterpreterStack::new();

    for number in input {
      let Ok(pushed) = arg_stack.push(Argument::Float(number));

      match pushed {
        Some(()) => {}
        None => return Ok(None),
      }
    }

    let Ok(evaluated) = eval_inner(&self.program, &mut arg_stack, self.max_steps);

    match evaluated {
      Some(()) => {}
      None => return Ok(None),
    }

    let Ok(items) = arg_stack.items();
    let mut out: Vec<_> = items
      .iter()
      .map(|argument| {
        let Ok(number) = argument.as_f32();

        number
      })
      .collect();

    let Ok(()) = self.clamper.clamp_output(&mut out);

    Ok(Some(out))
  }
}

fn evaluation_steps(source: &[u8]) -> Result<u64, console_core_never::Never> {
  let Ok(bytes) = console_core_number_conversion::fitted::<_, u64>(source.len());

  Ok(bytes.saturating_mul(2).saturating_add(1))
}

#[derive(Clone, Copy)]
enum Argument {
  Float(f32),
  Boolean(AsBoolAnswer),
}

impl Argument {
  fn as_bool(&self) -> core::result::Result<AsBoolAnswer, console_core_never::Never> {
    Ok(match self {
      Self::Float(value) => match value.classify() {
        FpCategory::Zero => AsBoolAnswer::No,
        FpCategory::Nan | FpCategory::Infinite | FpCategory::Normal | FpCategory::Subnormal => {
          AsBoolAnswer::Yes
        }
      },
      Self::Boolean(truth) => *truth,
    })
  }

  fn as_f32(&self) -> core::result::Result<f32, console_core_never::Never> {
    match self {
      Self::Float(value) => Ok(*value),
      Self::Boolean(truth) => truth.as_number(),
    }
  }
}

struct ArgumentsStack<T, const LIMIT: usize> {
  items: Vec<T>,
}

impl<T, const LIMIT: usize> ArgumentsStack<T, LIMIT> {
  fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { items: Vec::new() })
  }

  fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    console_core_number_conversion::fitted::<_, u32>(self.items.len())
  }

  fn push(&mut self, n: T) -> core::result::Result<Option<()>, console_core_never::Never> {
    Ok(match self.items.len().cmp(&LIMIT) {
      Ordering::Equal => {
        #[cfg(feature = "logging")]
        console_core_logging::error!("overflowed post script argument stack");

        None
      }
      Ordering::Less | Ordering::Greater => {
        self.items.push(n);

        Some(())
      }
    })
  }

  fn at(&self, position: u32) -> core::result::Result<Option<&T>, console_core_never::Never> {
    let Ok(index) = console_core_number_conversion::index(position);

    Ok(self.items.get(index))
  }

  fn last(&self) -> core::result::Result<Option<&T>, console_core_never::Never> {
    Ok(self.items.last())
  }

  fn pop(&mut self) -> core::result::Result<Option<T>, console_core_never::Never> {
    let popped = self.items.pop();

    match &popped {
      Some(_item) => {}
      None => {
        #[cfg(feature = "logging")]
        console_core_logging::error!("underflowed post script argument stack");
      }
    }

    Ok(popped)
  }

  fn items(&self) -> core::result::Result<&[T], console_core_never::Never> {
    Ok(&self.items)
  }

  fn items_mut(&mut self) -> core::result::Result<&mut [T], console_core_never::Never> {
    Ok(&mut self.items)
  }
}

type InterpreterStack = ArgumentsStack<Argument, 64>;
type ParseStack = ArgumentsStack<Vec<PostScriptOp>, 2>;

fn whole_f32(whole: i32) -> core::result::Result<f32, console_core_never::Never> {
  console_core_number_conversion::narrow_f32(f64::from(whole))
}

fn popped_count(
  arg_stack: &mut InterpreterStack,
) -> core::result::Result<Option<u32>, console_core_never::Never> {
  let Ok(popped) = arg_stack.pop();
  let argument = match popped {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(number) = argument.as_f32();

  Ok(Some({
    let Ok(numeric_value) = console_core_number_conversion::toward_zero_u32(f64::from(number));
    numeric_value
  }))
}

enum Next<'p> {
  Continue,
  Enter(&'p [PostScriptOp]),
}

enum Stepped {
  Went,
  Failed,
}

fn eval_inner(
  procedure: &[PostScriptOp],
  arg_stack: &mut InterpreterStack,
  max_steps: u64,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let mut frames = vec![procedure.iter()];

  for _ in 0..max_steps {
    let Ok(stepped) = step(&mut frames, arg_stack);

    match stepped {
      Some(Stepped::Went) => continue,
      Some(Stepped::Failed) => return Ok(None),
      None => return Ok(Some(())),
    }
  }

  Ok(None)
}

fn step<'p>(
  frames: &mut Vec<slice::Iter<'p, PostScriptOp>>,
  arg_stack: &mut InterpreterStack,
) -> core::result::Result<Option<Stepped>, console_core_never::Never> {
  let frame = match frames.last_mut() {
    Some(frame) => frame,
    None => return Ok(None),
  };

  Ok(Some(match frame.next() {
    None => {
      frames.truncate(frames.len().saturating_sub(1));

      Stepped::Went
    }
    Some(op) => {
      let Ok(executed) = execute(op, arg_stack);

      match executed {
        Some(Next::Continue) => Stepped::Went,
        Some(Next::Enter(procedure)) => {
          frames.push(procedure.iter());

          Stepped::Went
        }
        None => Stepped::Failed,
      }
    }
  }))
}

fn execute<'p>(
  op: &'p PostScriptOp,
  stack: &mut InterpreterStack,
) -> Result<Option<Next<'p>>, console_core_never::Never> {
  let applied = match op {
    PostScriptOp::Number(number) => {
      let Ok(wide) = number.as_f64();
      let Ok(narrow) = console_core_number_conversion::narrow_f32(wide);

      stack.push(Argument::Float(narrow))
    }
    PostScriptOp::Abs => unary(stack, f32::abs),
    PostScriptOp::Add => binary(stack, |left, right| left + right),
    PostScriptOp::Atan => binary(stack, |rise, run| {
      let angle = rise.atan2(run).to_degrees() % 360.0;

      match angle.partial_cmp(&0.0) {
        Some(Ordering::Less) => angle + 360.0,
        Some(Ordering::Equal | Ordering::Greater) | None => angle,
      }
    }),
    PostScriptOp::Ceiling => unary(stack, f32::ceil),
    PostScriptOp::Cos => unary(stack, |angle| angle.to_radians().cos()),
    PostScriptOp::Cvi | PostScriptOp::Truncate => unary(stack, f32::trunc),
    PostScriptOp::Cvr => unary(stack, |value| value),
    PostScriptOp::Div => binary(stack, |dividend, divisor| dividend / divisor),
    PostScriptOp::Exp => binary(stack, f32::powf),
    PostScriptOp::Floor => unary(stack, f32::floor),
    PostScriptOp::Idiv => integer_divide(stack),
    PostScriptOp::Ln => unary(stack, f32::ln),
    PostScriptOp::Log => unary(stack, f32::log10),
    PostScriptOp::Mod => binary(stack, f32::rem),
    PostScriptOp::Mul => binary(stack, |left, right| left * right),
    PostScriptOp::Neg => unary(stack, |number| -number),
    PostScriptOp::Round => unary(stack, f32::round),
    PostScriptOp::Sin => unary(stack, |angle| angle.to_radians().sin()),
    PostScriptOp::Sqrt => unary(stack, f32::sqrt),
    PostScriptOp::Sub => binary(stack, |left, right| left - right),
    PostScriptOp::And => binary(stack, |left, right| {
      let Ok(number) = bitwise_float([left, right], |lhs, rhs| lhs & rhs);

      number
    }),
    PostScriptOp::Bitshift => bitshift(stack),
    PostScriptOp::Eq => compare(stack, |left, right| {
      matches!(left.partial_cmp(&right), Some(Ordering::Equal))
    }),
    PostScriptOp::False => stack.push(Argument::Boolean(AsBoolAnswer::No)),
    PostScriptOp::Ge => compare(stack, |left, right| left >= right),
    PostScriptOp::Gt => compare(stack, |left, right| left > right),
    PostScriptOp::Le => compare(stack, |left, right| left <= right),
    PostScriptOp::Lt => compare(stack, |left, right| left < right),
    PostScriptOp::Ne => compare(stack, |left, right| {
      !matches!(left.partial_cmp(&right), Some(Ordering::Equal))
    }),
    PostScriptOp::Not => invert(stack),
    PostScriptOp::Or => binary_logic(
      stack,
      |left, right| left | right,
      |left, right| match (left, right) {
        (AsBoolAnswer::No, AsBoolAnswer::No) => AsBoolAnswer::No,
        (AsBoolAnswer::Yes, _) | (_, AsBoolAnswer::Yes) => AsBoolAnswer::Yes,
      },
    ),
    PostScriptOp::True => stack.push(Argument::Boolean(AsBoolAnswer::Yes)),
    PostScriptOp::Xor => binary_logic(
      stack,
      |left, right| left ^ right,
      |left, right| match (left, right) {
        (AsBoolAnswer::Yes, AsBoolAnswer::Yes) | (AsBoolAnswer::No, AsBoolAnswer::No) => {
          AsBoolAnswer::No
        }
        (AsBoolAnswer::Yes, AsBoolAnswer::No) | (AsBoolAnswer::No, AsBoolAnswer::Yes) => {
          AsBoolAnswer::Yes
        }
      },
    ),
    PostScriptOp::If(program) => return branch(stack, program, None),
    PostScriptOp::IfElse(yes, no) => return branch(stack, yes, Some(no)),
    PostScriptOp::Copy => copy(stack),
    PostScriptOp::Dup => {
      let Ok(last) = stack.last();

      match last.copied() {
        Some(item) => stack.push(item),
        None => Ok(None),
      }
    }
    PostScriptOp::Exch => exchange(stack),
    PostScriptOp::Index => duplicate_at(stack),
    PostScriptOp::Pop => {
      let Ok(popped) = stack.pop();

      Ok(popped.map(|_item| ()))
    }
    PostScriptOp::Roll => roll(stack),
  };
  let Ok(outcome) = applied;

  Ok(outcome.map(|()| Next::Continue))
}

fn unary(
  stack: &mut InterpreterStack,
  apply: impl FnOnce(f32) -> f32,
) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = stack.pop();
  let input = match popped {
    Some(input) => input,
    None => return Ok(None),
  };
  let Ok(number) = input.as_f32();

  stack.push(Argument::Float(apply(number)))
}

fn operands(
  stack: &mut InterpreterStack,
) -> Result<Option<[Argument; 2]>, console_core_never::Never> {
  let Ok(top) = stack.pop();
  let right = match top {
    Some(right) => right,
    None => return Ok(None),
  };
  let Ok(below) = stack.pop();

  Ok(below.map(|left| [left, right]))
}

fn numbers(stack: &mut InterpreterStack) -> Result<Option<[f32; 2]>, console_core_never::Never> {
  let Ok(popped) = operands(stack);
  let [left, right] = match popped {
    Some(pair) => pair,
    None => return Ok(None),
  };
  let Ok(lhs) = left.as_f32();
  let Ok(rhs) = right.as_f32();

  Ok(Some([lhs, rhs]))
}

fn binary(
  stack: &mut InterpreterStack,
  apply: impl FnOnce(f32, f32) -> f32,
) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = numbers(stack);

  match popped {
    Some([left, right]) => stack.push(Argument::Float(apply(left, right))),
    None => Ok(None),
  }
}

fn compare(
  stack: &mut InterpreterStack,
  test: impl FnOnce(f32, f32) -> bool,
) -> Result<Option<()>, console_core_never::Never> {
  binary(stack, |left, right| {
    let Ok(number) = AsBoolAnswer::from(test(left, right)).as_number();

    number
  })
}

fn bitwise_float(
  [left, right]: [f32; 2],
  apply: impl FnOnce(i32, i32) -> i32,
) -> Result<f32, console_core_never::Never> {
  let Ok(lhs) = console_core_number_conversion::toward_zero_i32(f64::from(left));
  let Ok(rhs) = console_core_number_conversion::toward_zero_i32(f64::from(right));

  whole_f32(apply(lhs, rhs))
}

fn binary_logic(
  stack: &mut InterpreterStack,
  bits: impl FnOnce(i32, i32) -> i32,
  truth: impl FnOnce(AsBoolAnswer, AsBoolAnswer) -> AsBoolAnswer,
) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = operands(stack);
  let [left, right] = match popped {
    Some(pair) => pair,
    None => return Ok(None),
  };
  let result = match (left, right) {
    (Argument::Float(lhs), Argument::Float(rhs)) => {
      let Ok(number) = bitwise_float([lhs, rhs], bits);

      Argument::Float(number)
    }
    (Argument::Boolean(_), _) | (Argument::Float(_), Argument::Boolean(_)) => {
      let Ok(lhs) = left.as_bool();
      let Ok(rhs) = right.as_bool();

      Argument::Boolean(truth(lhs, rhs))
    }
  };

  stack.push(result)
}

fn integer_divide(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = numbers(stack);
  let [dividend, divisor] = match popped {
    Some(pair) => pair,
    None => return Ok(None),
  };
  let Ok(whole_dividend) = console_core_number_conversion::toward_zero_i32(f64::from(dividend));
  let Ok(whole_divisor) = console_core_number_conversion::toward_zero_i32(f64::from(divisor));
  let quotient = match whole_dividend.checked_div(whole_divisor) {
    Some(quotient) => quotient,
    None => return Ok(None),
  };
  let Ok(number) = whole_f32(quotient);

  stack.push(Argument::Float(number))
}

fn bitshift(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = numbers(stack);
  let [number, places] = match popped {
    Some(pair) => pair,
    None => return Ok(None),
  };
  let Ok(bits) = console_core_number_conversion::toward_zero_u32(f64::from(number));
  let Ok(count) = console_core_number_conversion::toward_zero_i32(f64::from(places));
  let shifted = match count {
    0.. => bits.checked_shl(count.unsigned_abs()),
    ..0 => bits.checked_shr(count.unsigned_abs()),
  };

  match shifted {
    Some(result) => {
      let Ok(value) = console_core_number_conversion::narrow_f32(f64::from(result));

      stack.push(Argument::Float(value))
    }
    None => Ok(None),
  }
}

fn invert(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = stack.pop();
  let argument = match popped {
    Some(argument) => argument,
    None => return Ok(None),
  };
  let result = match argument {
    Argument::Float(number) => {
      let Ok(whole) = console_core_number_conversion::toward_zero_i32(f64::from(number));
      let Ok(inverted) = whole_f32(!whole);

      Argument::Float(inverted)
    }
    Argument::Boolean(AsBoolAnswer::Yes) => Argument::Boolean(AsBoolAnswer::No),
    Argument::Boolean(AsBoolAnswer::No) => Argument::Boolean(AsBoolAnswer::Yes),
  };

  stack.push(result)
}

fn branch<'p>(
  stack: &mut InterpreterStack,
  yes: &'p [PostScriptOp],
  no: Option<&'p [PostScriptOp]>,
) -> Result<Option<Next<'p>>, console_core_never::Never> {
  let Ok(popped) = stack.pop();
  let condition = match popped {
    Some(condition) => condition,
    None => return Ok(None),
  };
  let Ok(truth) = condition.as_bool();

  Ok(Some(match truth {
    AsBoolAnswer::Yes => Next::Enter(yes),
    AsBoolAnswer::No => no.map_or(Next::Continue, Next::Enter),
  }))
}

fn copy(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(requested) = popped_count(stack);
  let count = match requested {
    Some(count) => count,
    None => return Ok(None),
  };
  let Ok(height) = stack.height();
  let start = match height.checked_sub(count) {
    Some(start) => start,
    None => return Ok(None),
  };

  for position in start..height {
    let Ok(found) = stack.at(position);
    let copied = match found.copied() {
      Some(item) => item,
      None => return Ok(None),
    };
    let Ok(pushed) = stack.push(copied);

    match pushed {
      Some(()) => {}
      None => return Ok(None),
    }
  }

  Ok(Some(()))
}

fn exchange(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(popped) = operands(stack);
  let [left, right] = match popped {
    Some(pair) => pair,
    None => return Ok(None),
  };
  let Ok(pushed) = stack.push(right);

  match pushed {
    Some(()) => stack.push(left),
    None => Ok(None),
  }
}

fn duplicate_at(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(requested) = popped_count(stack);
  let depth = match requested {
    Some(depth) => depth,
    None => return Ok(None),
  };
  let Ok(height) = stack.height();
  let position = match depth
    .checked_add(1)
    .and_then(|reach| height.checked_sub(reach))
  {
    Some(position) => position,
    None => return Ok(None),
  };
  let Ok(found) = stack.at(position);

  match found.copied() {
    Some(item) => stack.push(item),
    None => Ok(None),
  }
}

fn roll(stack: &mut InterpreterStack) -> Result<Option<()>, console_core_never::Never> {
  let Ok(rotation) = stack.pop();
  let argument = match rotation {
    Some(argument) => argument,
    None => return Ok(None),
  };
  let Ok(number) = argument.as_f32();
  let Ok(turn) = console_core_number_conversion::toward_zero_i32(f64::from(number));
  let Ok(requested) = popped_count(stack);
  let count = match requested {
    Some(count) => count,
    None => return Ok(None),
  };
  let Ok(height) = stack.height();
  let start = match height.checked_sub(count) {
    Some(start) => start,
    None => return Ok(None),
  };
  let Ok(first) = console_core_number_conversion::index(start);
  let Ok(items) = stack.items_mut();
  let target = match items.get_mut(first..) {
    Some(slice) => slice,
    None => return Ok(None),
  };

  match target {
    [] => return Ok(Some(())),
    [_, ..] => {}
  }

  let shift = match turn.unsigned_abs().checked_rem(count) {
    Some(shift) => shift,
    None => return Ok(None),
  };
  let Ok(places) = console_core_number_conversion::index(shift);

  match turn {
    0.. => target.rotate_right(places),
    ..0 => target.rotate_left(places),
  }

  Ok(Some(()))
}

fn parse_procedure(
  data: &[u8],
) -> core::result::Result<Option<Vec<PostScriptOp>>, console_core_never::Never> {
  let Ok(mut r) = Reader::new(data);

  r.skip_white_spaces_and_comments();

  let Ok(opened) = r.forward_tag(b"{");

  match opened {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(first) = ParseStack::new();
  let mut frames = vec![(Vec::new(), first)];
  let Ok(bytes) = console_core_number_conversion::fitted::<_, u64>(data.len());

  for _ in 0..=bytes {
    let Ok(parsed) = parse_step(&mut r, &mut frames);

    match parsed {
      Parsed::Went => continue,
      Parsed::Failed => return Ok(None),
      Parsed::Finished(ops) => return Ok(Some(ops)),
    }
  }

  Ok(None)
}

enum Parsed {
  Went,
  Failed,
  Finished(Vec<PostScriptOp>),
}

fn parse_step(
  r: &mut Reader<'_>,
  frames: &mut Vec<(Vec<PostScriptOp>, ParseStack)>,
) -> core::result::Result<Parsed, console_core_never::Never> {
  r.skip_white_spaces_and_comments();

  let Ok(peeked) = r.peek_byte();

  Ok(match peeked {
    None => Parsed::Failed,
    Some(b'}') => {
      let Ok(closed) = r.forward_tag(b"}");
      let Ok(parsed) = close_procedure(frames, closed);

      parsed
    }
    Some(b'{') => {
      let Ok(opened) = r.forward_tag(b"{");
      let Ok(procedures) = ParseStack::new();

      frames.push((Vec::new(), procedures));

      match opened {
        Some(()) => Parsed::Went,
        None => Parsed::Failed,
      }
    }
    Some(_other) => match frames.last_mut() {
      Some((ops, procedures)) => {
        let Ok(read) = PostScriptOp::from_reader(r, procedures);

        match read {
          Some(op) => {
            ops.push(op);

            Parsed::Went
          }
          None => Parsed::Failed,
        }
      }
      None => Parsed::Failed,
    },
  })
}

fn close_procedure(
  frames: &mut Vec<(Vec<PostScriptOp>, ParseStack)>,
  closed: Option<()>,
) -> Result<Parsed, console_core_never::Never> {
  let operators = match (closed, frames.pop()) {
    (Some(()), Some((ops, _procedures))) => ops,
    (Some(()), None) | (None, _) => return Ok(Parsed::Failed),
  };

  Ok(match frames.last_mut() {
    Some((_outer, procedures)) => {
      let Ok(pushed) = procedures.push(operators);

      match pushed {
        Some(()) | None => Parsed::Went,
      }
    }
    None => Parsed::Finished(operators),
  })
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum PostScriptOp {
  Number(Number),
  Abs,
  Add,
  Atan,
  Ceiling,
  Cos,
  Cvi,
  Cvr,
  Div,
  Exp,
  Floor,
  Idiv,
  Ln,
  Log,
  Mod,
  Mul,
  Neg,
  Round,
  Sin,
  Sqrt,
  Sub,
  Truncate,
  And,
  Bitshift,
  Eq,
  False,
  Ge,
  Gt,
  Le,
  Lt,
  Ne,
  Not,
  Or,
  True,
  Xor,
  If(Vec<Self>),
  IfElse(Vec<Self>, Vec<Self>),
  Copy,
  Dup,
  Exch,
  Index,
  Pop,
  Roll,
}

impl PostScriptOp {
  fn from_reader(
    r: &mut Reader<'_>,
    stack: &mut ParseStack,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(context) = ReaderContext::dummy();
    let op = match r.read::<Number>(&context) {
      Some(n) => Self::Number(n),
      None => {
        let operator = match r.read::<content::Operator<'_>>(&context) {
          Some(value) => value,
          None => return Ok(None),
        };

        match operator.as_ref() {
          b"abs" => Self::Abs,
          b"add" => Self::Add,
          b"atan" => Self::Atan,
          b"ceiling" => Self::Ceiling,
          b"cos" => Self::Cos,
          b"cvi" => Self::Cvi,
          b"cvr" => Self::Cvr,
          b"div" => Self::Div,
          b"exp" => Self::Exp,
          b"floor" => Self::Floor,
          b"idiv" => Self::Idiv,
          b"ln" => Self::Ln,
          b"log" => Self::Log,
          b"mod" => Self::Mod,
          b"mul" => Self::Mul,
          b"neg" => Self::Neg,
          b"round" => Self::Round,
          b"sin" => Self::Sin,
          b"sqrt" => Self::Sqrt,
          b"sub" => Self::Sub,
          b"truncate" => Self::Truncate,
          b"and" => Self::And,
          b"bitshift" => Self::Bitshift,
          b"eq" => Self::Eq,
          b"false" => Self::False,
          b"ge" => Self::Ge,
          b"gt" => Self::Gt,
          b"le" => Self::Le,
          b"lt" => Self::Lt,
          b"ne" => Self::Ne,
          b"not" => Self::Not,
          b"or" => Self::Or,
          b"true" => Self::True,
          b"xor" => Self::Xor,
          b"if" => {
            let Ok(pop) = stack.pop();

            Self::If(match pop {
              Some(value) => value,
              None => return Ok(None),
            })
          }
          b"ifelse" => {
            let Ok(fallback) = stack.pop();
            let no = match fallback {
              Some(program) => program,
              None => return Ok(None),
            };
            let Ok(primary) = stack.pop();
            let yes = match primary {
              Some(program) => program,
              None => return Ok(None),
            };

            Self::IfElse(yes, no)
          }
          b"copy" => Self::Copy,
          b"dup" => Self::Dup,
          b"exch" => Self::Exch,
          b"index" => Self::Index,
          b"pop" => Self::Pop,
          b"roll" => Self::Roll,
          _ => {
            #[cfg(feature = "logging")]
            console_core_logging::error!("encountered unknown postscript operator {operator:?}");

            return Ok(None);
          }
        }
      }
    };

    Ok(Some(op))
  }
}

#[cfg(test)]
mod tests {
  use crate::type4::{
    InterpreterStack, PostScriptOp, Type4, eval_inner, evaluation_steps, parse_procedure,
  };
  use crate::{Clamper, Function, FunctionType, TupleVec, Values};
  use std::f32::consts::LN_10;

  use console_core_portable_document_syntax::object::Number;

  #[test]
  fn stack_overflow_in_operator_is_rejected() -> Result<(), crate::TestFailure> {
    let Ok(mut stack) = InterpreterStack::new();
    let program = vec![PostScriptOp::True; 65];
    assert_eq!(eval_inner(&program, &mut stack, 128), Ok(None));

    Ok(())
  }

  #[test]
  fn oversized_shift_is_rejected() -> Result<(), crate::TestFailure> {
    let Ok(mut stack) = InterpreterStack::new();
    let Ok(number) = Number::from_i32(1);
    let program = [
      PostScriptOp::Number(number),
      PostScriptOp::Number({
        let Ok(explicit_value) = Number::from_i32(32);
        explicit_value
      }),
      PostScriptOp::Bitshift,
    ];
    assert_eq!(eval_inner(&program, &mut stack, 128), Ok(None));

    Ok(())
  }

  #[test]
  fn too_many_input_arguments_are_rejected() -> Result<(), crate::TestFailure> {
    let function = Type4 {
      program: Vec::new(),
      max_steps: 2,
      clamper: Clamper {
        domain: TupleVec::new(),
        range: None,
      },
    };
    assert_eq!(function.eval(vec![0.0; 65]), Ok(None));

    Ok(())
  }

  #[test]
  fn lex_1() -> Result<(), crate::TestFailure> {
    let program = b"{ copy dup 2.0 exch roll }";
    let Ok(procedure) = parse_procedure(program);
    let parsed = procedure.ok_or(crate::TestFailure::MissingValue)?;

    let Ok(number) = Number::from_f32(2.0);
    assert_eq!(
      parsed,
      vec![
        PostScriptOp::Copy,
        PostScriptOp::Dup,
        PostScriptOp::Number(number),
        PostScriptOp::Exch,
        PostScriptOp::Roll,
      ]
    );

    Ok(())
  }

  #[test]
  fn lex_3() -> Result<(), crate::TestFailure> {
    let program = b" {  {dup exch} if {0} {1} ifelse }";
    let Ok(procedure) = parse_procedure(program);
    let parsed = procedure.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(
      parsed,
      vec![
        PostScriptOp::If(vec![PostScriptOp::Dup, PostScriptOp::Exch]),
        PostScriptOp::IfElse(
          vec![PostScriptOp::Number({
            let Ok(explicit_value) = Number::from_i32(0);
            explicit_value
          })],
          vec![PostScriptOp::Number({
            let Ok(explicit_value) = Number::from_i32(1);
            explicit_value
          })]
        )
      ]
    );

    Ok(())
  }

  fn op_impl(prog: &str, out: &[f32]) -> Result<(), crate::TestFailure> {
    let source = format!("{{{prog}}}");
    let Ok(parsed) = parse_procedure(source.as_bytes());
    let program = parsed.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(max_steps) = evaluation_steps(source.as_bytes());

    let type4 = Type4 {
      program,
      max_steps,
      clamper: Clamper {
        domain: TupleVec::default(),
        range: None,
      },
    };

    let Ok(eval) = type4.eval(Values::new());
    let res = eval.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(res.as_slice(), out);

    Ok(())
  }

  #[test]
  fn op_abs() -> Result<(), crate::TestFailure> {
    op_impl("4.5 abs", &[4.5])?;
    op_impl("-3 abs", &[3.0])?;
    op_impl("0 abs", &[0.0])?;

    Ok(())
  }

  #[test]
  fn op_atan() -> Result<(), crate::TestFailure> {
    op_impl("0 1 atan", &[0.0])?;
    op_impl("1 0 atan", &[90.0])?;
    op_impl("-100 0 atan", &[270.0])?;
    op_impl("4 4 atan", &[45.0])?;

    Ok(())
  }

  #[test]
  fn op_ceiling() -> Result<(), crate::TestFailure> {
    op_impl("3.2 ceiling", &[4.0])?;
    op_impl("-4.8 ceiling", &[-4.0])?;
    op_impl("99 ceiling", &[99.0])?;

    Ok(())
  }

  #[test]
  fn op_cos() -> Result<(), crate::TestFailure> {
    op_impl("0 cos", &[1.0])?;

    op_impl("90 cos", &[-4.371139e-8])?;

    Ok(())
  }

  #[test]
  fn op_cvi() -> Result<(), crate::TestFailure> {
    op_impl("-47.8 cvi", &[-47.0])?;
    op_impl("520.9 cvi", &[520.0])?;

    Ok(())
  }

  #[test]
  fn op_cvr() -> Result<(), crate::TestFailure> {
    op_impl("-47.8 cvr", &[-47.8])?;
    op_impl("520 cvr", &[520.0])?;

    Ok(())
  }

  #[test]
  fn op_div() -> Result<(), crate::TestFailure> {
    op_impl("3 2 div", &[1.5])?;
    op_impl("4 2 div", &[2.0])?;

    Ok(())
  }

  #[test]
  fn op_exp() -> Result<(), crate::TestFailure> {
    op_impl("9 0.5 exp", &[3.0])?;
    op_impl("-9 -1 exp", &[-0.11111111])?;

    Ok(())
  }

  #[test]
  fn op_floor() -> Result<(), crate::TestFailure> {
    op_impl("3.2 floor", &[3.0])?;
    op_impl("-4.8 floor", &[-5.0])?;
    op_impl("99 floor", &[99.0])?;

    Ok(())
  }

  #[test]
  fn op_idiv() -> Result<(), crate::TestFailure> {
    op_impl("3 2 idiv", &[1.0])?;
    op_impl("4 2 idiv", &[2.0])?;
    op_impl("-5 2 idiv", &[-2.0])?;

    Ok(())
  }

  #[test]
  fn op_ln() -> Result<(), crate::TestFailure> {
    op_impl("10 ln", &[LN_10])?;
    op_impl("100 ln", &[4.6051702])?;

    Ok(())
  }

  #[test]
  fn op_log() -> Result<(), crate::TestFailure> {
    op_impl("10 log", &[1.0])?;
    op_impl("100 log", &[2.0])?;

    Ok(())
  }

  #[test]
  fn op_mod() -> Result<(), crate::TestFailure> {
    op_impl("5 3 mod", &[2.0])?;
    op_impl("5 2 mod", &[1.0])?;
    op_impl("-5 3 mod", &[-2.0])?;

    Ok(())
  }

  #[test]
  fn op_mul() -> Result<(), crate::TestFailure> {
    op_impl("5 3 mul", &[15.0])?;
    op_impl("-2 6 mul", &[-12.0])?;

    Ok(())
  }

  #[test]
  fn op_neg() -> Result<(), crate::TestFailure> {
    op_impl("4.5 neg", &[-4.5])?;
    op_impl("-3 neg", &[3.0])?;

    Ok(())
  }

  #[test]
  fn op_round() -> Result<(), crate::TestFailure> {
    op_impl("3.2 round", &[3.0])?;
    op_impl("6.5 round", &[7.0])?;
    op_impl("-4.8 round", &[-5.0])?;

    op_impl("99 round", &[99.0])?;

    Ok(())
  }

  #[test]
  fn op_sin() -> Result<(), crate::TestFailure> {
    op_impl("0.0 sin", &[0.0])?;
    op_impl("90.0 sin", &[1.0])?;

    Ok(())
  }

  #[test]
  fn op_sqrt() -> Result<(), crate::TestFailure> {
    op_impl("100 sqrt", &[10.0])?;

    Ok(())
  }

  #[test]
  fn op_sub() -> Result<(), crate::TestFailure> {
    op_impl("3 4 sub", &[-1.0])?;
    op_impl("6 0 sub", &[6.0])?;

    Ok(())
  }

  #[test]
  fn op_truncate() -> Result<(), crate::TestFailure> {
    op_impl("3.2 truncate", &[3.0])?;
    op_impl("-4.8 truncate", &[-4.0])?;
    op_impl("99 truncate", &[99.0])?;

    Ok(())
  }

  #[test]
  fn op_and() -> Result<(), crate::TestFailure> {
    op_impl("true true and", &[1.0])?;
    op_impl("true false and", &[0.0])?;
    op_impl("false true and", &[0.0])?;
    op_impl("false false and", &[0.0])?;
    op_impl("99 1 and", &[1.0])?;
    op_impl("52 7 and", &[4.0])?;

    Ok(())
  }

  #[test]
  fn op_bitshift() -> Result<(), crate::TestFailure> {
    op_impl("7 3 bitshift", &[56.0])?;
    op_impl("142 -3 bitshift", &[17.0])?;

    Ok(())
  }

  #[test]
  fn op_eq() -> Result<(), crate::TestFailure> {
    op_impl("4.0 4 eq", &[1.0])?;
    op_impl("-2.0 -3 eq", &[0.0])?;

    Ok(())
  }

  #[test]
  fn op_false() -> Result<(), crate::TestFailure> {
    op_impl("false", &[0.0])?;

    Ok(())
  }

  #[test]
  fn op_ge() -> Result<(), crate::TestFailure> {
    op_impl("4.2 4 ge", &[1.0])?;
    op_impl("4.2 4.2 ge", &[1.0])?;
    op_impl("4.2 6 ge", &[0.0])?;

    Ok(())
  }

  #[test]
  fn op_gt() -> Result<(), crate::TestFailure> {
    op_impl("4.2 4 gt", &[1.0])?;
    op_impl("4.2 4.2 gt", &[0.0])?;
    op_impl("4.2 6 gt", &[0.0])?;

    Ok(())
  }

  #[test]
  fn op_le() -> Result<(), crate::TestFailure> {
    op_impl("4.2 4 le", &[0.0])?;
    op_impl("4.2 4.2 le", &[1.0])?;
    op_impl("4.2 6 le", &[1.0])?;

    Ok(())
  }

  #[test]
  fn op_lt() -> Result<(), crate::TestFailure> {
    op_impl("4.2 4 lt", &[0.0])?;
    op_impl("4.2 4.2 lt", &[0.0])?;
    op_impl("4.2 6 lt", &[1.0])?;

    Ok(())
  }

  #[test]
  fn op_ne() -> Result<(), crate::TestFailure> {
    op_impl("3.0 3 ne", &[0.0])?;
    op_impl("3.0 3.0 ne", &[0.0])?;
    op_impl("3.0 3.1 ne", &[1.0])?;

    Ok(())
  }

  #[test]
  fn op_not() -> Result<(), crate::TestFailure> {
    op_impl("true not", &[0.0])?;
    op_impl("false not", &[1.0])?;
    op_impl("52 not", &[-53.0])?;

    Ok(())
  }

  #[test]
  fn op_or() -> Result<(), crate::TestFailure> {
    op_impl("true true or", &[1.0])?;
    op_impl("true false or", &[1.0])?;
    op_impl("false true or", &[1.0])?;
    op_impl("false false or", &[0.0])?;
    op_impl("17 5 or", &[21.0])?;

    Ok(())
  }

  #[test]
  fn op_xor() -> Result<(), crate::TestFailure> {
    op_impl("true true xor", &[0.0])?;
    op_impl("true false xor", &[1.0])?;
    op_impl("false true xor", &[1.0])?;
    op_impl("false false xor", &[0.0])?;
    op_impl("7 3 xor", &[4.0])?;
    op_impl("12 3 xor", &[15.0])?;

    Ok(())
  }

  #[test]
  fn op_if() -> Result<(), crate::TestFailure> {
    op_impl("true { 1.0 } if", &[1.0])?;
    op_impl("false { 1.0 } if", &[])?;

    Ok(())
  }

  #[test]
  fn op_ifelse() -> Result<(), crate::TestFailure> {
    op_impl("true { 1.0 } { 2.0 } ifelse", &[1.0])?;
    op_impl("false { 1.0 } { 2.0 } ifelse", &[2.0])?;

    Ok(())
  }

  #[test]
  fn nested_procedures_return_to_where_they_were_entered() -> Result<(), crate::TestFailure> {
    op_impl("true { true { 3 } if 4 } if 5", &[3.0, 4.0, 5.0])?;
    op_impl(
      "false { 1 } { true { 2 } { 3 } ifelse 4 } ifelse 5",
      &[2.0, 4.0, 5.0],
    )?;
    op_impl("true { false { 1 } if } if 2", &[2.0])?;

    Ok(())
  }

  #[test]
  fn integer_division_with_no_quotient_is_rejected() -> Result<(), crate::TestFailure> {
    let Ok(parsed) = parse_procedure(b"{ 1 0 idiv }");
    let by_zero = parsed.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(mut stack) = InterpreterStack::new();

    assert_eq!(eval_inner(&by_zero, &mut stack, 128), Ok(None));

    let Ok(parsed_2) = parse_procedure(b"{ -2147483648 -1 idiv }");
    let past_the_end = parsed_2.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(mut stack_2) = InterpreterStack::new();

    assert_eq!(eval_inner(&past_the_end, &mut stack_2, 128), Ok(None));

    Ok(())
  }

  #[test]
  fn unclosed_procedure_is_rejected() {
    assert_eq!(parse_procedure(b"{ true { 1 } if"), Ok(None));
    assert_eq!(parse_procedure(b"{ { 1 }"), Ok(None));
  }

  #[test]
  fn op_copy() -> Result<(), crate::TestFailure> {
    op_impl("1 2 3 2 copy", &[1.0, 2.0, 3.0, 2.0, 3.0])?;
    op_impl("1 2 3 0 copy", &[1.0, 2.0, 3.0])?;

    Ok(())
  }

  #[test]
  fn op_dup() -> Result<(), crate::TestFailure> {
    op_impl("1 2 3 dup", &[1.0, 2.0, 3.0, 3.0])?;

    Ok(())
  }

  #[test]
  fn op_exch() -> Result<(), crate::TestFailure> {
    op_impl("1 2 3 exch", &[1.0, 3.0, 2.0])?;

    Ok(())
  }

  #[test]
  fn op_index() -> Result<(), crate::TestFailure> {
    op_impl("1 2 3 4 0 index", &[1.0, 2.0, 3.0, 4.0, 4.0])?;
    op_impl("1 2 3 4 3 index", &[1.0, 2.0, 3.0, 4.0, 1.0])?;

    Ok(())
  }

  #[test]
  fn op_pop() -> Result<(), crate::TestFailure> {
    op_impl("1 2 3 pop", &[1.0, 2.0])?;

    Ok(())
  }

  #[test]
  fn op_roll() -> Result<(), crate::TestFailure> {
    op_impl("1 2 3 3 -1 roll", &[2.0, 3.0, 1.0])?;
    op_impl("1 2 3 3 1 roll", &[3.0, 1.0, 2.0])?;
    op_impl("1 2 3 3 0 roll", &[1.0, 2.0, 3.0])?;
    op_impl("1 2 3 3 5 roll", &[2.0, 3.0, 1.0])?;
    op_impl("0 2 roll", &[])?;
    op_impl(
      "1 2 3 4 5 6 7 5 2 roll",
      &[1.0, 2.0, 6.0, 7.0, 3.0, 4.0, 5.0],
    )?;

    Ok(())
  }

  #[test]
  fn domain() -> Result<(), crate::TestFailure> {
    let source = b"{  }";
    let Ok(parsed) = parse_procedure(source);
    let program = parsed.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(max_steps) = evaluation_steps(source);

    let Ok(type4) = Function::compiled(FunctionType::Type4(Type4 {
      program,
      max_steps,
      clamper: Clamper {
        domain: vec![(-5.0, 5.0), (-5.0, 5.0), (-5.0, 5.0)],
        range: None,
      },
    }));

    let input = vec![-10.0, -2.0, 6.0];
    let Ok(eval) = type4.eval(input);
    let res = eval.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(res.as_slice(), &[-5.0, -2.0, 5.0]);

    Ok(())
  }
}
