//! xHCI USB Legacy Support handoff, adapted from nos. See `docs/third-party/nos.txt`.
//! Only the firmware can clear the BIOS-owned semaphore. On timeout this
//! driver leaves the controller to firmware.

use core::arch::x86_64::_rdtsc;

use console_core_never::Never;

use super::Error;
use super::ring::{read32, write32};

const LEGACY: u32 = 1;
const BIOS_OWNED: u32 = 1 << 16;
const OS_OWNED: u32 = 1 << 24;
const CONTROL_KEEP: u32 = 0x000e_1fee;
const ACK_SMI: u32 = 0xe000_0000;
const WAIT_CYCLES: u64 = 6_000_000_000;

pub(super) struct Capabilities {
  pub(super) base: u64,
  pub(super) parameters: u32,
}

impl Capabilities {
  pub(super) fn claim(self) -> Result<(), Error> {
    let mut offset = u64::from(self.parameters.wrapping_shr(16)).saturating_mul(4);

    for _entry in 0..64_u8 {
      match offset {
        0 => return Ok(()),
        0x20..=0x3ff8 => {}
        _ => return Err(Error::Registers),
      }

      let capability = self.base.saturating_add(offset);
      let Ok(flags) = Register(capability).read();

      match flags & 0xff {
        LEGACY => return claim_legacy(capability, flags),
        _ => {}
      }

      let next = u64::from(flags.wrapping_shr(8) & 0xff);

      match next {
        0 => return Ok(()),
        step => offset = offset.saturating_add(step.saturating_mul(4)),
      }
    }

    Err(Error::Registers)
  }
}

fn claim_legacy(address: u64, flags: u32) -> Result<(), Error> {
  let support = Register(address);
  let Ok(()) = support.write(flags | OS_OWNED);
  let Ok(start) = timestamp();

  for _poll in 0..WAIT_CYCLES {
    let Ok(current) = support.read();
    let Ok(now) = timestamp();

    match (
      current & (BIOS_OWNED | OS_OWNED),
      now.wrapping_sub(start) >= WAIT_CYCLES,
    ) {
      (OS_OWNED, _) => {
        let control = Register(address.saturating_add(4));
        let Ok(settings) = control.read();
        let Ok(()) = control.write((settings & CONTROL_KEEP) | ACK_SMI);

        return Ok(());
      }
      (_, true) => break,
      (_, false) => core::hint::spin_loop(),
    }
  }

  Err(Error::Timeout("USB firmware ownership handoff"))
}

struct Register(u64);

impl Register {
  fn read(&self) -> Result<u32, Never> {
    read32(self.0)
  }

  fn write(&self, flags: u32) -> Result<(), Never> {
    write32(self.0, flags)
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this samples the TSC on the boot CPU for the bounded firmware handshake"
  )
)]
fn timestamp() -> Result<u64, Never> {
  // SAFETY: the x86-64 driver polls on the boot CPU and its TSC cannot migrate.
  Ok(unsafe { _rdtsc() })
}
