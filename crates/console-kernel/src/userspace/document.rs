//! The preview retains the requested viewport. Renderer calls and panel updates
//! run as effects between keyboard polls.

#[path = "../../native-browser/wire.rs"]
pub(super) mod wire;

mod machine;

use console_core_bitmap_font::legacy::BASIC_LEGACY;
use console_core_state_machine::{Never, SingleEffectMachine, SingleTransition};
use console_kernel::interrupts::{Address, USER_SYSCALL};
use console_kernel_handles::{
  Boot, Bytes, CreatedProcess, HandleError, HandleValue, Kernel, MappingAccess, ProcessId, Right,
  Rights,
};

use super::paging::{FRAME_ADDRESS, Frame, Task};
use super::{Error, Reserved, Trap, USER_WRITE_PROTECTION, lend};
use crate::display;
use crate::input::{self, Action, Turn};
use crate::serial::Serial;

use machine::{Completion, Document, Effect, Progress, Request, View};

const HTML: &[u8] = include_bytes!("../../native-browser/page.html");
const RENDERER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native-renderer.bin"));
const ROWS_PER_TURN: u16 = 4;
const _: () = assert!(
  HTML.len() < core::mem::size_of::<wire::Html>()
    && FRAME_ADDRESS == wire::FRAME_ADDRESS
    && USER_SYSCALL.0 == wire::EXIT_INTERRUPT
);

pub(crate) struct Preview<'a> {
  runtime: Runtime<'a>,
  view: View,
  completion: Option<Completion>,
}

struct Runtime<'a> {
  memory: &'a mut Reserved,
  stack: Address,
  session: Option<Session>,
}

enum Performed {
  Input(Action),
  Completed(Completion),
  Done,
}

enum Notice {
  Opened,
  Scrolled,
  Restarted,
}

struct Session {
  kernel: Kernel,
  root: ProcessId,
  job: HandleValue,
  frame: HandleValue,
  renderer: Renderer,
  rendered: Option<u64>,
  last_row: u64,
  next_scanline: Option<u16>,
  notice: Notice,
}

struct Renderer {
  process: CreatedProcess,
  pixels: HandleValue,
}

impl<'a> Preview<'a> {
  pub(crate) fn new(memory: &'a mut Reserved, stack: Address) -> Result<Self, Never> {
    let Ok(SingleTransition { state, effect }) = Document::initialize(&(), None);
    let mut preview = Self {
      runtime: Runtime {
        memory,
        stack,
        session: None,
      },
      view: state,
      completion: None,
    };
    let Ok(_action) = preview.apply(effect);
    Ok(preview)
  }

  pub(crate) fn turn(&mut self, turn: Turn) -> Result<Action, Never> {
    let progress = self.runtime.session.as_ref().map(|session| Progress {
      rendered: session.rendered,
      last_row: session.last_row,
      next_scanline: session.next_scanline,
    });
    let Ok(SingleTransition { state, effect }) = Document::handle(
      self.view,
      Request {
        turn,
        progress,
        completion: self.completion.take(),
      },
    );
    self.view = state;

    self.apply(effect)
  }

  fn apply(&mut self, effect: Option<Effect>) -> Result<Action, Never> {
    match effect {
      Some(effect_2) => match self.runtime.perform(effect_2) {
        Ok(Performed::Input(pressed)) => return Ok(pressed),
        Ok(Performed::Completed(report)) => self.completion = Some(report),
        Ok(Performed::Done) => {}
        Err(error) => {
          self.runtime.session = None;
          let Ok(()) = display::reset();
          let Ok(()) = input::line(format_args!("Native browser stopped: {error}"));
          self.completion = Some(Completion::Failed);
        }
      },
      ::core::option::Option::None => {}
    }

    Ok(Action::Continue)
  }
}

impl Runtime<'_> {
  fn perform(&mut self, effect: Effect) -> Result<Performed, Error> {
    match effect {
      Effect::Keyboard { code, edge, report } => {
        let Ok(action) = input::present(code, edge, report);
        Ok(Performed::Input(action))
      }
      Effect::Open => {
        self.open()?;
        Ok(Performed::Completed(Completion::Opened))
      }
      Effect::Close => {
        self.session = None;
        let Ok(()) = display::reset();
        let Ok(()) = input::line(format_args!("Native browser closed. F2 opens it again."));
        Ok(Performed::Done)
      }
      Effect::Restart => {
        let session = self.session.as_mut().ok_or(HandleError::BadState)?;
        session.restart_after_fault(self.memory, self.stack)?;
        Ok(Performed::Completed(Completion::Restarted))
      }
      Effect::Render(row) => {
        let session = self.session.as_mut().ok_or(HandleError::BadState)?;
        session.render(self.memory, self.stack, row)?;
        Ok(Performed::Completed(Completion::Rendered {
          last_row: session.last_row,
        }))
      }
      Effect::Present(first) => {
        let session = self.session.as_mut().ok_or(HandleError::BadState)?;
        let Ok(()) = session.present(&self.memory.frame, first);
        Ok(Performed::Done)
      }
    }
  }

  fn open(&mut self) -> Result<(), Error> {
    self.session = None;
    let mut document = wire::Document {
      font: BASIC_LEGACY,
      html: [0; core::mem::size_of::<wire::Html>()],
    };

    for (to, from) in document.html.iter_mut().zip(HTML) {
      *to = *from;
    }

    self.memory.load_document(document)?;
    let session = Session::open(&mut self.memory.frame)?;
    let Ok(()) = display::reset();
    self.session = Some(session);

    Ok(())
  }
}

impl Session {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the preview registers its scrubbed boot-reserved frame with a fresh handle table before creating the renderer"
    )
  )]
  fn open(frame: &mut Frame) -> Result<Self, Error> {
    let Boot {
      mut kernel,
      root,
      root_job: job,
      executable_resource: _,
    } = Kernel::boot_with_memory_backing(Bytes(262_144), Bytes(0), 0)?;
    let Ok(()) = frame.scrub();
    let Ok(physical) = frame.identity_address();
    // SAFETY: The boot reservation keeps this initialized page exclusively
    // owned by the preview. The boot frame check has released its registration.
    let frame_2 = unsafe { kernel.register_frame(root, physical) }?;
    let renderer = Renderer::create(&mut kernel, root, job, frame_2)?;

    Ok(Self {
      kernel,
      root,
      job,
      frame: frame_2,
      renderer,
      rendered: None,
      last_row: u64::from(wire::HTML_BYTES),
      next_scanline: None,
      notice: Notice::Opened,
    })
  }

  fn invoke(
    &mut self,
    memory: &mut Reserved,
    stack: Address,
    argument: u64,
  ) -> Result<Trap, Error> {
    let mut task = Task {
      id: self.renderer.process.id,
      space: &mut memory.renderer,
      stack: stack.0,
    };
    task.run_mapped(
      &mut self.kernel,
      self.renderer.pixels,
      MappingAccess::Write,
      RENDERER,
      argument,
    )
  }

  fn render(&mut self, memory: &mut Reserved, stack: Address, scroll: u64) -> Result<(), Error> {
    let trap = self.invoke(memory, stack, scroll)?;
    let result = match trap.reason {
      0 | u64::MAX => trap,
      _ => {
        self.replace_renderer()?;
        self.invoke(memory, stack, scroll)?
      }
    };

    let last_visible_row = u64::from(wire::HTML_BYTES);

    match (result.reason, result.value) {
      (0, rows) => match bool::then_some((1..=last_visible_row).contains(&rows), ()) {
        Some(()) => self.last_row = rows.saturating_sub(u64::from(wire::ROWS)),
        None => return Err(Error::Renderer(0, rows)),
      },
      (reason, value) => return Err(Error::Renderer(reason, value)),
    }

    self.rendered = Some(scroll);
    self.next_scanline = (scroll <= self.last_row).then_some(0);
    Ok(())
  }

  fn present(&mut self, frame: &Frame, first: u16) -> Result<(), Never> {
    let end = first.saturating_add(ROWS_PER_TURN).min(wire::HEIGHT);
    let Ok(pixels) = frame.pixels();
    let Ok(_shown) = display::draw_bitmap_rows(
      pixels
        .as_chunks::<{ core::mem::size_of::<wire::Scanline>() / core::mem::size_of::<u32>() }>()
        .0,
      u64::from(first)..u64::from(end),
    );
    self.next_scanline = (end < wire::HEIGHT).then_some(end);

    match self.next_scanline {
      Some(_) => {}
      None => {
        let line = match self.notice {
          Notice::Opened => "console-kernel: Native browser opened the local HTML page",
          Notice::Scrolled => "console-kernel: Native browser scrolled",
          Notice::Restarted => "console-kernel: Native browser replaced the faulted renderer",
        };
        let Ok(()) = Serial.print(line);
        self.notice = Notice::Scrolled;
      }
    }

    Ok(())
  }

  fn restart_after_fault(&mut self, memory: &mut Reserved, stack: Address) -> Result<(), Error> {
    let checked_value = self.invoke(memory, stack, wire::FAULT_REQUEST)?;

    match checked_value {
      Trap {
        reason: USER_WRITE_PROTECTION,
        value: wire::DOCUMENT_ADDRESS,
      } => {}
      Trap { reason, value } => return Err(Error::Renderer(reason, value)),
    }

    self.replace_renderer()?;
    self.rendered = None;
    self.next_scanline = None;
    self.notice = Notice::Restarted;

    Ok(())
  }

  fn replace_renderer(&mut self) -> Result<(), Error> {
    let stopped = self.renderer.process;
    self.kernel.kill(self.root, stopped.process)?;
    self.kernel.close(self.root, stopped.process)?;

    match self.kernel.handles(stopped.id) {
      Err(HandleError::ProcessNotFound) => {
        let renderer = Renderer::create(&mut self.kernel, self.root, self.job, self.frame)?;
        self.renderer = renderer;
        Ok(())
      }
      Ok(_) | Err(_) => Err(Error::from(HandleError::BadState)),
    }
  }
}

impl Renderer {
  fn create(
    kernel: &mut Kernel,
    root: ProcessId,
    job: HandleValue,
    frame: HandleValue,
  ) -> Result<Self, HandleError> {
    let process = kernel.create_process(root, job)?;
    let Ok(writable) = Rights::from_rights(&[Right::Read, Right::Write, Right::Transfer]);
    let pixels = lend(kernel, root, frame, &process, writable)?;
    kernel.close(root, process.channel)?;

    Ok(Self { process, pixels })
  }
}
