//! Single-segment DMA rings adapted from the nos polling driver.
//! See NOS-LICENSE for the copyright and license. The cycle bit publishes a
//! transfer after all of its payload and later entries are available.

use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use console_core_never::Never;

use super::regs::*;

pub(super) const RING_LENGTH: u32 = 64;
const LAST_LINK: u32 = RING_LENGTH.saturating_sub(1);
const TRB_BYTES: u64 = 16;

#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct Trb {
  pub(super) parameter: u64,
  pub(super) status: u32,
  pub(super) control: u32,
}

const _: [(); 16] = [(); size_of::<Trb>()];

pub(super) struct Ring {
  mapping: Mapping,
  index: u32,
  cycle: u32,
}

impl Ring {
  pub(super) fn new(mapping: Mapping) -> Result<Self, Never> {
    Ok(Self {
      mapping,
      index: 0,
      cycle: 1,
    })
  }

  pub(super) fn dequeue(&self) -> Result<u64, Never> {
    let Ok(position) = self.position();

    Ok(position.bus)
  }

  fn position(&self) -> Result<Mapping, Never> {
    self
      .mapping
      .at(u64::from(self.index).saturating_mul(TRB_BYTES))
  }

  #[cfg_attr(
    dylint_lib = "explicit069_lock_discipline",
    expect(
      explicit069_lock_discipline,
      reason = "the cycle bit hands this transfer to the controller after its DMA words are visible"
    )
  )]
  pub(super) fn append<const N: usize>(&mut self, transfer: [Trb; N]) -> Result<[u64; N], Never> {
    let mut addresses = [0; N];
    let mut first = None;

    for (trb, address) in transfer.into_iter().zip(&mut addresses) {
      let Ok(slot) = self.position();
      let register = slot.cpu;
      let control = trb.control | self.cycle;
      let Ok(()) = Quadword(register).write(trb.parameter);
      let Ok(()) = write32(register.saturating_add(8), trb.status);

      match first {
        None => first = Some((register, control)),
        Some(_) => {
          let Ok(()) = write32(register.saturating_add(12), control);
        }
      }

      *address = slot.bus;
      self.index = self.index.saturating_add(1);

      match ::core::primitive::bool::then_some(self.index == LAST_LINK, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = self.link();
        }
        ::core::option::Option::None => {}
      }
    }

    match first {
      Some((address, control)) => {
        compiler_fence(Ordering::Release);
        let Ok(()) = write32(address.saturating_add(12), control);
      }
      None => {}
    }

    Ok(addresses)
  }

  fn link(&mut self) -> Result<(), Never> {
    let Ok(slot) = self.position();
    let register = slot.cpu;
    let Ok(()) = Quadword(register).write(self.mapping.bus);
    let Ok(()) = write32(register.saturating_add(8), 0);
    let Ok(link_type) = trb_type_field(TRB_LINK);
    let control = link_type | TRB_TOGGLE_CYCLE | self.cycle;
    let Ok(()) = write32(register.saturating_add(12), control);
    self.index = 0;
    self.cycle ^= TRB_CYCLE;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit069_lock_discipline",
    expect(
      explicit069_lock_discipline,
      reason = "the controller changes the cycle bit after writing the DMA words, which the CPU reads after the fence"
    )
  )]
  pub(super) fn pop(&mut self) -> Result<Option<Trb>, Never> {
    let Ok(slot) = self.position();
    let register = slot.cpu;
    let Ok(control) = read32(register.saturating_add(12));

    match ::core::primitive::bool::then_some(control & TRB_CYCLE == self.cycle, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    }

    compiler_fence(Ordering::Acquire);
    let Ok(parameter) = read64(register);
    let Ok(status) = read32(register.saturating_add(8));
    let event = Trb {
      parameter,
      status,
      control,
    };
    self.index = self.index.saturating_add(1);

    match ::core::primitive::bool::then_some(self.index == RING_LENGTH, ()) {
      ::core::option::Option::Some(()) => {
        self.index = 0;
        self.cycle ^= TRB_CYCLE;
      }
      ::core::option::Option::None => {}
    }

    Ok(Some(event))
  }
}

#[derive(Clone, Copy)]
pub(super) struct Mapping {
  pub(super) cpu: u64,
  pub(super) bus: u64,
}

impl Mapping {
  pub(super) fn at(self, offset: u64) -> Result<Self, Never> {
    Ok(Self {
      cpu: self.cpu.saturating_add(offset),
      bus: self.bus.saturating_add(offset),
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the fixed-width xHCI address becomes a Rust pointer for the volatile store"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the xHCI DWORD store for checked register and DMA offsets"
  )
)]
pub(super) fn write32(address: u64, value: u32) -> Result<(), Never> {
  let at = usize::from_le_bytes(address.to_le_bytes());

  // SAFETY: callers supply an aligned DWORD in the controller BAR or an owned
  // DMA page. Volatile access preserves each device-visible store.
  unsafe { ptr::with_exposed_provenance_mut::<u32>(at).write_volatile(value) };

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the fixed-width xHCI address becomes a Rust pointer for the volatile load"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the xHCI DWORD load for checked register and DMA offsets"
  )
)]
pub(super) fn read32(address: u64) -> Result<u32, Never> {
  let at = usize::from_le_bytes(address.to_le_bytes());
  // SAFETY: callers supply an aligned DWORD in the controller BAR or a DMA page.
  Ok(unsafe { ptr::with_exposed_provenance::<u32>(at).read_volatile() })
}

pub(super) struct Quadword(pub(super) u64);

impl Quadword {
  pub(super) fn write(self, value: u64) -> Result<(), Never> {
    let [a, b, c, d, e, f, g, h] = value.to_le_bytes();
    let Ok(()) = write32(self.0, u32::from_le_bytes([a, b, c, d]));

    write32(self.0.saturating_add(4), u32::from_le_bytes([e, f, g, h]))
  }
}

pub(super) fn read64(address: u64) -> Result<u64, Never> {
  let Ok(first) = read32(address);
  let Ok(second) = read32(address.saturating_add(4));
  let [a, b, c, d] = first.to_le_bytes();
  let [e, f, g, h] = second.to_le_bytes();

  Ok(u64::from_le_bytes([a, b, c, d, e, f, g, h]))
}
