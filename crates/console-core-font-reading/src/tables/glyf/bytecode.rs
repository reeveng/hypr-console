//! TrueType hinting bytecode.

mod decode;
mod instruction;
mod opcode;

pub use decode::{DecodeError, Decoder, decode_all};
pub use instruction::{InlineOperands, Instruction};
pub use opcode::{Opcode, OperandEncoding};

#[cfg(any(test, feature = "scaler_test"))]
pub use instruction::MockInlineOperands;
