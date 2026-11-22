//! Sparse-set nodes are packed least-significant bits first after their header.
//!
//! A packed node is placed in its byte by multiplying it by the place it takes,
//! and the place grows by the node's span each time: the byte is full exactly
//! when the next place would not fit in a byte, so the overflow that would have
//! been a fault is the signal to start the next one.

use alloc::vec::Vec;
use console_core_never::Never;

use super::sparse_bit_set::{BranchFactor, EncodingError};

pub(crate) struct OutputBitStream {
  data: Vec<u8>,
  pending: u8,
  place: u8,
  branch_factor: BranchFactor,
}

enum Packing {
  Packed { mask: u32, span: u8 },
  Byte,
  Word,
}

impl OutputBitStream {
  pub(crate) fn new(branch_factor: BranchFactor, height: u8) -> Result<Self, EncodingError> {
    let id = match branch_factor {
      BranchFactor::Two => 0,
      BranchFactor::Four => 1,
      BranchFactor::Eight => 2,
      BranchFactor::ThirtyTwo => 3,
    };
    let header = match height {
      0..=31 => height.saturating_mul(4) | id,
      _too_tall => return Err(EncodingError::HeightTooLarge(height)),
    };
    let data = alloc::vec![header];

    Ok(Self {
      data,
      pending: 0,
      place: 1,
      branch_factor,
    })
  }

  pub fn into_bytes(mut self) -> Result<Vec<u8>, Never> {
    match self.place {
      1 => {}
      _partial => self.data.push(self.pending),
    }

    Ok(self.data)
  }

  pub fn write_node(&mut self, bits: u32) -> Result<(), Never> {
    let packing = match self.branch_factor {
      BranchFactor::Two => Packing::Packed {
        mask: 0b11,
        span: 4,
      },
      BranchFactor::Four => Packing::Packed {
        mask: 0b1111,
        span: 16,
      },
      BranchFactor::Eight => Packing::Byte,
      BranchFactor::ThirtyTwo => Packing::Word,
    };

    match packing {
      Packing::Packed { mask, span } => {
        let [node, _, _, _] = (bits & mask).to_le_bytes();

        self.pending |= node.saturating_mul(self.place);

        match self.place.checked_mul(span) {
          Some(next) => self.place = next,
          None => {
            self.data.push(self.pending);
            self.pending = 0;
            self.place = 1;
          }
        }
      }
      Packing::Byte => {
        let [byte, _, _, _] = bits.to_le_bytes();

        self.data.push(byte);
      }
      Packing::Word => self.data.extend_from_slice(&bits.to_le_bytes()),
    }

    Ok(())
  }
}

#[cfg(test)]
#[allow(
  clippy::unusual_byte_groupings,
  reason = "bit groups follow the sparse set encoding"
)]
mod test {
  use super::*;

  #[test]
  fn init() -> Result<(), crate::TestFailure> {
    let os =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Two, 13))?;
    assert_eq!(os.into_bytes(), Ok(vec![0b0_01101_00]));
    let os_2 =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Four, 23))?;
    assert_eq!(os_2.into_bytes(), Ok(vec![0b0_10111_01]));
    let os_3 =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Eight, 1))?;
    assert_eq!(os_3.into_bytes(), Ok(vec![0b0_00001_10]));
    let os_4 =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::ThirtyTwo, 31))?;
    assert_eq!(os_4.into_bytes(), Ok(vec![0b0_11111_11]));

    Ok(())
  }

  #[test]
  fn bf2() -> Result<(), crate::TestFailure> {
    let mut os =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Two, 13))?;
    let Ok(()) = os.write_node(0b10);
    let Ok(()) = os.write_node(0b00);
    let Ok(()) = os.write_node(0b11);
    let Ok(()) = os.write_node(0b01);
    let Ok(()) = os.write_node(0b01);
    let Ok(()) = os.write_node(0b11);
    let Ok(bytes) = os.into_bytes();
    assert_eq!(bytes, vec![0b0_01101_00, 0b01_11_00_10, 0b00_00_11_01,]);

    Ok(())
  }

  #[test]
  fn bf4() -> Result<(), crate::TestFailure> {
    let mut os =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Four, 23))?;
    let Ok(()) = os.write_node(0b0010);
    let Ok(()) = os.write_node(0b0111);
    let Ok(()) = os.write_node(0b1101);
    let Ok(bytes) = os.into_bytes();
    assert_eq!(bytes, vec![0b0_10111_01, 0b0111_0010, 0b0000_1101,]);

    Ok(())
  }

  #[test]
  fn bf8() -> Result<(), crate::TestFailure> {
    let mut os =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Eight, 1))?;
    let Ok(()) = os.write_node(0b01110010);
    let Ok(()) = os.write_node(0b00001101);
    let Ok(bytes) = os.into_bytes();
    assert_eq!(bytes, vec![0b0_00001_10, 0b01110010, 0b00001101,]);

    Ok(())
  }

  #[test]
  fn bf32() -> Result<(), crate::TestFailure> {
    let mut os =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::ThirtyTwo, 31))?;
    let Ok(()) = os.write_node(0b10000000_00000000_00001101_01110010);
    let Ok(bytes) = os.into_bytes();
    assert_eq!(
      bytes,
      vec![0b0_11111_11, 0b01110010, 0b00001101, 0b00000000, 0b10000000]
    );

    Ok(())
  }

  #[test]
  fn truncating() -> Result<(), crate::TestFailure> {
    let mut os =
      crate::test_support::Required::required(OutputBitStream::new(BranchFactor::Four, 23))?;
    let Ok(()) = os.write_node(0b11110010);
    let Ok(bytes) = os.into_bytes();
    assert_eq!(bytes, vec![0b0_10111_01, 0b0000_0010]);

    Ok(())
  }
}
