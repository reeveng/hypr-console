//! AMD-Vi setup and command encodings adapted from NONOS, copyright (C) 2026
//! NONOS Contributors, AGPL-3.0-or-later. See the repository LICENSE and
//! https://github.com/NON-OS/microkernel/tree/main/src/arch/x86_64/iommu/amd_vi.
//!
//! The initial device table denies every requester. One xHCI controller gets
//! a separate I/O address window into its DMA pages. Command completion is
//! required before it can bus master or the window can be reused.

use alloc::alloc::alloc_zeroed;
use core::alloc::Layout;
use core::arch::x86_64::_rdtsc;
use core::fmt;
use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use console_core_never::Never;

use crate::ivrs::{Requester, Unit};

const PAGE: u64 = 4096;
const DEVICE_TABLE_BYTES: u64 = 2_097_152;
const DEVICE_TABLE: Register = Register(0x0000);
const COMMAND_RING: Register = Register(0x0008);
const EVENT_RING: Register = Register(0x0010);
const CONTROL: Register = Register(0x0018);
const EXCLUSION_BASE: Register = Register(0x0020);
const EXCLUSION_LIMIT: Register = Register(0x0028);
const FEATURES: Register = Register(0x0030);
const COMMAND_HEAD: Register = Register(0x2000);
const COMMAND_TAIL: Register = Register(0x2008);
const EVENT_HEAD: Register = Register(0x2010);
const EVENT_TAIL: Register = Register(0x2018);
const STATUS: Register = Register(0x2020);
const RUNNING: u64 = 1 | (1 << 2) | (1 << 3) | (1 << 4) | (0x3f << 12) | (1 << 50);
const ENABLE: u64 = 1 | (1 << 2) | (1 << 10) | (1 << 12);
const INVALIDATE_ALL_SUPPORTED: u64 = 1 << 6;
const MASK_ADDRESS: u64 = 0x000f_ffff_ffff_f000;
const TRANSLATION: u64 = 3;
const MODE_ONE: u64 = 1 << 9;
const DMA_READ_WRITE: u64 = 3 << 61;
const DOMAIN: u64 = 1;
const FORCE_COHERENT: u64 = 1 << 60;
const INVALIDATE_ALL: u64 = 8 << 60;
const INVALIDATE_DEVICE: u64 = 2 << 60;
const INVALIDATE_DOMAIN: u64 = (3 << 60) | (DOMAIN << 32);
const WAIT_COMPLETE: u64 = 1 << 60;
const RING_LENGTH_FIELD: u64 = 8 << 56;
const COMMAND_SLOT: u64 = 16;
const COMPLETION_WRITTEN: u64 = 1;
const WAIT_CYCLES: u64 = 6_000_000_000;
const IO_START: u64 = 0x1_0000;
const IO_END: u64 = 0x20_0000;
const ABORT_INTERRUPTS: u64 = 1;

#[derive(Clone, Copy)]
struct Register(u64);

struct Control(u64);

struct Word(u64);

#[derive(Debug)]
pub(super) enum Error {
  Allocation,
  FirmwareActive,
  InvalidationUnsupported,
  Timeout,
  Disabled,
  Window,
}

impl fmt::Display for Error {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str(match self {
      Self::Allocation => "could not reserve AMD-Vi tables",
      Self::FirmwareActive => "AMD-Vi still has active firmware tables",
      Self::InvalidationUnsupported => "AMD-Vi lacks full cache invalidation",
      Self::Timeout => "AMD-Vi invalidation did not complete",
      Self::Disabled => "AMD-Vi did not enable translation",
      Self::Window => "the USB DMA pages do not fit in the AMD-Vi window",
    })
  }
}

pub(super) struct Pages {
  pub(super) base: u64,
  pub(super) bytes: u64,
}

pub(super) struct Mapped(pub(super) u64);

pub(super) struct Remapper {
  registers: u64,
  devices: u64,
  commands: u64,
  completion: u64,
  root: u64,
  tail: u64,
  requester: Option<Requester>,
}

impl Remapper {
  pub(super) fn start(unit: Unit<'_>) -> Result<Self, Error> {
    let registers = unit.registers;
    let Ok(control) = Word(registers.saturating_add(CONTROL.0)).read();
    let Ok(features) = Word(registers.saturating_add(FEATURES.0)).read();

    match (control & RUNNING, features & INVALIDATE_ALL_SUPPORTED) {
      (0, 0) => return Err(Error::InvalidationUnsupported),
      (0, _) => {}
      (_, _) => return Err(Error::FirmwareActive),
    }

    let devices = reserve(DEVICE_TABLE_BYTES)?;
    let commands = reserve(PAGE)?;
    let events = reserve(PAGE)?;
    let completion = reserve(PAGE)?;
    let root = reserve(PAGE)?;
    let mut remapper = Self {
      registers,
      devices,
      commands,
      completion,
      root,
      tail: 0,
      requester: None,
    };
    let Ok(()) = remapper.deny_all();
    let Ok(()) = remapper.enable(events, Control(control));
    let Ok(enabled) = Word(registers.saturating_add(CONTROL.0)).read();

    match enabled & 1 {
      0 => return Err(Error::Disabled),
      _ => {}
    }

    let () = remapper.finish(&[Command::InvalidateAll])?;

    Ok(remapper)
  }

  fn deny_all(&self) -> Result<(), Never> {
    for id in 0..=u16::MAX {
      let entry = self
        .devices
        .saturating_add(u64::from(id).saturating_mul(32));
      let Ok(()) = Word(entry).write(TRANSLATION);
      let Ok(()) = Word(entry.saturating_add(16)).write(ABORT_INTERRUPTS);
    }

    Ok(())
  }

  fn enable(&self, event_base: u64, previous_control: Control) -> Result<(), Never> {
    let Ok(()) = self.write(EXCLUSION_BASE, 0);
    let Ok(()) = self.write(EXCLUSION_LIMIT, 0);
    let Ok(()) = self.write(DEVICE_TABLE, self.devices | 511);
    let Ok(()) = self.write(COMMAND_RING, self.commands | RING_LENGTH_FIELD);
    let Ok(()) = self.write(EVENT_RING, event_base | RING_LENGTH_FIELD);
    let Ok(()) = self.write(COMMAND_HEAD, 0);
    let Ok(()) = self.write(COMMAND_TAIL, 0);
    let Ok(()) = self.write(EVENT_HEAD, 0);
    let Ok(()) = self.write(EVENT_TAIL, 0);
    let Ok(()) = self.write(STATUS, 0x1f);
    compiler_fence(Ordering::SeqCst);
    self.write(CONTROL, previous_control.0 | ENABLE)
  }

  pub(super) fn grant(&mut self, requester: Requester, pages: Pages) -> Result<Mapped, Error> {
    match (
      self.requester,
      pages.bytes != 0
        && pages.bytes.is_multiple_of(PAGE)
        && pages.base & MASK_ADDRESS == pages.base
        && pages.base != IO_START
        && pages.bytes <= IO_END.saturating_sub(IO_START)
        && pages.base.saturating_add(pages.bytes.saturating_sub(PAGE)) & MASK_ADDRESS
          == pages.base.saturating_add(pages.bytes.saturating_sub(PAGE)),
    ) {
      (None, true) => {}
      (Some(_), _) | (None, false) => return Err(Error::Window),
    }

    for page in 0..pages.bytes.div_euclid(PAGE) {
      let offset = page.saturating_mul(PAGE);
      let index = IO_START.saturating_add(offset).div_euclid(PAGE);
      let slot = self.root.saturating_add(index.saturating_mul(8));
      let physical = pages.base.saturating_add(offset);
      let Ok(()) = Word(slot).write(physical | 1 | FORCE_COHERENT | DMA_READ_WRITE);
    }

    let Ok(entry) = self.entry(requester);
    let Ok(()) = Word(entry.saturating_add(8)).write(DOMAIN);
    let Ok(()) = Word(entry).write(self.root | TRANSLATION | MODE_ONE | DMA_READ_WRITE);
    self.requester = Some(requester);
    let () = self.finish(&[Command::Device(requester), Command::Domain])?;

    Ok(Mapped(IO_START))
  }

  pub(super) fn revoke(&mut self) -> Result<(), Error> {
    let requester = match self.requester {
      Some(requester) => requester,
      None => return Ok(()),
    };
    let Ok(entry) = self.entry(requester);
    let Ok(()) = Word(entry).write(TRANSLATION);
    let () = self.finish(&[Command::Device(requester), Command::Domain])?;

    for index in IO_START.div_euclid(PAGE)..IO_END.div_euclid(PAGE) {
      let Ok(()) = Word(self.root.saturating_add(index.saturating_mul(8))).write(0);
    }

    self.requester = None;

    Ok(())
  }

  fn entry(&self, requester: Requester) -> Result<u64, Never> {
    Ok(
      self
        .devices
        .saturating_add(u64::from(requester.0).saturating_mul(32)),
    )
  }

  fn write(&self, register: Register, value: u64) -> Result<(), Never> {
    Word(self.registers.saturating_add(register.0)).write(value)
  }

  fn finish(&mut self, commands: &[Command]) -> Result<(), Error> {
    let Ok(()) = Word(self.completion).write(0);
    let done = Command::Complete(self.completion);

    for command in commands.iter().chain(core::iter::once(&done)) {
      let Ok((first, second)) = command.words();
      let slot = self.commands.saturating_add(self.tail);
      let Ok(()) = Word(slot).write(first);
      let Ok(()) = Word(slot.saturating_add(8)).write(second);
      self.tail = self.tail.saturating_add(COMMAND_SLOT).rem_euclid(PAGE);
    }

    compiler_fence(Ordering::SeqCst);
    let Ok(()) = self.write(COMMAND_TAIL, self.tail);
    let Ok(start) = timestamp();

    for _poll in 0..WAIT_CYCLES {
      let Ok(completion) = Word(self.completion).read();
      let Ok(now) = timestamp();

      match (
        completion == COMPLETION_WRITTEN,
        now.wrapping_sub(start) >= WAIT_CYCLES,
      ) {
        (true, _) => return Ok(()),
        (false, true) => return Err(Error::Timeout),
        (false, false) => core::hint::spin_loop(),
      }
    }

    Err(Error::Timeout)
  }
}

enum Command {
  InvalidateAll,
  Device(Requester),
  Domain,
  Complete(u64),
}

impl Command {
  fn words(&self) -> Result<(u64, u64), Never> {
    Ok(match self {
      Self::InvalidateAll => (INVALIDATE_ALL, 0),
      Self::Device(requester) => (INVALIDATE_DEVICE | u64::from(requester.0), 0),
      Self::Domain => (INVALIDATE_DOMAIN, 0x7fff_ffff_ffff_f003),
      Self::Complete(address) => (WAIT_COMPLETE | address | 1, COMPLETION_WRITTEN),
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the page allocation's size crosses the Rust allocator ABI here"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  allow(
    explicit066_no_heap_after_init,
    reason = "these pages are reserved once at driver start and remain owned by the remapper until machine shutdown"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this reserves an aligned contiguous allocation for the DMA hardware"
  )
)]
pub(super) fn reserve(bytes: u64) -> Result<u64, Error> {
  let requested = usize::try_from(bytes).map_err(|_too_wide| Error::Allocation);
  let length = requested?;
  let aligned = Layout::from_size_align(length, 4096).map_err(|_invalid| Error::Allocation);
  let layout = aligned?;

  // SAFETY: the layout is nonempty for all callers, and these pages remain reserved while either firmware or a device might retain an address.
  let memory = unsafe { alloc_zeroed(layout) };
  let physical = u64::try_from(memory.expose_provenance()).map_err(|_too_wide| Error::Allocation);
  let address = physical?;

  match ::core::primitive::bool::then_some(address != 0 && address & MASK_ADDRESS == address, ()) {
    ::core::option::Option::Some(()) => Ok(address),
    ::core::option::Option::None => Err(Error::Allocation),
  }
}

impl Word {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the x86-64 physical address is converted at the pointer boundary"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this is the volatile AMD-Vi MMIO and table read"
    )
  )]
  fn read(self) -> Result<u64, Never> {
    let base = usize::from_le_bytes(self.0.to_le_bytes());

    // SAFETY: callers hold the AMD-Vi unit's firmware-declared MMIO window or a word in its reserved coherent memory.
    Ok(unsafe { ptr::with_exposed_provenance::<u64>(base).read_volatile() })
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the x86-64 physical address is converted at the pointer boundary"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this is the volatile AMD-Vi MMIO and table write"
    )
  )]
  fn write(self, value: u64) -> Result<(), Never> {
    let base = usize::from_le_bytes(self.0.to_le_bytes());

    // SAFETY: callers hold the AMD-Vi unit's firmware-declared MMIO window or a word in its reserved coherent memory.
    unsafe { ptr::with_exposed_provenance_mut::<u64>(base).write_volatile(value) };

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this samples the boot CPU TSC for the finite hardware deadline"
  )
)]
fn timestamp() -> Result<u64, Never> {
  // SAFETY: x86-64 supplies the TSC, and the boot task remains on its one processor.
  Ok(unsafe { _rdtsc() })
}
