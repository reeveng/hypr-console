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

pub(super) fn run(root: Option<Root>, machine: Machine) -> Result<(), xhci::Error> {
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

  let Ok(()) = line(format_args!(
    "USB keyboard ready. Press keys to see events. Escape exits."
  ));
  let input = read(&mut keyboard);
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
fn read(keyboard: &mut Keyboard) -> Result<(), xhci::Error> {
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
        core::hint::spin_loop();
        continue;
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
    let Ok(action) = present(&mut queue, report);

    match action {
      Action::Continue => {}
      Action::Exit => return Ok(()),
    }
  }
}

enum Action {
  Continue,
  Exit,
}

fn present(queue: &mut InputQueue<32>, report: BootReport) -> Result<Action, Never> {
  for _pending in 0..32_u8 {
    let Ok(pending) = queue.take();
    let input = match pending {
      Some(event) => event.input,
      None => break,
    };
    let (code, edge) = match input {
      Input::Key { code, edge } => (code, edge),
      Input::Position { .. } => continue,
    };

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
  }

  Ok(Action::Continue)
}

fn line(message: fmt::Arguments<'_>) -> Result<(), Never> {
  let Ok(()) = display::write_without_waiting(format_args!("console-kernel: {message}\n"));
  let _written = write!(Serial, "console-kernel: {message}\r\n");

  Ok(())
}
