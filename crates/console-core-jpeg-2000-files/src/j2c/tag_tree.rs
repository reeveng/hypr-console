//! The tag tree, described in Section B.10.2.
//!
//! Each leaf stores an integer value. Each intermediate node stores the smallest
//! value of its children. Levels are stored from root to leaves in row-major order.

use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_size::Size;

use crate::error::{Result, ValidationError};
use crate::reader::BitReader;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum TagNode {
  LowerBound(u32),
  Resolved(u32),
}

pub(crate) type TagPosition = console_core_point::Point<u32>;

fn root_level(size: Size<u32>) -> core::result::Result<u32, Never> {
  Ok(
    u32::BITS.wrapping_sub(
      size
        .width
        .max(size.height)
        .saturating_sub(1)
        .leading_zeros(),
    ),
  )
}

#[derive(Copy, Clone)]
pub(crate) struct TagTree {
  start: u64,
  size: Size<u32>,
}

impl TagTree {
  pub(crate) fn new(size: Size<u32>, nodes: &mut Vec<TagNode>) -> Result<Self> {
    let Ok(start) = fixed_index(nodes.len());
    let Ok(root_level) = root_level(size);
    let width = u64::from(size.width);
    let height = u64::from(size.height);
    let mut count = 0u64;

    for level in (0..=root_level).rev() {
      let scale = 1u64.wrapping_shl(level);
      let level_count = width.div_ceil(scale).wrapping_mul(height.div_ceil(scale));
      let checked_value = count
        .checked_add(level_count)
        .ok_or(ValidationError::ImageTooLarge)?;
      count = checked_value;
    }

    let length = start
      .checked_add(count)
      .ok_or(ValidationError::ImageTooLarge)?;
    crate::resize_decoder_buffer(
      nodes,
      length,
      crate::DecoderBuffer::TagTree,
      TagNode::LowerBound(0),
    )?;

    Ok(Self { start, size })
  }

  pub(crate) fn read(
    &self,
    position: TagPosition,
    reader: &mut BitReader<'_>,
    limit: u32,
    nodes: &mut [TagNode],
  ) -> core::result::Result<Option<u32>, Never> {
    match ::core::primitive::bool::then_some(
      position.x < self.size.width && position.y < self.size.height,
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    }

    let Ok(root_level) = root_level(self.size);
    let mut start = self.start;
    let mut parent = 0;

    for level in (0..=root_level).rev() {
      let scale = 1u64.wrapping_shl(level);
      let width = u64::from(self.size.width).div_ceil(scale);
      let height = u64::from(self.size.height).div_ceil(scale);
      let x = u64::from(position.x).wrapping_div(scale);
      let y = u64::from(position.y).wrapping_div(scale);
      let offset = start.wrapping_add(y.wrapping_mul(width)).wrapping_add(x);
      let Ok(native_index) = index(offset);
      let node = match nodes.get_mut(native_index) {
        Some(level_node) => level_node,
        None => return Ok(None),
      };
      let Ok(next) = read_node(node, reader, TagBounds { parent, limit });

      parent = match next {
        Some(value) => value,
        None => return Ok(None),
      };

      match ::core::cmp::Ord::cmp(
        &::core::convert::identity(parent),
        &::core::convert::identity(limit),
      ) {
        ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => break,
        ::core::cmp::Ordering::Less => start = start.wrapping_add(width.wrapping_mul(height)),
      }
    }

    Ok(Some(parent))
  }
}

struct TagBounds {
  parent: u32,
  limit: u32,
}

fn read_node(
  node: &mut TagNode,
  reader: &mut BitReader<'_>,
  TagBounds { parent, limit }: TagBounds,
) -> core::result::Result<Option<u32>, Never> {
  let lower_bound = match *node {
    TagNode::LowerBound(value) => parent.max(value),
    TagNode::Resolved(value) => return Ok(Some(value)),
  };

  for candidate in lower_bound..limit {
    let Ok(bit) = reader.read_bits_with_stuffing(1);

    match bit {
      Some(0) => {}
      Some(1) => {
        *node = TagNode::Resolved(candidate);
        return Ok(Some(candidate));
      }
      Some(_) | None => return Ok(None),
    }
  }

  let value = lower_bound.max(limit);
  *node = TagNode::LowerBound(value);
  Ok(Some(value))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rectangular_tree_reuses_parent_values_and_resumes_unresolved_leaves() -> Result<()> {
    let mut nodes = Vec::new();
    let tree = TagTree::new(
      Size {
        width: 3,
        height: 2,
      },
      &mut nodes,
    )?;
    let Ok(mut reader) = BitReader::new(&[0b1101_0110, 0b1000_0000]);

    let right_bottom = TagPosition { x: 2, y: 1 };
    let left_top = TagPosition { x: 1, y: 0 };
    let right_top = TagPosition { x: 2, y: 0 };
    let outside = TagPosition { x: 3, y: 0 };

    let Ok(first_limit) = tree.read(right_bottom, &mut reader, 1, &mut nodes);
    assert_eq!(first_limit, Some(1));
    let Ok(same_limit) = tree.read(right_bottom, &mut reader, 1, &mut nodes);
    assert_eq!(same_limit, Some(1));
    let Ok(outside_result) = tree.read(outside, &mut reader, 2, &mut nodes);
    assert_eq!(outside_result, None);
    let Ok(higher_limit) = tree.read(right_bottom, &mut reader, 2, &mut nodes);
    assert_eq!(higher_limit, Some(1));
    let Ok(left) = tree.read(left_top, &mut reader, 2, &mut nodes);
    assert_eq!(left, Some(1));
    let Ok(right) = tree.read(right_top, &mut reader, 2, &mut nodes);
    assert_eq!(right, Some(1));

    Ok(())
  }
}
