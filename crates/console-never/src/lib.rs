//! The error of a function that cannot fail.
//!
//! EXPLICIT002 asks that an infallible function still say `Result<T, Never>`,
//! so that a call site reads the same whether or not the thing it calls can go
//! wrong, and so that a function which later learns how to fail changes
//! nothing about the shape of the code that calls it. Until this crate the
//! rule had nowhere to point: there was no such type here, and it sat
//! registered `Allow` inside its own suite for exactly as long as that was
//! true.
//!
//! It is an enum with no variants, because that is the only way to tell the
//! compiler that no value of the type can ever be made. Everything else
//! follows from that: `Err` is unconstructible, so a `match` on the result of
//! an infallible function has one arm and needs no other, and the error a
//! caller does not write is a case the compiler agrees cannot arrive rather
//! than one nobody got round to. `absurd` is the way out where a value has to
//! be produced from an error that was never made.
//!
//! **What this crate does not cost.** `Result<T, Never>` is laid out as `T` --
//! an uninhabited variant takes no room and needs no tag -- so saying it
//! everywhere is a sentence in the signature and not a byte in the program.
//! That is a claim about the compiler rather than about this file, so it is
//! held by a test below rather than asserted here.
//!
//! **Why it is not `!`.** EXPLICIT003 forbids `Result<T, !>`, and the two
//! rules are not in disagreement: what 003 is about is a `Result` written with
//! the primitive never type, which on stable Rust is not a type anybody may
//! write in that position anyway, and which says nothing to a reader about
//! whether the absence was meant. A name says it. `Never` in a signature reads
//! as *this cannot fail*, which is a promise; `!` reads as an argument with
//! the compiler.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Never {}

impl Never {
    pub fn absurd(self) -> ! {
        match self {}
    }
}

impl fmt::Display for Never {
    fn fmt(&self, _to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

impl std::error::Error for Never {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn a_result_that_cannot_fail_is_the_size_of_the_value() {
        assert_eq!(size_of::<Result<u8, Never>>(), size_of::<u8>());
        assert_eq!(size_of::<Result<u64, Never>>(), size_of::<u64>());
        assert_eq!(size_of::<Result<String, Never>>(), size_of::<String>());
        assert_eq!(size_of::<Result<(), Never>>(), size_of::<()>());
    }

    #[test]
    fn the_error_arm_is_one_nobody_has_to_write() {
        let answered: Result<u8, Never> = Ok(7);

        let Ok(value) = answered;

        assert_eq!(value, 7);
    }
}
