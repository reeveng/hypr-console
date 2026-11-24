//! The native preview waits for a boot-keyboard report, converts it to the
//! input queue's edges, and shows them until Escape closes the controller.

#![forbid(unsafe_code)]

use core::fmt::{self, Write};

use console_core_never::Never;
use console_kernel::events::{EventId, Input, InputEvent, InputQueue, KeyEdge};
use console_kernel::usb_keyboard::{BootReport, character};

use crate::acpi::Root;
use crate::display;
use crate::serial::Serial;
use crate::xhci::{self, DmaMode, Keyboard, Machine};

pub(super) fn run<S>(
  root: Option<Root>,
  machine: Machine,
  state: S,
  turn: impl Fn(&mut S, Turn) -> Result<Action, Never>,
) -> Result<(), xhci::Error> {
  drive(root, machine, state, turn, || {
    let Ok(()) = line(format_args!(
      "USB keyboard ready. Press keys to see events. Escape exits."
    ));
    line(format_args!("F2 opens the native browser."))
  })
}

pub(super) fn listen<S>(
  root: Option<Root>,
  machine: Machine,
  state: S,
  key: impl Fn(&mut S, u16, KeyEdge, BootReport) -> Result<Action, Never>,
  ready: impl FnOnce() -> Result<(), Never>,
) -> Result<(), xhci::Error> {
  drive(
    root,
    machine,
    state,
    |held, turn| match turn {
      Turn::Key { code, edge, report } => key(held, code, edge, report),
      Turn::Idle => Ok(Action::Continue),
    },
    ready,
  )
}

fn drive<S>(
  root: Option<Root>,
  machine: Machine,
  mut state: S,
  turn: impl Fn(&mut S, Turn) -> Result<Action, Never>,
  ready: impl FnOnce() -> Result<(), Never>,
) -> Result<(), xhci::Error> {
  let attached = xhci::open(root, machine)?;
  let mut keyboard = match attached {
    Some(keyboard) => keyboard,
    None => return Ok(()),
  };
  let Ok(mode) = keyboard.dma_mode();

  match mode {
    DmaMode::AmdVi => {
      let Ok(()) = line(format_args!("AMD-Vi DMA window active"));
    }
    DmaMode::Direct => {}
  }

  let Ok(()) = ready();
  let input = read(&mut keyboard, &mut state, turn);
  let closed = keyboard.close();
  let () = closed?;

  match mode {
    DmaMode::AmdVi => {
      let Ok(()) = line(format_args!("AMD-Vi DMA window revoked"));
    }
    DmaMode::Direct => {}
  }

  input
}

#[cfg_attr(
  dylint_lib = "explicit054_no_loop",
  expect(
    explicit054_no_loop,
    reason = "the preview stays at the input console until Escape or a controller error"
  )
)]
fn read<S>(
  keyboard: &mut Keyboard,
  state: &mut S,
  turn: impl Fn(&mut S, Turn) -> Result<Action, Never>,
) -> Result<(), xhci::Error> {
  let mut sequence = 0_u64;
  let mut queue = InputQueue::<32>::EMPTY;
  let mut previous = BootReport::EMPTY;

  loop {
    let pending = keyboard.poll()?;
    let decoded = match pending {
      Some(bytes) => {
        let Ok(report) = BootReport::read(bytes);

        report
      }
      None => None,
    };
    let report = match decoded {
      Some(report) => report,
      None => {
        let Ok(action) = turn(state, Turn::Idle);

        match action {
          Action::Exit => return Ok(()),
          Action::Continue => {
            core::hint::spin_loop();
            continue;
          }
        }
      }
    };

    let Ok(edges) = previous.edges(report);

    for (usage, edge) in edges {
      sequence = sequence.saturating_add(1);
      let event = InputEvent {
        id: EventId(sequence),
        input: Input::Key {
          code: u16::from(usage),
          edge,
        },
      };
      let admission = queue.offer(event).map_err(|_fault| xhci::Error::InputQueue);
      let _admitted = admission?;
    }

    previous = report;

    'over__pending_2: for _pending in 0..32_u8 {
      let Ok(pending_2) = queue.take();

      match pending_2.map(|event| event.input) {
        Some(Input::Key { code, edge }) => match turn(state, Turn::Key { code, edge, report }) {
          Ok(Action::Continue) => {}
          Ok(Action::Exit) => return Ok(()),
        },
        Some(Input::Position { .. }) => {}
        None => break 'over__pending_2,
      }
    }
  }
}

pub(super) enum Turn {
  Key {
    code: u16,
    edge: KeyEdge,
    report: BootReport,
  },
  Idle,
}

pub(super) enum Action {
  Continue,
  Exit,
}

pub(super) fn present(code: u16, edge: KeyEdge, report: BootReport) -> Result<Action, Never> {
  match (edge, code) {
    (KeyEdge::Press, 0x29) => {
      let Ok(()) = line(format_args!("USB keyboard test ended"));

      return Ok(Action::Exit);
    }
    (KeyEdge::Press, _) => {
      let Ok(shift) = report.shifted();
      let Ok(printable) = character(code, shift);

      match printable {
        Some(letter) => {
          let Ok(()) = line(format_args!("key pressed: {letter} (USB {code:#04x})"));
        }
        None => {
          let Ok(()) = line(format_args!("key pressed: USB {code:#04x}"));
        }
      }
    }
    (KeyEdge::Release, _) => {
      let Ok(()) = line(format_args!("key released: USB {code:#04x}"));
    }
  }

  Ok(Action::Continue)
}

pub(super) fn line(message: fmt::Arguments<'_>) -> Result<(), Never> {
  let Ok(()) = display::print(message);
  let _written = write!(Serial, "console-kernel: {message}\r\n");

  Ok(())
}
