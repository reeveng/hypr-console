//! What the daemon decided to do, said without doing it.
//!
//! Everything in this crate that thinks is a function from what arrived to one
//! of these. Nothing that thinks touches a device, which is why the arithmetic
//! can be held still and asked the same question twice.
//!
//! [`Effect::Using`] is the odd one and is here rather than anywhere else for
//! the same reason as the rest: which input someone is using is decided by
//! what arrived, so it is decided where everything else is, and carrying it
//! out -- saying so to `console-events` and writing the word down for the next
//! session -- is the binary's, like starting a program is.

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Service};
use console_input_event_devices::EventType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
  pub kind: EventType,
  pub code: u16,
  pub value: i32,
}

impl Output {
  pub fn relative(code: u16, value: i32) -> Result<Self, Never> {
    Ok(Output {
      kind: EventType::RELATIVE,
      code,
      value,
    })
  }

  pub fn key(code: u16, value: i32) -> Result<Self, Never> {
    Ok(Output {
      kind: EventType::KEY,
      code,
      value,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
  Run(Vec<String>),
  Dispatch(CompositorAction),
  Frame(Vec<Output>),
  Tell(console_onscreen::PadInput),
  Using(console_input_bindings::bound::Input),
  Reconnected(Reconnected),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Run(_) => &[Service::Process],
      Self::Dispatch(_) => &[Service::Compositor],
      Self::Frame(_) => &[Service::Input],
      Self::Tell(_) => &[Service::Panels],
      Self::Using(_) => &[Service::Events, Service::FileSystem],
      Self::Reconnected(_) => &[Service::Console],
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reconnected {
  pub device: crate::reading::From,
  pub gone: std::time::Duration,
}

pub use console_compositor::{
  Action as CompositorAction, Carrying as Payload, Direction, WorkspaceTarget,
};

impl Effect {
  pub fn run(arguments: &[&str]) -> Result<Self, Never> {
    Ok(Effect::Run(
      arguments.iter().map(|word| (*word).to_string()).collect(),
    ))
  }

  pub fn workspace(target: WorkspaceTarget, carrying: Payload) -> Result<Self, Never> {
    Ok(Effect::Dispatch(CompositorAction::MoveWorkspace {
      target,
      carrying,
    }))
  }
}
