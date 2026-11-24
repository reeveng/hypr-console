//! Lowers the scalar subset accepted by `compute_kernel!`.
//! The statement form `match (condition).then_some(())` accepts a `Some(())`
//! block followed by a `None` block and evaluates the condition once.

use std::fmt::{self, Write as _};
use std::io;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

const ACCESS: &str = r#"
layout(push_constant) uniform Dispatch { uint params[8]; };
layout(set = 0, binding = 0, std430) buffer Arena { uint words[]; } arena;

uint console_id(uint i) { return i < 3u ? gl_GlobalInvocationID[i] : 0u; }
uint console_param(uint i) { return i < 8u ? params[i] : 0u; }
uint console_word(uint i) { return i < uint(arena.words.length()) ? arena.words[i] : 0u; }
void console_set_word(uint i, uint x) { if (i < uint(arena.words.length())) arena.words[i] = x; }
bool console_not(bool x) { return !x; }
int console_not(int x) { return ~x; }
uint console_not(uint x) { return ~x; }
uint console_add(uint a, uint b) { return a + b; }
uint console_sub(uint a, uint b) { return a - b; }
uint console_mul(uint a, uint b) { return a * b; }
uint console_div(uint a, uint b) { return a / b; }
uint console_rem(uint a, uint b) { return a % b; }
uint console_shl(uint value, uint shift) { return value << (shift & 31u); }
uint console_shr(uint value, uint shift) { return value >> (shift & 31u); }
int console_add(int a, int b) { return int(uint(a) + uint(b)); }
int console_sub(int a, int b) { return int(uint(a) - uint(b)); }
int console_mul(int a, int b) { return int(uint(a) * uint(b)); }
int console_shr(int value, uint shift) { return value >> (shift & 31u); }
int console_rem_euclid(int value, int divisor) {
    int remainder = value - (value / divisor) * divisor;
    return remainder < 0 ? (divisor < 0 ? remainder - divisor : remainder + divisor) : remainder;
}
"#;

const MAP_ACCESS: &str = r#"
uint console_extent(uint axis) { return axis < 3u ? params[axis] : 0u; }
uint console_index() {
    uvec3 id = gl_GlobalInvocationID;
    return id.x + params[0] * (id.y + params[1] * id.z);
}
uint console_input_len() { return params[4]; }
uint console_map_param(uint i) { return i < 2u ? params[6u + i] : 0u; }
uint console_map_word(uint i) { return i < params[4] ? console_word(params[3] + i) : 0u; }
"#;

const MAP_ENTRY: &str = r#"
void main() {
    if (any(greaterThanEqual(gl_GlobalInvocationID, uvec3(params[0], params[1], params[2]))))
        return;
    console_set_word(params[5] + console_index(), console_item());
}
"#;

const RANGE_END: &str = "console_range_end";

#[derive(Clone, Copy)]
pub(super) enum Dialect {
  Batch,
  Map,
}

pub(super) struct Source<'a> {
  pub(super) binding: &'a str,
  pub(super) body: &'a str,
}

enum Fault {
  Invalid(&'static str),
  Unexpected(String),
  Formatting(fmt::Error),
}

impl From<Fault> for io::Error {
  fn from(fault: Fault) -> Self {
    match fault {
      Fault::Invalid(message) => Self::new(io::ErrorKind::InvalidInput, message),
      Fault::Unexpected(message) => Self::new(io::ErrorKind::InvalidInput, message),
      Fault::Formatting(error) => Self::other(error),
    }
  }
}

impl From<fmt::Error> for Fault {
  fn from(error: fmt::Error) -> Self {
    Self::Formatting(error)
  }
}

pub(super) fn glsl(
  workgroup: [u32; 3],
  source: Source<'_>,
  dialect: Dialect,
) -> io::Result<String> {
  lower(workgroup, source, dialect).map_err(io::Error::from)
}

fn lower([x, y, z]: [u32; 3], source: Source<'_>, dialect: Dialect) -> Result<String, Fault> {
  let positive = x > 0 && y > 0 && z > 0;

  match bool::then_some(positive, ()) {
    Some(()) => {}
    None => return Err(Fault::Invalid("workgroup dimensions must be positive")),
  }

  let tokens = tokenize(source.body)?;
  let mut parser = Parser {
    tokens: &tokens,
    binding: source.binding,
    dialect,
    expression_steps: Vec::new(),
  };
  let mut shader = format!(
    "#version 450\nlayout(local_size_x = {x}, local_size_y = {y}, local_size_z = {z}) in;\n{ACCESS}\n"
  );

  let body = match dialect {
    Dialect::Batch => {
      shader.push_str("void main() ");
      Body::Statements
    }
    Dialect::Map => {
      shader.push_str(MAP_ACCESS);
      shader.push_str("uint console_item() ");
      Body::TailValue
    }
  };
  parser.body(&mut shader, body)?;

  match parser.tokens {
    [] => {}
    [_, ..] => return Err(Fault::Invalid("expected one kernel body")),
  }

  match dialect {
    Dialect::Batch => {}
    Dialect::Map => shader.push_str(MAP_ENTRY),
  }

  Ok(shader)
}

fn tokenize(mut source: &str) -> Result<Vec<&str>, Fault> {
  let mut tokens = Vec::new();

  for _source_byte in source.bytes() {
    source = source.trim_start_matches(|character: char| character.is_ascii_whitespace());
    let first = match source.as_bytes() {
      [first, ..] => *first,
      [] => return Ok(tokens),
    };
    let width = token_width(source, first)?;
    let Ok(position) = index(width);
    let (token, rest) = source
      .split_at_checked(position)
      .ok_or(Fault::Invalid("invalid UTF-8 kernel token boundary"))?;

    match token {
      RANGE_END => return Err(Fault::Invalid("reserved kernel range name")),
      _ => {}
    }

    tokens.push(token);
    source = rest;
  }

  Ok(tokens)
}

fn token_width(source: &str, first: u8) -> Result<u64, Fault> {
  const TWO: &[&str] = &[
    "::", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "+=", "-=", "*=", "/=", "%=", "&=",
    "|=", "^=", "..", "=>",
  ];
  let letter = first.is_ascii_alphabetic() || first == b'_';
  let Ok(width) = match (
    bool::then_some(first.is_ascii_digit(), ()),
    bool::then_some(letter, ()),
  ) {
    (Some(()), _) => number_width(source),
    (None, Some(())) => fixed_index(
      source
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        .count(),
    ),
    (None, None) => {
      let two_characters = TWO.iter().any(|&op| source.starts_with(op));
      return match (
        bool::then_some(two_characters, ()),
        bool::then_some(b"{}();:,.=+-*/%<>!&|^".contains(&first), ()),
      ) {
        (Some(()), _) => Ok(2),
        (None, Some(())) => Ok(1),
        (None, None) => Err(Fault::Unexpected(format!(
          "unsupported kernel token near {source:?}"
        ))),
      };
    }
  };
  Ok(width)
}

fn number_width(source: &str) -> Result<u64, Never> {
  let decimal =
    !source.starts_with("0x") && !source.starts_with("0b") && !source.starts_with("0o");

  fixed_index(
    source
      .bytes()
      .zip(source.bytes().skip(1).chain([0]))
      .scan(0, |previous, (byte, next)| {
        let exponent_sign =
          decimal && matches!(*previous, b'e' | b'E') && matches!(byte, b'+' | b'-');
        *previous = byte;
        let decimal_point = byte == b'.' && next != b'.';
        let accepted =
          byte.is_ascii_alphanumeric() || byte == b'_' || decimal_point || exponent_sign;
        bool::then_some(accepted, ())
      })
      .count(),
  )
}

#[derive(Clone, Copy)]
enum Body {
  Statements,
  TailValue,
}

enum Statement {
  Body(Body),
  Else,
  MatchAlternative,
  EndMatch,
}

struct Scalar(&'static str);

fn scalar_type(format: &str) -> Result<Scalar, Fault> {
  let name = match format {
    "u32" => "uint",
    "i32" => "int",
    "f32" => "precise float",
    "bool" => "bool",
    other => {
      return Err(Fault::Unexpected(format!(
        "unsupported kernel scalar type {other:?}"
      )));
    }
  };
  Ok(Scalar(name))
}

enum Expression<'a> {
  Read {
    minimum: u8,
  },
  Primary,
  Continue {
    minimum: u8,
  },
  Binary {
    minimum: u8,
    left: String,
    operator: &'a str,
  },
  Not,
  Negate,
  Group,
  Argument {
    call: String,
  },
}

struct Parser<'a> {
  tokens: &'a [&'a str],
  binding: &'a str,
  dialect: Dialect,
  expression_steps: Vec<Expression<'a>>,
}

impl<'a> Parser<'a> {
  fn next(&mut self) -> Result<&'a str, Fault> {
    let (first, rest) = self
      .tokens
      .split_first()
      .ok_or(Fault::Invalid("incomplete kernel body"))?;
    self.tokens = rest;
    Ok(first)
  }

  fn consume(&mut self, value: &str) -> Result<Option<()>, Never> {
    match self.tokens {
      [first, rest @ ..] => match bool::then_some(*first == value, ()) {
        Some(()) => {
          self.tokens = rest;
          Ok(Some(()))
        }
        None => Ok(None),
      },
      [] => Ok(None),
    }
  }

  fn require(&mut self, value: &str) -> Result<(), Fault> {
    let token = self.next()?;

    match bool::then_some(token == value, ()) {
      Some(()) => Ok(()),
      None => Err(Fault::Unexpected(format!(
        "expected {value:?}, found {token:?} in kernel"
      ))),
    }
  }

  fn begin_body(&mut self, out: &mut String) -> Result<(), Fault> {
    self.require("{")?;
    out.push_str("{\n");
    Ok(())
  }

  fn body(&mut self, out: &mut String, body: Body) -> Result<(), Fault> {
    self.begin_body(out)?;
    let mut pending = vec![Statement::Body(body)];

    for _step in self.tokens.iter().flat_map(|_| [(), ()]) {
      match pending.pop() {
        Some(Statement::Body(current)) => {
          let Ok(ending) = self.consume("}");

          match ending {
            Some(()) => out.push_str("}\n"),
            None => {
              pending.push(Statement::Body(current));
              self.statement(out, current, &mut pending)?;
            }
          }
        }
        Some(Statement::Else) => self.alternative(out, &mut pending)?,
        Some(Statement::MatchAlternative) => self.match_alternative(out, &mut pending)?,
        Some(Statement::EndMatch) => {
          let Ok(_comma) = self.consume(",");
          self.require("}")?;
          let Ok(_semicolon) = self.consume(";");
        }
        None => return Ok(()),
      }
    }

    Err(Fault::Invalid("incomplete kernel body"))
  }

  fn alternative(&mut self, out: &mut String, pending: &mut Vec<Statement>) -> Result<(), Fault> {
    let Ok(alternative) = self.consume("else");

    match alternative {
      Some(()) => {
        out.push_str("else ");

        match self.tokens {
          ["if", ..] => self.control(out, pending)?,
          _ => {
            self.begin_body(out)?;
            pending.push(Statement::Body(Body::Statements));
          }
        }
      }
      None => {}
    }

    Ok(())
  }

  fn control(&mut self, out: &mut String, pending: &mut Vec<Statement>) -> Result<(), Fault> {
    let kind = self.next()?;
    let condition = self.expression()?;
    write!(out, "{kind} ({condition}) ")?;
    self.begin_body(out)?;

    match kind {
      "if" => pending.push(Statement::Else),
      _ => {}
    }

    pending.push(Statement::Body(Body::Statements));
    Ok(())
  }

  fn range_loop(&mut self, out: &mut String, pending: &mut Vec<Statement>) -> Result<(), Fault> {
    self.require("for")?;
    let index = self.next()?;
    self.require("in")?;
    let start_token = self.next()?;

    match (start_token.as_bytes(), start_token.strip_suffix("u32")) {
      ([b'0'..=b'9', ..], Some(_)) => {}
      _ => return Err(Fault::Invalid("kernel range must start at a u32 literal")),
    }

    self.require("..")?;
    let start = literal(start_token)?;
    let end = self.expression()?;
    write!(
      out,
      "for (uint {RANGE_END} = uint({end}), {index} = {start}; {index} < {RANGE_END}; ++{index}) "
    )?;
    self.begin_body(out)?;
    pending.push(Statement::Body(Body::Statements));

    Ok(())
  }

  fn conditional_match(
    &mut self,
    out: &mut String,
    pending: &mut Vec<Statement>,
  ) -> Result<(), Fault> {
    self.require("match")?;
    self.require("(")?;
    let condition = self.expression()?;
    self.require(")")?;

    for token in [
      ".",
      "then_some",
      "(",
      "(",
      ")",
      ")",
      "{",
      "Some",
      "(",
      "(",
      ")",
      ")",
      "=>",
    ] {
      self.require(token)?;
    }

    write!(out, "if ({condition}) ")?;
    self.begin_body(out)?;
    pending.push(Statement::MatchAlternative);
    pending.push(Statement::Body(Body::Statements));

    Ok(())
  }

  fn match_alternative(
    &mut self,
    out: &mut String,
    pending: &mut Vec<Statement>,
  ) -> Result<(), Fault> {
    let Ok(_comma) = self.consume(",");
    self.require("None")?;
    self.require("=>")?;
    out.push_str("else ");
    self.begin_body(out)?;
    pending.push(Statement::EndMatch);
    pending.push(Statement::Body(Body::Statements));

    Ok(())
  }

  fn binding(&mut self, out: &mut String) -> Result<(), Fault> {
    self.require("let")?;
    let Ok(success) = self.consume("Ok");

    match success {
      Some(()) => self.result_binding(out),
      None => {
        let Ok(_mutable) = self.consume("mut");
        let name = self.next()?;
        self.require(":")?;
        let given = self.next()?;
        let scalar = scalar_type(given)?;
        self.write_binding(out, name, scalar)
      }
    }
  }

  fn result_binding(&mut self, out: &mut String) -> Result<(), Fault> {
    self.require("(")?;
    let Ok(unit) = self.consume("(");

    match unit {
      Some(()) => {
        for token in [")", ")", "="] {
          self.require(token)?;
        }

        let call = self.expression()?;
        self.require(";")?;
        writeln!(out, "{call};")?;
        Ok(())
      }
      None => {
        let Ok(_mutable) = self.consume("mut");
        let name = self.next()?;

        for token in [")", ":", "Result", "<"] {
          self.require(token)?;
        }

        let given = self.next()?;
        let scalar = scalar_type(given)?;

        for token in [",", "Never", ">"] {
          self.require(token)?;
        }

        self.write_binding(out, name, scalar)
      }
    }
  }

  fn write_binding(&mut self, out: &mut String, name: &str, scalar: Scalar) -> Result<(), Fault> {
    self.require("=")?;
    let value = self.expression()?;
    self.require(";")?;
    writeln!(out, "{} {name} = {value};", scalar.0)?;
    Ok(())
  }

  fn statement(
    &mut self,
    out: &mut String,
    body: Body,
    pending: &mut Vec<Statement>,
  ) -> Result<(), Fault> {
    match (self.tokens, self.dialect) {
      (["let", ..], _) => self.binding(out)?,
      (["if" | "while", ..], _) => self.control(out, pending)?,
      (["for", ..], _) => self.range_loop(out, pending)?,
      (["match", ..], _) => self.conditional_match(out, pending)?,
      (["return", ..], Dialect::Map) => {
        self.next()?;
        let value = self.expression()?;
        self.require(";")?;
        writeln!(out, "return {value};")?;
      }
      (["return" | "break" | "continue", ..], _) => {
        let kind = self.next()?;
        self.require(";")?;
        writeln!(out, "{kind};")?;
      }
      _ => {
        let left = self.expression()?;

        match (body, self.tokens) {
          (Body::TailValue, ["}", ..]) => {
            writeln!(out, "return {left};")?;
            return Ok(());
          }
          (Body::Statements, _) | (Body::TailValue, _) => {}
        }

        out.push_str(&left);

        match self.tokens {
          [
            "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=",
            ..,
          ] => {
            let operator = self.next()?;
            let right = self.expression()?;
            write!(out, " {operator} {right}")?;
          }
          _ => {}
        }

        self.require(";")?;
        out.push_str(";\n");
      }
    }

    Ok(())
  }

  fn expression(&mut self) -> Result<String, Fault> {
    let mut pending = std::mem::take(&mut self.expression_steps);
    pending.push(Expression::Read { minimum: 1 });
    let mut value = String::new();

    for _step in self.tokens.iter().flat_map(|_| [(); 6]) {
      match pending.pop() {
        Some(Expression::Read { minimum }) => {
          pending.push(Expression::Continue { minimum });
          pending.push(Expression::Primary);
        }
        Some(Expression::Primary) => self.primary(&mut pending, &mut value)?,
        Some(Expression::Continue { minimum }) => {
          self.continue_expression(minimum, &mut pending, &mut value)?;
        }
        Some(Expression::Binary {
          minimum,
          left,
          operator,
        }) => {
          value = format!("({left} {operator} {value})");
          pending.push(Expression::Continue { minimum });
        }
        Some(Expression::Not) => value = format!("console_not({value})"),
        Some(Expression::Negate) => value = format!("(-{value})"),
        Some(Expression::Group) => self.require(")")?,
        Some(Expression::Argument { mut call }) => {
          call.push_str(&value);
          let Ok(separator) = self.consume(",");

          match (separator, self.tokens) {
            (Some(()), [")", ..]) | (None, _) => {
              self.require(")")?;
              call.push(')');
              value = call;
            }
            (Some(()), _) => {
              call.push_str(", ");
              pending.push(Expression::Argument { call });
              pending.push(Expression::Read { minimum: 1 });
            }
          }
        }
        None => {
          self.expression_steps = pending;
          return Ok(value);
        }
      }
    }

    Err(Fault::Invalid("incomplete kernel expression"))
  }

  fn continue_expression(
    &mut self,
    minimum: u8,
    pending: &mut Vec<Expression<'a>>,
    value: &mut String,
  ) -> Result<(), Fault> {
    let operator = match self.tokens {
      [operator, ..] => *operator,
      [] => return Ok(()),
    };
    let priority: u8 = match operator {
      "||" => 1,
      "&&" => 2,
      "==" | "!=" | "<" | ">" | "<=" | ">=" => 3,
      "|" => 4,
      "^" => 5,
      "&" => 6,
      "<<" | ">>" => 7,
      "+" | "-" => 8,
      "*" | "/" | "%" => 9,
      "as" => 10,
      _ => return Ok(()),
    };

    match bool::then_some(priority >= minimum, ()) {
      Some(()) => {}
      None => return Ok(()),
    }

    self.next()?;

    match operator {
      "as" => {
        let scalar_type = self.next()?;
        let constructor = match scalar_type {
          "u32" => "uint",
          "i32" => "int",
          "f32" => "float",
          ty => {
            return Err(Fault::Unexpected(format!(
              "unsupported kernel cast to {ty:?}"
            )));
          }
        };
        *value = format!("{constructor}({value})");
        pending.push(Expression::Continue { minimum });
      }
      _ => {
        pending.push(Expression::Binary {
          minimum,
          left: std::mem::take(value),
          operator,
        });
        pending.push(Expression::Read {
          minimum: priority.saturating_add(1),
        });
      }
    }

    Ok(())
  }

  fn primary(
    &mut self,
    pending: &mut Vec<Expression<'a>>,
    value: &mut String,
  ) -> Result<(), Fault> {
    let token = self.next()?;

    match token {
      "(" => {
        pending.push(Expression::Group);
        pending.push(Expression::Read { minimum: 1 });
      }
      "!" => {
        pending.push(Expression::Not);
        pending.push(Expression::Primary);
      }
      "-" => {
        pending.push(Expression::Negate);
        pending.push(Expression::Primary);
      }
      "WordAt" => self.begin_call("uint", pending, value)?,
      "::" => {
        self.require("core")?;
        self.require("::")?;
        self.require("primitive")?;
        self.require("::")?;
        let scalar = self.next()?;
        let provided = self.scalar_function(scalar)?;
        let callee = provided.ok_or(Fault::Invalid("expected a portable primitive operation"))?;
        self.begin_call(callee, pending, value)?;
      }
      name => {
        let function = match bool::then_some(name == self.binding, ()) {
          Some(()) => {
            self.require(".")?;
            let field = self.next()?;
            let accessor = self.accessor(field)?;
            Some(accessor)
          }
          None => self.scalar_function(name)?,
        };

        match function {
          Some(callee) => self.begin_call(callee, pending, value)?,
          None => {
            let literal = atom(name)?;
            *value = literal;
          }
        }
      }
    }

    Ok(())
  }

  fn begin_call(
    &mut self,
    function: &str,
    pending: &mut Vec<Expression<'a>>,
    value: &mut String,
  ) -> Result<(), Fault> {
    self.require("(")?;
    let Ok(ending) = self.consume(")");
    let mut call = format!("{function}(");

    match ending {
      Some(()) => {
        call.push(')');
        *value = call;
      }
      None => {
        pending.push(Expression::Argument { call });
        pending.push(Expression::Read { minimum: 1 });
      }
    }

    Ok(())
  }

  fn accessor(&self, field: &str) -> Result<&'static str, Fault> {
    match (self.dialect, field) {
      (_, "id") => Ok("console_id"),
      (Dialect::Map, "extent") => Ok("console_extent"),
      (Dialect::Map, "index") => Ok("console_index"),
      (Dialect::Map, "input_len") => Ok("console_input_len"),
      (Dialect::Map, "param") => Ok("console_map_param"),
      (Dialect::Map, "word") => Ok("console_map_word"),
      (Dialect::Batch, "param") => Ok("console_param"),
      (Dialect::Batch, "word") => Ok("console_word"),
      (Dialect::Batch, "set_word") => Ok("console_set_word"),
      (_, unknown) => Err(Fault::Unexpected(format!(
        "unknown kernel invocation accessor {unknown:?}"
      ))),
    }
  }

  fn scalar_function(&mut self, scalar: &str) -> Result<Option<&'static str>, Fault> {
    match (scalar, self.tokens) {
      ("u32", ["::", ..]) => {
        self.next()?;
        let operation = self.next()?;

        match operation {
          "wrapping_add" => Ok(Some("console_add")),
          "wrapping_sub" => Ok(Some("console_sub")),
          "wrapping_mul" => Ok(Some("console_mul")),
          "wrapping_div" => Ok(Some("console_div")),
          "wrapping_rem" => Ok(Some("console_rem")),
          "wrapping_shl" => Ok(Some("console_shl")),
          "wrapping_shr" => Ok(Some("console_shr")),
          "max" => Ok(Some("max")),
          "cast_signed" => Ok(Some("int")),
          _ => Err(Fault::Unexpected(format!(
            "unsupported kernel integer operation {operation:?}"
          ))),
        }
      }
      ("i32", ["::", ..]) => {
        self.next()?;
        let operation = self.next()?;

        match operation {
          "wrapping_add" => Ok(Some("console_add")),
          "wrapping_sub" => Ok(Some("console_sub")),
          "wrapping_mul" => Ok(Some("console_mul")),
          "wrapping_shr" => Ok(Some("console_shr")),
          "rem_euclid" => Ok(Some("console_rem_euclid")),
          "clamp" => Ok(Some("clamp")),
          "cast_unsigned" => Ok(Some("uint")),
          _ => Err(Fault::Unexpected(format!(
            "unsupported kernel signed integer operation {operation:?}"
          ))),
        }
      }
      ("f32", ["::", ..]) => {
        self.next()?;
        let operation = self.next()?;

        match operation {
          "to_bits" => Ok(Some("floatBitsToUint")),
          "from_bits" => Ok(Some("uintBitsToFloat")),
          _ => Err(Fault::Unexpected(format!(
            "unsupported kernel float operation {operation:?}"
          ))),
        }
      }
      _ => Ok(None),
    }
  }
}

fn atom(token: &str) -> Result<String, Fault> {
  match token.as_bytes() {
    [b'0'..=b'9', ..] => literal(token),
    [b'A'..=b'Z' | b'a'..=b'z' | b'_', ..] => Ok(token.to_owned()),
    _ => Err(Fault::Unexpected(format!(
      "unsupported expression starting with {token:?}"
    ))),
  }
}

fn literal(number: &str) -> Result<String, Fault> {
  let normalized = number.replace('_', "");
  let (digits, suffix) = match (
    normalized.strip_suffix("u32"),
    normalized.strip_suffix("i32"),
    normalized.strip_suffix("f32"),
  ) {
    (Some(value), _, _) => (value, "u"),
    (None, Some(value), _) => (value, ""),
    (None, None, Some(value)) => {
      return Ok(match bool::then_some(value.contains(['.', 'e', 'E']), ()) {
        Some(()) => value.to_owned(),
        None => format!("{value}.0"),
      });
    }
    (None, None, None) => (normalized.as_str(), ""),
  };

  match (digits.strip_prefix("0b"), digits.strip_prefix("0o")) {
    (Some(binary), _) => {
      let value = u32::from_str_radix(binary, 2)
        .map_err(|_| Fault::Invalid("invalid binary kernel literal"))?;
      Ok(format!("{value}{suffix}"))
    }
    (None, Some(octal)) => {
      let value = u32::from_str_radix(octal, 8)
        .map_err(|_| Fault::Invalid("invalid octal kernel literal"))?;
      Ok(format!("{value}{suffix}"))
    }
    (None, None) => Ok(format!("{digits}{suffix}")),
  }
}
