//! Native session for an immutable program set supplied by the image build.

use alloc::string::String;
use console_core_never::Never;
use console_kernel::events::KeyEdge;
use console_kernel::interrupts::Address;
use console_kernel::usb_keyboard::{BootReport, Shift, character};

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
const EXPRESSION_LENGTH: usize = 11;

enum View {
  Launcher,
  Calculator,
}

struct Session<'a> {
  view: View,
  expression: String,
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
  let mut session = Session {
    view: View::Launcher,
    expression: String::with_capacity(EXPRESSION_LENGTH),
    program: calculator,
    memory,
    stack,
  };
  let Ok(()) = launcher(calculator);

  input::listen(
    root,
    machine,
    |code, edge, report| session.key(code, edge, report),
    || line(format_args!("Console keyboard ready")),
  )
}

fn launcher(calculator: &[u8]) -> Result<(), Never> {
  let Ok(()) = line(format_args!("Console launcher"));
  match calculator.is_empty() {
    false => line(format_args!("C: Calculator | Esc: Exit")),
    true => line(format_args!("Esc: Exit")),
  }
}

impl Session<'_> {
  fn key(&mut self, code: u16, edge: KeyEdge, report: BootReport) -> Result<Action, Never> {
    match (edge, &self.view, code) {
      (KeyEdge::Release, _, _) => {}
      (KeyEdge::Press, View::Launcher, ESCAPE) => return Ok(Action::Exit),
      (KeyEdge::Press, View::Launcher, C) => match self.program.is_empty() {
        false => {
          self.view = View::Calculator;
          let Ok(()) = line(format_args!(
            "Calculator opened. Type 12+34 and press Enter. Esc returns."
          ));
        }
        true => {}
      },
      (KeyEdge::Press, View::Launcher, _) => {}
      (KeyEdge::Press, View::Calculator, ESCAPE) => {
        self.expression.clear();
        self.view = View::Launcher;
        let Ok(()) = launcher(self.program);
      }
      (KeyEdge::Press, View::Calculator, RETURN) => {
        let Ok(()) = self.evaluate();
        self.expression.clear();
      }
      (KeyEdge::Press, View::Calculator, BACKSPACE) => {
        let _removed = self.expression.pop();
        let Ok(()) = line(format_args!("{}", self.expression));
      }
      (KeyEdge::Press, View::Calculator, _) => {
        let Ok(shift) = report.shifted();
        let Ok(letter) = character(code, shift);
        let input = match (code, shift, letter) {
          (0x2d, Shift::Clear, _) => Some('-'),
          (0x2e, Shift::Pressed, _) => Some('+'),
          (0x38, Shift::Clear, _) => Some('/'),
          (_, _, Some(letter @ ('0'..='9' | '*'))) => Some(letter),
          _ => None,
        };
        match (input, self.expression.len() < EXPRESSION_LENGTH) {
          (Some(letter), true) => {
            self.expression.push(letter);
            let Ok(()) = line(format_args!("{}", self.expression));
          }
          (Some(_), false) => {
            let Ok(()) = line(format_args!("Expression is full. Press Backspace."));
          }
          (None, _) => {}
        }
      }
    }

    Ok(Action::Continue)
  }

  fn evaluate(&mut self) -> Result<(), Never> {
    let Some(request) = request(&self.expression) else {
      return line(format_args!(
        "Enter two whole numbers from 0 to 65535 with +, -, * or /."
      ));
    };
    match userspace::calculate(self.memory, self.stack, self.program, request) {
      Ok(Some(answer)) => line(format_args!("{} = {answer}", self.expression)),
      Ok(None) => line(format_args!("{} = Error", self.expression)),
      Err(error) => line(format_args!("Calculator could not run: {error}")),
    }
  }
}

fn request(expression: &str) -> Option<[u8; 8]> {
  let at = expression.find(['+', '-', '*', '/'])?;
  let (left, operator_and_right) = expression.split_at(at);
  let [operator, right @ ..] = operator_and_right.as_bytes() else {
    return None;
  };
  let [left_lo, left_hi] = operand(left)?.to_le_bytes();
  let [right_lo, right_hi] = operand(core::str::from_utf8(right).ok()?)?.to_le_bytes();

  Some([left_lo, left_hi, right_lo, right_hi, *operator, 0, 0, 0])
}

fn operand(text: &str) -> Option<u16> {
  match text.bytes().all(|byte| byte.is_ascii_digit()) {
    true => text.parse().ok(),
    false => None,
  }
}
