#![no_std]

#[path = "../src/arithmetic.rs"]
mod arithmetic;

use core::arch::{asm, naked_asm};

// Console enters with the stack aligned to 16 bytes and the request in RDI.
// `call` establishes the SysV callee alignment before entering Rust.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
extern "sysv64" fn entry() -> ! {
  naked_asm!("call {}", sym calculate);
}

extern "sysv64" fn calculate(request: u64) -> ! {
  let [left_lo, left_hi, right_lo, right_hi, operator, ..] = request.to_le_bytes();
  let left = f64::from(u16::from_le_bytes([left_lo, left_hi]));
  let right = f64::from(u16::from_le_bytes([right_lo, right_hi]));
  let answer = arithmetic::calculate(left, operator, right).unwrap_or(f64::NAN);

  // SAFETY: Console installs interrupt 0x80 as its user exit gate. Call zero
  // returns the bits in RDI to the kernel, ending this invocation.
  unsafe { asm!("int 0x80", in("rax") 0_u64, in("rdi") answer.to_bits(), options(noreturn)) }
}
