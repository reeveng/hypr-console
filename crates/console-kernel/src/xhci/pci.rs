//! PCI configuration access for the legacy segment-zero host bridge.
//! Discovery follows only the secondary buses assigned by firmware.

use core::arch::asm;

use console_core_never::Never;

use crate::ivrs::Requester;

const CONFIG_ADDRESS: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;
const QEMU_XHCI: u32 = 0x000d_1b36;
const SERIAL_BUS_USB_XHCI: u32 = 0x0c_03_30;
const PCI_BRIDGE: u32 = 0x0604;
const MEMORY_SPACE: u16 = 1 << 1;
const BUS_MASTER: u16 = 1 << 2;
const INTERRUPT_DISABLE: u16 = 1 << 10;

#[derive(Clone, Copy)]
pub(super) struct Function(u16);

#[derive(Clone, Copy)]
pub(super) enum Scope {
  Emulated,
  Remapped,
}

pub(super) enum Master {
  Enabled,
  Disabled,
}

#[derive(Debug)]
pub(super) struct TooManyControllers;

pub(super) struct Controllers([Option<Function>; 16]);

impl Controllers {
  pub(super) fn all(self) -> Result<impl Iterator<Item = Function>, Never> {
    Ok(self.0.into_iter().flatten())
  }
}

struct Discovery {
  buses: [bool; 256],
  functions: Controllers,
  scope: Scope,
}

pub(super) fn controllers(scope: Scope) -> Result<Controllers, TooManyControllers> {
  let mut discovery = Discovery {
    buses: [false; 256],
    functions: Controllers([None; 16]),
    scope,
  };
  let Ok(()) = discovery.mark(0);

  for bus in 0..=u8::MAX {
    let () = discovery.visit(bus)?;
  }

  Ok(discovery.functions)
}

impl Discovery {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the PCI bus number becomes a Rust array index"
    )
  )]
  fn mark(&mut self, bus: u8) -> Result<(), Never> {
    match self.buses.get_mut(usize::from(bus)) {
      Some(entry) => *entry = true,
      None => {}
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the PCI bus number becomes a Rust array index"
    )
  )]
  fn visit(&mut self, bus: u8) -> Result<(), TooManyControllers> {
    match self.buses.get(usize::from(bus)) {
      Some(true) => {}
      Some(false) | None => return Ok(()),
    }

    for device in 0..32_u16 {
      let base = u16::from(bus).wrapping_shl(8) | device.wrapping_shl(3);
      let Ok(id) = Function(base).read(0);

      match ::core::primitive::bool::then_some(id & 0xffff == 0xffff, ()) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => {}
      }

      let Ok(header) = Function(base).read(0x0c);
      let limit = match header & 0x0080_0000 {
        0 => 1,
        _ => 8,
      };

      for number in 0..limit {
        let () = self.function(bus, Function(base | number))?;
      }
    }

    Ok(())
  }

  fn function(&mut self, bus: u8, function: Function) -> Result<(), TooManyControllers> {
    let Ok(class) = function.read(8);

    match ::core::primitive::bool::then_some(class.wrapping_shr(16) == PCI_BRIDGE, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(bridge) = function.read(0x18);
        let [_, secondary, subordinate, _] = bridge.to_le_bytes();

        match ::core::primitive::bool::then_some(secondary > bus && secondary <= subordinate, ())
        {
          ::core::option::Option::Some(()) => {
            let Ok(()) = self.mark(secondary);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(controller) = function.matches(self.scope, class);

    match controller {
      Match::Controller => {
        let vacant = self
          .functions
          .0
          .iter_mut()
          .find(|entry| entry.is_none())
          .ok_or(TooManyControllers);
        let slot = vacant?;
        *slot = Some(function);
      }
      Match::Other => {}
    }

    Ok(())
  }
}

enum Match {
  Controller,
  Other,
}

impl Function {
  fn matches(self, scope: Scope, class: u32) -> Result<Match, Never> {
    match ::core::primitive::bool::then_some(class.wrapping_shr(8) == SERIAL_BUS_USB_XHCI, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(Match::Other),
    }

    let Ok(id) = self.read(0);

    Ok(match (scope, id) {
      (_, QEMU_XHCI)
      | (Scope::Remapped, 0x15b9_1022 | 0x15ba_1022 | 0x15c0_1022 | 0x15c1_1022) => {
        Match::Controller
      }
      (Scope::Remapped | Scope::Emulated, _) => Match::Other,
    })
  }

  pub(super) fn requester(self) -> Result<Requester, Never> {
    Ok(Requester(self.0))
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this function performs the x86 configuration-address port write"
    )
  )]
  fn select(self, offset: u32) -> Result<(), Never> {
    let address = 0x8000_0000 | u32::from(self.0).wrapping_shl(8) | offset;

    // SAFETY: only the boot CPU performs the paired PCI config accesses, with interrupts masked.
    unsafe {
      asm!("out dx, eax", in("dx") CONFIG_ADDRESS, in("eax") address,
        options(nostack, preserves_flags));
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this function performs the x86 configuration-data port read"
    )
  )]
  fn read(self, offset: u32) -> Result<u32, Never> {
    let Ok(()) = self.select(offset);
    let value;

    // SAFETY: select latched this aligned DWORD in configuration space.
    unsafe {
      asm!("in eax, dx", in("dx") CONFIG_DATA, out("eax") value,
        options(nostack, preserves_flags));
    }

    Ok(value)
  }

  pub(super) fn bar(self) -> Result<Option<u64>, Never> {
    let Ok(low) = self.read(0x10);
    let high = match low & 0b111 {
      0b000 => 0,
      0b100 => {
        let Ok(high) = self.read(0x14);

        high
      }
      _ => return Ok(None),
    };
    let base = u64::from(high).wrapping_shl(32) | u64::from(low & !0xf);

    Ok(match base {
      0 => None,
      address => Some(address),
    })
  }

  pub(super) fn memory_space(self) -> Result<(), Never> {
    let Ok(word) = self.command_word();

    self.write_command(word | MEMORY_SPACE)
  }

  pub(super) fn bus_master(self, state: Master) -> Result<(), Never> {
    let Ok(word) = self.command_word();
    let memory = word | MEMORY_SPACE | INTERRUPT_DISABLE;

    self.write_command(match state {
      Master::Enabled => memory | BUS_MASTER,
      Master::Disabled => memory & !BUS_MASTER,
    })
  }

  fn command_word(self) -> Result<u16, Never> {
    let Ok(value) = self.read(4);
    let [low, high, _, _] = value.to_le_bytes();

    Ok(u16::from_le_bytes([low, high]))
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this function performs the PCI command register word write"
    )
  )]
  fn write_command(self, command: u16) -> Result<(), Never> {
    let Ok(()) = self.select(4);

    // SAFETY: CONFIG_DATA addresses the command register. The word write leaves PCI status alone.
    unsafe {
      asm!("out dx, ax", in("dx") CONFIG_DATA, in("ax") command,
        options(nostack, preserves_flags));
    }

    Ok(())
  }
}
