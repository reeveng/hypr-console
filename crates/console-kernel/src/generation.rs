//! Native session for an immutable program set supplied by the image build.

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_kernel::events::KeyEdge;
use console_kernel::interrupts::Address;
use console_kernel::usb_keyboard::{BootReport, Shift, character};
use core::fmt;

use crate::acpi::Root;
use crate::input::{self, Action, line};
use crate::userspace::{self, Reserved};
use crate::xhci::{Error, Machine};

#[cfg(console_generation)]
pub(super) const CALCULATOR: Option<&[u8]> =
  Some(include_bytes!(concat!(env!("OUT_DIR"), "/calculator.bin")));
#[cfg(not(console_generation))]
pub(super) const CALCULATOR: Option<&[u8]> = None;

const ESCAPE: u16 = 0x29;
const RETURN: u16 = 0x28;
const BACKSPACE: u16 = 0x2a;
const C: u16 = 0x06;

enum View {
  Launcher,
  Calculator,
}

struct Session<'a> {
  view: View,
  expression: Expression,
  program: &'static [u8],
  memory: &'a mut Reserved,
  stack: Address,
}

pub(super) fn run(
  root: Option<Root>,
  machine: Machine,
  memory: &mut Reserved,
  stack: Address,
  calculator: &'static [u8],
) -> Result<(), Error> {
  let session = Session {
    view: View::Launcher,
    expression: Expression::EMPTY,
    program: calculator,
    memory,
    stack,
  };
  let Ok(()) = launcher(calculator);

  input::listen(root, machine, session, Session::key, || {
    line(format_args!("Console keyboard ready"))
  })
}

fn launcher(calculator: &[u8]) -> Result<(), Never> {
  let Ok(()) = line(format_args!("Console launcher"));

  match calculator {
    [_, ..] => line(format_args!("C: Calculator | Esc: Exit")),
    [] => line(format_args!("Esc: Exit")),
  }
}

impl Session<'_> {
  fn key(&mut self, code: u16, edge: KeyEdge, report: BootReport) -> Result<Action, Never> {
    match (edge, &self.view, code) {
      (KeyEdge::Release, _, _) => {}
      (KeyEdge::Press, View::Launcher, ESCAPE) => return Ok(Action::Exit),
      (KeyEdge::Press, View::Launcher, C) => {
        match ::core::primitive::bool::then_some(self.program.is_empty(), ()) {
          ::core::option::Option::None => {
            self.view = View::Calculator;
            let Ok(()) = line(format_args!(
              "Calculator opened. Type 12+34 and press Enter. Esc returns."
            ));
          }
          ::core::option::Option::Some(()) => {}
        }
      }
      (KeyEdge::Press, View::Launcher, _) => {}
      (KeyEdge::Press, View::Calculator, ESCAPE) => {
        self.expression = Expression::EMPTY;
        self.view = View::Launcher;
        let Ok(()) = launcher(self.program);
      }
      (KeyEdge::Press, View::Calculator, RETURN) => {
        let Ok(()) = self.evaluate();
        self.expression = Expression::EMPTY;
      }
      (KeyEdge::Press, View::Calculator, BACKSPACE) => {
        let Ok(()) = self.expression.erase();
        let Ok(()) = line(format_args!("{}", self.expression));
      }
      (KeyEdge::Press, View::Calculator, _) => {
        let Ok(shift) = report.shifted();
        let Ok(letter) = character(code, shift);
        let input = match (code, shift, letter) {
          (0x2d, Shift::Clear, _) => Some(b'-'),
          (0x2e, Shift::Pressed, _) => Some(b'+'),
          (0x38, Shift::Clear, _) => Some(b'/'),
          (_, _, Some(digit_or_star @ ('0'..='9' | '*'))) => {
            let Ok(byte) = wrapped::<_, u8>(u32::from(digit_or_star));
            Some(byte)
          }
          _ => None,
        };

        match input {
          Some(byte) => match self.expression.append(byte) {
            Ok(Insertion::Stored) => {
              let Ok(()) = line(format_args!("{}", self.expression));
            }
            Ok(Insertion::Full) => {
              let Ok(()) = line(format_args!("Expression is full. Press Backspace."));
            }
          },
          None => {}
        }
      }
    }

    Ok(Action::Continue)
  }

  fn evaluate(&mut self) -> Result<(), Never> {
    let request = match self.expression.text().and_then(request) {
      Ok(request) => request,
      Err(InvalidExpression::Operator | InvalidExpression::Operand) => {
        return line(format_args!(
          "Enter two whole numbers from 0 to 65535 with +, -, * or /."
        ));
      }
    };

    match userspace::calculate(self.memory, self.stack, self.program, request) {
      Ok(Some(answer)) => line(format_args!("{} = {answer}", self.expression)),
      Ok(None) => line(format_args!("{} = Error", self.expression)),
      Err(error) => line(format_args!("Calculator could not run: {error}")),
    }
  }
}

struct Expression {
  bytes: [u8; 11],
  length: u8,
}

enum Insertion {
  Stored,
  Full,
}

impl Expression {
  const EMPTY: Self = Self {
    bytes: [0; 11],
    length: 0,
  };

  fn append(&mut self, byte: u8) -> Result<Insertion, Never> {
    let Ok(end) = index(self.length);

    match self.bytes.get_mut(end) {
      Some(slot) => {
        *slot = byte;
        self.length = self.length.saturating_add(1);
        Ok(Insertion::Stored)
      }
      None => Ok(Insertion::Full),
    }
  }

  fn erase(&mut self) -> Result<(), Never> {
    self.length = self.length.saturating_sub(1);
    Ok(())
  }

  fn text(&self) -> Result<&str, InvalidExpression> {
    let Ok(end) = index(self.length);
    let initialized = self.bytes.get(..end).ok_or(InvalidExpression::Operand)?;

    core::str::from_utf8(initialized).map_err(|_| InvalidExpression::Operand)
  }
}

impl fmt::Display for Expression {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    let text = self.text().map_err(|_| fmt::Error)?;
    out.write_str(text)
  }
}

enum InvalidExpression {
  Operator,
  Operand,
}

fn request(expression: &str) -> Result<[u8; 8], InvalidExpression> {
  let found = expression
    .find(['+', '-', '*', '/'])
    .map(fixed_index)
    .ok_or(InvalidExpression::Operator)?;
  let Ok(operator_offset) = found;
  let Ok(operator_index) = index(operator_offset);
  let (left, remainder) = expression
    .split_at_checked(operator_index)
    .ok_or(InvalidExpression::Operator)?;
  let (operator, right) = remainder
    .split_at_checked(1)
    .ok_or(InvalidExpression::Operator)?;
  let opcode = operator
    .as_bytes()
    .first()
    .copied()
    .ok_or(InvalidExpression::Operator)?;
  let left_operand = operand(left)?;
  let right_operand = operand(right)?;
  let [left_lo, left_hi] = left_operand.to_le_bytes();
  let [right_lo, right_hi] = right_operand.to_le_bytes();

  Ok([left_lo, left_hi, right_lo, right_hi, opcode, 0, 0, 0])
}

fn operand(text: &str) -> Result<u16, InvalidExpression> {
  match text.bytes().all(|byte| byte.is_ascii_digit()).then_some(()) {
    Some(()) => text.parse().map_err(|_| InvalidExpression::Operand),
    None => Err(InvalidExpression::Operand),
  }
}
