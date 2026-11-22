//! Provides serialization of [`IntSet`]'s to a highly compact bitset format as defined in the
//! IFT specification:
//!
//! <https://w3c.github.io/IFT/Overview.html#sparse-bit-set-decoding>
//!
//! A tree's width is the branch factor raised to its height, and the header's
//! five bits let a height ask for more than any width a `u64` holds; that power
//! is checked, and a height whose width does not fit is the encoding's fault or
//! the input's rather than a number that wrapped. Below the root every width is
//! a quotient of the one above it and every child's first value a sum inside
//! its parent, so those never reach the edge they saturate at.

use alloc::{collections::VecDeque, vec::Vec};
use core::cmp::Ordering;
use core::num::{NonZeroU32, NonZeroU64};
use std::error::Error;
use std::fmt;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::IntSet;
use super::U32Set;
use super::input_bit_stream::{Factor, InputBitStream, SupportedFactor, decode_header};
use super::output_bit_stream::OutputBitStream;

#[derive(Debug, PartialEq)]
pub struct DecodingError;

impl Error for DecodingError {}

impl fmt::Display for DecodingError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "The input data stream was too short to be a valid sparse bit set."
    )
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum BranchFactor {
  Two,
  Four,
  Eight,
  ThirtyTwo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingError {
  UnsupportedBranchFactor(u8),
  HeightTooLarge(u8),
}

impl fmt::Display for EncodingError {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::UnsupportedBranchFactor(value) => {
        write!(output, "unsupported sparse-set branch factor {value}")
      }
      Self::HeightTooLarge(height) => write!(
        output,
        "sparse-set height {height} exceeds the header width"
      ),
    }
  }
}

impl core::error::Error for EncodingError {}

#[derive(Clone, Copy, Debug)]
pub struct DecodingBounds {
  pub bias: u32,
  pub maximum: u32,
}

impl IntSet<u32> {
  pub fn from_sparse_bit_set(data: &[u8]) -> Result<IntSet<u32>, DecodingError> {
    Self::from_sparse_bit_set_bounded(
      data,
      DecodingBounds {
        bias: 0,
        maximum: u32::MAX,
      },
    )
    .map(|(set, _remaining)| set)
  }

  pub fn from_sparse_bit_set_bounded(
    data: &[u8],
    bounds: DecodingBounds,
  ) -> Result<(IntSet<u32>, &[u8]), DecodingError> {
    let (factor, height) = decode_header(data)?;
    let Ok(limit) = factor.max_height();

    match height.cmp(&limit) {
      Ordering::Less | Ordering::Equal => {}
      Ordering::Greater => return Err(DecodingError),
    }

    let (bits, remaining) = match factor {
      BranchFactor::Two => decode_nodes::<2>(data, height, bounds),
      BranchFactor::Four => decode_nodes::<4>(data, height, bounds),
      BranchFactor::Eight => decode_nodes::<8>(data, height, bounds),
      BranchFactor::ThirtyTwo => decode_nodes::<32>(data, height, bounds),
    }?;
    let Ok(set) = IntSet::from_bitset(bits);

    Ok((set, remaining))
  }

  pub fn to_sparse_bit_set(&self) -> Result<Vec<u8>, EncodingError> {
    let Ok(ranges) = EncodingRanges::new(self);
    let mut best: Option<Vec<u8>> = None;

    for factor in [
      BranchFactor::Two,
      BranchFactor::Four,
      BranchFactor::Eight,
      BranchFactor::ThirtyTwo,
    ] {
      let Ok(height) = ranges.height(factor);
      let Ok(limit) = factor.max_height();

      match height.cmp(&limit) {
        Ordering::Less | Ordering::Equal => {
          let candidate = ranges.encode(factor)?;
          let length_order = match &best {
            Some(previous) => candidate.len().cmp(&previous.len()),
            None => Ordering::Less,
          };

          match length_order {
            Ordering::Less => best = Some(candidate),
            Ordering::Equal | Ordering::Greater => {}
          }
        }
        Ordering::Greater => {}
      }
    }

    match best {
      Some(bytes) => Ok(bytes),
      None => Err(EncodingError::HeightTooLarge(32)),
    }
  }
}

pub fn to_sparse_bit_set_with_bf<const BF: u8>(
  set: &IntSet<u32>,
) -> Result<Vec<u8>, EncodingError> {
  let requested = BranchFactor::from_val(BF)?;
  let Ok(ranges) = EncodingRanges::new(set);
  let Ok(height) = ranges.height(requested);
  let Ok(limit) = requested.max_height();
  let factor = match (requested, height.cmp(&limit)) {
    (BranchFactor::Two, Ordering::Greater) => BranchFactor::Four,
    (BranchFactor::Two, Ordering::Less | Ordering::Equal)
    | (
      BranchFactor::Four | BranchFactor::Eight | BranchFactor::ThirtyTwo,
      Ordering::Less | Ordering::Equal | Ordering::Greater,
    ) => requested,
  };

  ranges.encode(factor)
}

struct EncodingRanges {
  members: Vec<core::ops::RangeInclusive<u64>>,
}

#[derive(Clone, Copy)]
struct Extent {
  first: u64,
  width: u64,
}

enum Coverage {
  Empty,
  Full,
  Partial,
}

enum Layer {
  Children,
  Leaves,
}

struct TreeWriter<'a> {
  ranges: &'a EncodingRanges,
  output: OutputBitStream,
  factor: BranchFactor,
}

impl Extent {
  fn root(branches: NonZeroU32, height: u8) -> Result<Option<Self>, Never> {
    let width = u64::from(branches.get()).checked_pow(u32::from(height));

    Ok(width.map(|width_2| Self {
      first: 0,
      width: width_2,
    }))
  }

  fn children(self, branches: NonZeroU32) -> Result<impl Iterator<Item = (u32, Self)>, Never> {
    let width = self.width / NonZeroU64::from(branches);
    let children = (0..branches.get()).map_while(move |child| {
      let bit = 1u32.checked_shl(child)?;
      let displacement = width.checked_mul(u64::from(child))?;
      let first = self.first.checked_add(displacement)?;

      Some((bit, Self { first, width }))
    });

    Ok(children)
  }

  fn leaf(&self) -> Result<Leaf, Never> {
    Ok(match self.width {
      1 => Leaf::Value,
      _wider => Leaf::Subtree,
    })
  }
}

enum Leaf {
  Value,
  Subtree,
}

impl EncodingRanges {
  fn new(set: &IntSet<u32>) -> Result<Self, Never> {
    let Ok(ranges) = set.iter_ranges();
    let members = ranges
      .map(|range| u64::from(*range.start())..=u64::from(*range.end()))
      .collect();

    Ok(Self { members })
  }

  fn height(&self, factor: BranchFactor) -> Result<u8, Never> {
    match self.members.last() {
      Some(range) => {
        let Ok(maximum) = fitted(*range.end());

        factor.tree_height_for(maximum)
      }
      None => Ok(0),
    }
  }

  fn coverage(&self, node: Extent) -> Result<Coverage, Never> {
    let last = node.first.saturating_add(node.width.saturating_sub(1));

    Ok(
      match self.members.get(
        self
          .members
          .partition_point(|range| *range.end() < node.first),
      ) {
        Some(range) => match range.start().cmp(&last) {
          Ordering::Greater => Coverage::Empty,
          Ordering::Less | Ordering::Equal => {
            match (range.start().cmp(&node.first), range.end().cmp(&last)) {
              (Ordering::Less | Ordering::Equal, Ordering::Equal | Ordering::Greater) => {
                Coverage::Full
              }
              (Ordering::Greater, Ordering::Less | Ordering::Equal | Ordering::Greater)
              | (Ordering::Less | Ordering::Equal, Ordering::Less) => Coverage::Partial,
            }
          }
        },
        None => Coverage::Empty,
      },
    )
  }

  fn encode(&self, factor: BranchFactor) -> Result<Vec<u8>, EncodingError> {
    let Ok(height) = self.height(factor);
    let output = OutputBitStream::new(factor, height)?;
    let mut writer = TreeWriter {
      ranges: self,
      output,
      factor,
    };
    let Ok(branches) = factor.value();
    let Ok(root) = Extent::root(branches, height);
    let root_3 = match root {
      Some(root_2) => root_2,
      None => return Err(EncodingError::HeightTooLarge(height)),
    };
    let mut current = alloc::vec![root_3];

    for depth in 1..=height {
      let layer = match depth.cmp(&height) {
        Ordering::Equal => Layer::Leaves,
        Ordering::Less | Ordering::Greater => Layer::Children,
      };
      let Ok(next) = writer.layer(current, layer);

      current = next;
    }

    let Ok(bytes) = writer.output.into_bytes();

    Ok(bytes)
  }
}

impl TreeWriter<'_> {
  fn layer(&mut self, current: Vec<Extent>, layer: Layer) -> Result<Vec<Extent>, Never> {
    let mut next = Vec::new();

    for node in current {
      let Ok(coverage) = self.ranges.coverage(node);

      match coverage {
        Coverage::Full => {
          let Ok(()) = self.output.write_node(0);
        }
        Coverage::Partial => {
          let Ok(()) = self.split(node, &layer, &mut next);
        }
        Coverage::Empty => {}
      }
    }

    Ok(next)
  }

  fn split(&mut self, node: Extent, layer: &Layer, next: &mut Vec<Extent>) -> Result<(), Never> {
    let Ok(branches) = self.factor.value();
    let Ok(children) = node.children(branches);
    let mut mask = 0u32;

    for (bit, child) in children {
      let Ok(coverage) = self.ranges.coverage(child);

      match coverage {
        Coverage::Empty => {}
        Coverage::Full | Coverage::Partial => {
          mask |= bit;

          match layer {
            Layer::Children => next.push(child),
            Layer::Leaves => {}
          }
        }
      }
    }

    self.output.write_node(mask)
  }
}

const TWO: NonZeroU32 = match NonZeroU32::new(2) {
  Some(branches) => branches,
  None => NonZeroU32::MIN,
};

const FOUR: NonZeroU32 = match NonZeroU32::new(4) {
  Some(branches) => branches,
  None => NonZeroU32::MIN,
};

const EIGHT: NonZeroU32 = match NonZeroU32::new(8) {
  Some(branches) => branches,
  None => NonZeroU32::MIN,
};

const THIRTY_TWO: NonZeroU32 = match NonZeroU32::new(32) {
  Some(branches) => branches,
  None => NonZeroU32::MIN,
};

impl BranchFactor {
  pub(crate) fn value(&self) -> Result<NonZeroU32, Never> {
    Ok(match self {
      BranchFactor::Two => TWO,
      BranchFactor::Four => FOUR,
      BranchFactor::Eight => EIGHT,
      BranchFactor::ThirtyTwo => THIRTY_TWO,
    })
  }

  pub(crate) fn max_height(&self) -> Result<u8, Never> {
    Ok(match self {
      BranchFactor::Two => 31,
      BranchFactor::Four => 16,
      BranchFactor::Eight => 11,
      BranchFactor::ThirtyTwo => 7,
    })
  }

  fn tree_height_for(&self, maximum: u32) -> Result<u8, Never> {
    let Ok(levels) = self.bits_per_level();
    let bits = match NonZeroU32::new(u32::BITS.saturating_sub(maximum.leading_zeros())) {
      Some(bits) => bits,
      None => NonZeroU32::MIN,
    };

    fitted(bits.div_ceil(levels).get())
  }

  fn from_val(val: u8) -> Result<BranchFactor, EncodingError> {
    match val {
      2 => Ok(BranchFactor::Two),
      4 => Ok(BranchFactor::Four),
      8 => Ok(BranchFactor::Eight),
      32 => Ok(BranchFactor::ThirtyTwo),
      _unsupported => Err(EncodingError::UnsupportedBranchFactor(val)),
    }
  }

  fn bits_per_level(&self) -> Result<NonZeroU32, Never> {
    Ok(match self {
      BranchFactor::Two => NonZeroU32::MIN,
      BranchFactor::Four => TWO,
      BranchFactor::Eight => NonZeroU32::MIN.saturating_add(2),
      BranchFactor::ThirtyTwo => NonZeroU32::MIN.saturating_add(4),
    })
  }
}

struct TreeReader {
  pending: VecDeque<Extent>,
  members: U32Set,
  bounds: DecodingBounds,
  branches: NonZeroU32,
}

fn decode_nodes<const BF: u8>(
  data: &[u8],
  height: u8,
  bounds: DecodingBounds,
) -> Result<(U32Set, &[u8]), DecodingError>
where
  Factor<BF>: SupportedFactor,
{
  let Ok(members) = U32Set::empty();
  let branches = <Factor<BF> as SupportedFactor>::BRANCHES;
  let mut reader = TreeReader {
    pending: VecDeque::new(),
    members,
    bounds,
    branches,
  };
  let mut stream = InputBitStream::<BF>::from(data);

  match height {
    0 => {}
    _nonempty => {
      let Ok(root) = Extent::root(branches, height);
      let root_2 = root.ok_or(DecodingError)?;

      reader.pending.push_back(root_2);
    }
  }

  let Ok(bytes) = fitted::<_, u64>(data.len());
  let node_budget = bytes.saturating_sub(1).saturating_mul(8) / NonZeroU64::from(branches);

  'nodes: for _node in 0..node_budget {
    let extent = match reader.pending.pop_front() {
      Some(extent) => extent,
      None => break 'nodes,
    };
    let mask = stream.next().ok_or(DecodingError)?;
    let Ok(()) = reader.node(extent, mask);
  }

  match reader.pending.front() {
    Some(_unread) => return Err(DecodingError),
    None => {}
  }

  let Ok(consumed) = stream.bytes_consumed();
  let Ok(at) = index(consumed);
  let remaining = data.get(at..).ok_or(DecodingError)?;

  Ok((reader.members, remaining))
}

impl TreeReader {
  fn node(&mut self, extent: Extent, mask: u32) -> Result<(), Never> {
    match mask {
      0 => self.insert(extent),
      _children => {
        let Ok(children) = extent.children(self.branches);

        for (bit, child) in children {
          let Ok(leaf) = child.leaf();

          match (mask & bit, leaf) {
            (0, Leaf::Value | Leaf::Subtree) => {}
            (_set, Leaf::Value) => {
              let Ok(()) = self.insert(child);
            }
            (_set, Leaf::Subtree) => self.pending.push_back(child),
          }
        }

        Ok(())
      }
    }
  }

  fn insert(&mut self, extent: Extent) -> Result<(), Never> {
    let first = extent.first.saturating_add(u64::from(self.bounds.bias));
    let last = first
      .saturating_add(extent.width.saturating_sub(1))
      .min(u64::from(self.bounds.maximum));

    match first.cmp(&last) {
      Ordering::Less | Ordering::Equal => {
        let Ok(first_2) = fitted(first);
        let Ok(last_2) = fitted(last);

        self.members.insert_range(first_2..=last_2)
      }
      Ordering::Greater => Ok(()),
    }
  }
}

#[cfg(test)]
#[allow(
  clippy::unusual_byte_groupings,
  reason = "bit groups follow the sparse set encoding"
)]
mod test {
  use super::*;
  use crate::Emptiness;
  use crate::test_support::Required;

  #[test]
  fn a_full_domain_is_encoded_from_ranges() -> Result<(), crate::TestFailure> {
    let Ok(all) = IntSet::<u32>::all();
    let encoded = all.to_sparse_bit_set().required()?;
    let requested_two = to_sparse_bit_set_with_bf::<2>(&all).required()?;
    assert_eq!(encoded, [0x41, 0]);
    assert_eq!(requested_two, encoded);
    let (bounded, remaining) = IntSet::<u32>::from_sparse_bit_set_bounded(
      &encoded,
      DecodingBounds {
        bias: 0,
        maximum: 511,
      },
    )
    .required()?;
    let Ok(length) = bounded.len();
    let Ok(values) = bounded.iter();
    assert_eq!(length, 512);
    assert!(values.eq(0..512));
    assert!(remaining.is_empty());
    assert_eq!(
      to_sparse_bit_set_with_bf::<3>(&all),
      Err(EncodingError::UnsupportedBranchFactor(3))
    );
    assert!(matches!(
      OutputBitStream::new(BranchFactor::Two, 32),
      Err(EncodingError::HeightTooLarge(32))
    ));

    Ok(())
  }

  #[test]
  fn spec_example_2() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = [
      0b00001110, 0b00100001, 0b00010001, 0b00000001, 0b00000100, 0b00000010, 0b00001000,
    ];

    let set = IntSet::<u32>::from_sparse_bit_set(&bytes).required()?;
    let expected: IntSet<u32> = [2, 33, 323].iter().copied().collect();
    assert_eq!(set, expected);

    Ok(())
  }

  #[test]
  fn spec_example_3() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = [0b00000000];
    let set = IntSet::<u32>::from_sparse_bit_set(&bytes).required()?;
    let expected: IntSet<u32> = [].iter().copied().collect();
    assert_eq!(set, expected);

    Ok(())
  }

  #[test]
  fn spec_example_4() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = [0b00001101, 0b00000011, 0b00110001];
    let set = IntSet::<u32>::from_sparse_bit_set(&bytes).required()?;
    let Ok(empty) = IntSet::<u32>::empty();
    let mut expected: IntSet<u32> = empty;
    let Ok(()) = expected.insert_range(0..=17);
    assert_eq!(set, expected);

    Ok(())
  }

  #[test]
  fn invalid() -> Result<(), crate::TestFailure> {
    let bytes = [
      0b00001110, 0b00100001, 0b00010001, 0b00000001, 0b00000100, 0b00000010,
    ];
    assert_eq!(
      IntSet::<u32>::from_sparse_bit_set(&bytes),
      Err(DecodingError)
    );
    let bytes_2 = [
      0b0_01000_11,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000001,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
    ];
    assert_eq!(
      IntSet::<u32>::from_sparse_bit_set(&bytes_2),
      Err(DecodingError)
    );

    Ok(())
  }

  #[test]
  fn invalid_biased_and_bounded() -> Result<(), crate::TestFailure> {
    let bytes = [0b0_00011_01, 0b0000_0011, 0b1111_0011];

    for (bias, maximum) in [
      (0, u32::MAX),
      (0, 20),
      (0, 19),
      (0, 18),
      (0, 15),
      (0, 14),
      (1, 20),
      (2, 20),
      (3, 20),
      (6, 20),
    ] {
      assert_eq!(
        IntSet::<u32>::from_sparse_bit_set_bounded(&bytes, DecodingBounds { bias, maximum }),
        Err(DecodingError),
        "bias {bias}, maximum {maximum}"
      );
    }

    Ok(())
  }

  #[test]
  fn larger_than_u32() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = [
      0b0_00111_11,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000001,
    ];

    {
      let queried_value = IntSet::<u32>::from_sparse_bit_set(&bytes).required()?;
      let Ok(empty) = IntSet::<u32>::empty();
      assert_eq!(queried_value, empty);
    }

    let bytes_2 = [
      0b0_00111_11,
      0b00000000,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000000,
    ];

    {
      let queried_value = IntSet::<u32>::from_sparse_bit_set(&bytes_2).required()?;
      let Ok(empty) = IntSet::<u32>::empty();
      assert_eq!(queried_value, empty);
    }

    Ok(())
  }

  #[test]
  fn from_sparse_bit_set_bounded_with_remaining_data() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = [0b00001101, 0b00000011, 0b00110001, 0b10101010];
    let Ok(empty) = IntSet::<u32>::empty();
    let mut expected: IntSet<u32> = empty;
    let Ok(()) = expected.insert_range(0..=17);

    {
      let queried_value = IntSet::<u32>::from_sparse_bit_set_bounded(
        &bytes,
        DecodingBounds {
          bias: 0,
          maximum: 19,
        },
      )
      .required()?;
      let trailing = bytes.get(3..).required()?;
      assert_eq!(queried_value, (expected.clone(), trailing));
    }

    Ok(())
  }

  #[test]
  fn from_sparse_bit_set_biased_and_bounded() -> Result<(), crate::TestFailure> {
    let bytes = [0b0_00011_01, 0b0000_0011, 0b1111_0011, 0b0000_0001];
    let cases = [
      DecodingBounds {
        bias: 0,
        maximum: 20,
      },
      DecodingBounds {
        bias: 0,
        maximum: 19,
      },
      DecodingBounds {
        bias: 1,
        maximum: 20,
      },
      DecodingBounds {
        bias: 1,
        maximum: 18,
      },
      DecodingBounds {
        bias: 0,
        maximum: 14,
      },
      DecodingBounds {
        bias: 6,
        maximum: 20,
      },
      DecodingBounds {
        bias: 0,
        maximum: 0,
      },
      DecodingBounds {
        bias: 1,
        maximum: 0,
      },
    ];

    for bounds in cases {
      let (set, remaining) =
        IntSet::<u32>::from_sparse_bit_set_bounded(&bytes, bounds).required()?;
      let expected = IntSet::from_iter(bounds.bias..=bounds.maximum);
      assert_eq!(set, expected, "{bounds:?}");
      assert!(remaining.is_empty());
    }

    let (set, remaining) = IntSet::<u32>::from_sparse_bit_set_bounded(
      &[0],
      DecodingBounds {
        bias: 5,
        maximum: 0,
      },
    )
    .required()?;
    assert_eq!(set.is_empty(), Ok(Emptiness::Empty));
    assert!(remaining.is_empty());

    Ok(())
  }

  #[test]
  fn test_tree_height_for() -> Result<(), crate::TestFailure> {
    assert_eq!(BranchFactor::Two.tree_height_for(0), Ok(1));
    assert_eq!(BranchFactor::Two.tree_height_for(1), Ok(1));
    assert_eq!(BranchFactor::Two.tree_height_for(2), Ok(2));
    assert_eq!(BranchFactor::Two.tree_height_for(117), Ok(7));
    assert_eq!(BranchFactor::Four.tree_height_for(0), Ok(1));
    assert_eq!(BranchFactor::Four.tree_height_for(3), Ok(1));
    assert_eq!(BranchFactor::Four.tree_height_for(4), Ok(2));
    assert_eq!(BranchFactor::Four.tree_height_for(63), Ok(3));
    assert_eq!(BranchFactor::Four.tree_height_for(64), Ok(4));
    assert_eq!(BranchFactor::Eight.tree_height_for(0), Ok(1));
    assert_eq!(BranchFactor::Eight.tree_height_for(7), Ok(1));
    assert_eq!(BranchFactor::Eight.tree_height_for(8), Ok(2));
    assert_eq!(BranchFactor::Eight.tree_height_for(32767), Ok(5));
    assert_eq!(BranchFactor::Eight.tree_height_for(32768), Ok(6));
    assert_eq!(BranchFactor::ThirtyTwo.tree_height_for(0), Ok(1));
    assert_eq!(BranchFactor::ThirtyTwo.tree_height_for(31), Ok(1));
    assert_eq!(BranchFactor::ThirtyTwo.tree_height_for(32), Ok(2));
    assert_eq!(BranchFactor::ThirtyTwo.tree_height_for(1_048_575), Ok(4));
    assert_eq!(BranchFactor::ThirtyTwo.tree_height_for(1_048_576), Ok(5));

    Ok(())
  }

  #[test]
  fn generate_spec_example_2() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<8>(
      &[2, 33, 323].iter().copied().collect(),
    ))?;
    let expected_bytes = [
      0b00001110, 0b00100001, 0b00010001, 0b00000001, 0b00000100, 0b00000010, 0b00001000,
    ];

    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn generate_spec_example_3() -> Result<(), crate::TestFailure> {
    let Ok(empty) = IntSet::<u32>::empty();
    let actual_bytes =
      crate::test_support::Required::required(to_sparse_bit_set_with_bf::<2>(&empty))?;
    let expected_bytes = [0b00000000];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn generate_spec_example_4() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<4>(
      &(0..=17).collect(),
    ))?;
    let expected_bytes = [0b00001101, 0b0000_0011, 0b0011_0001];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn encode_one_level() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<8>(
      &[2, 6].iter().copied().collect(),
    ))?;
    let expected_bytes = [0b0_00001_10, 0b01000100];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn encode_one_level_filled() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<8>(
      &(0..=7).collect(),
    ))?;
    let expected_bytes = [0b0_00001_10, 0b00000000];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn encode_two_level_filled() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<8>(
      &(3..=21).collect(),
    ))?;
    let expected_bytes = [0b0_00010_10, 0b00000111, 0b11111000, 0b00000000, 0b00111111];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn encode_two_level_not_filled() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<4>(
      &[0, 4, 8, 12].iter().copied().collect(),
    ))?;
    let expected_bytes = [0b0_00010_01, 0b0001_1111, 0b0001_0001, 0b0000_0001];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn encode_four_level_filled() -> Result<(), crate::TestFailure> {
    let Ok(mut s) = IntSet::<u32>::empty();
    let Ok(()) = s.insert_range(64..=127);
    let Ok(()) = s.insert_range(512..=1023);
    let Ok(_before) = s.insert(4000);
    let actual_bytes =
      crate::test_support::Required::required(to_sparse_bit_set_with_bf::<8>(&s))?;
    let expected_bytes = [
      0b0_00100_10,
      0b10000011,
      0b00000010,
      0b00000000,
      0b01000000,
      0b00000000,
      0b00010000,
      0b00000001,
    ];
    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn encode_bf32() -> Result<(), crate::TestFailure> {
    let actual_bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<32>(
      &[2, 31, 323].iter().copied().collect(),
    ))?;
    let expected_bytes = [
      0b0_00010_11,
      0b00000001,
      0b00000100,
      0b00000000,
      0b00000000,
      0b00000100,
      0b00000000,
      0b00000000,
      0b10000000,
      0b00001000,
      0b00000000,
      0b00000000,
      0b00000000,
    ];

    assert_eq!(actual_bytes, expected_bytes);

    Ok(())
  }

  #[test]
  fn round_trip() -> Result<(), crate::TestFailure> {
    let s1: IntSet<u32> = [11, 74, 9358].iter().copied().collect();
    let mut s2: IntSet<u32> = s1.clone();
    let Ok(()) = s2.insert_range(67..=412);

    check_round_trip::<2>(&s1)?;
    check_round_trip::<4>(&s1)?;
    check_round_trip::<8>(&s1)?;
    check_round_trip::<32>(&s1)?;

    check_round_trip::<2>(&s2)?;
    check_round_trip::<4>(&s2)?;
    check_round_trip::<8>(&s2)?;
    check_round_trip::<32>(&s2)?;
    Ok(())
  }

  fn check_round_trip<const BF: u8>(s: &IntSet<u32>) -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let bytes = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<BF>(s))?;
    let s_prime = IntSet::<u32>::from_sparse_bit_set(&bytes).required()?;
    assert_eq!(*s, s_prime);
    Ok(())
  }

  #[test]
  fn find_smallest_bf() -> Result<(), crate::TestFailure> {
    let s: IntSet<u32> = [11, 74, 9358].iter().copied().collect();
    let bytes = crate::test_support::Required::required(s.to_sparse_bit_set())?;
    let header = bytes.get(0..1).required()?;
    assert_eq!(vec![0b0_00111_01], header);
    let s_2: IntSet<u32> = [
      16, 0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30,
    ]
    .iter()
    .copied()
    .collect();
    let bytes_2 = crate::test_support::Required::required(s_2.to_sparse_bit_set())?;
    let header_2 = bytes_2.get(0..1).required()?;
    assert_eq!(vec![0b0_00001_11], header_2);

    Ok(())
  }

  #[test]
  fn encode_maxu32() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let s: IntSet<u32> = [1, u32::MAX].iter().copied().collect();
    let bytes = crate::test_support::Required::required(s.to_sparse_bit_set())?;
    let s_prime = IntSet::<u32>::from_sparse_bit_set(&bytes);
    let decoded = s_prime.required()?;
    assert_eq!(s, decoded);
    let s_2: IntSet<u32> = [1, u32::MAX].iter().copied().collect();
    let bytes_2 = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<2>(&s_2))?;
    let s_prime_2 = IntSet::<u32>::from_sparse_bit_set(&bytes_2);
    let decoded_2 = s_prime_2.required()?;
    assert_eq!(s_2, decoded_2);
    let s_3: IntSet<u32> = [1, u32::MAX].iter().copied().collect();
    let bytes_3 = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<4>(&s_3))?;
    let s_prime_3 = IntSet::<u32>::from_sparse_bit_set(&bytes_3);
    let decoded_3 = s_prime_3.required()?;
    assert_eq!(s_3, decoded_3);
    let s_4: IntSet<u32> = [1, u32::MAX].iter().copied().collect();
    let bytes_4 = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<8>(&s_4))?;
    let s_prime_4 = IntSet::<u32>::from_sparse_bit_set(&bytes_4);
    let decoded_4 = s_prime_4.required()?;
    assert_eq!(s_4, decoded_4);
    let s_5: IntSet<u32> = [1, u32::MAX].iter().copied().collect();
    let bytes_5 = crate::test_support::Required::required(to_sparse_bit_set_with_bf::<32>(&s_5))?;
    let s_prime_5 = IntSet::<u32>::from_sparse_bit_set(&bytes_5);
    let decoded_5 = s_prime_5.required()?;
    assert_eq!(s_5, decoded_5);

    Ok(())
  }
}
