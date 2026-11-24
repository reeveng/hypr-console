//! Prove the address mapping used by portable CPU partitions.

use super::super::WordAt;
use super::*;

#[kani::proof]
fn an_item_can_read_shared_inputs_and_write_only_its_owned_slot() {
  let prefix: [u32; 2] = kani::any();
  let suffix: [u32; 1] = kani::any();
  let initial: u32 = kani::any();
  let written: u32 = kani::any();
  let position: u32 = kani::any();
  let mut slot = initial;
  // Words 2 and 3 belong to this pass. This item owns word 3.
  let Ok(mut item) = Invocation::partitioned(
    [1, 0, 0],
    &[0; 8],
    PartitionWords {
      before: &prefix,
      after: &suffix,
      position: 3,
      end: 4,
      slot: &mut slot,
    },
  );
  let Ok(read) = item.word(position);
  let expected = match position {
    0 => prefix[0],
    1 => prefix[1],
    3 => initial,
    4 => suffix[0],
    _ => 0,
  };
  assert_eq!(read, expected);
  assert_eq!(
    item.partition_access(),
    Ok(match position {
      2 => PartitionAccess::Violated,
      _ => PartitionAccess::Allowed,
    })
  );
  let Ok(()) = item.set_word(WordAt(position), written);
  assert_eq!(
    item.partition_access(),
    Ok(match position {
      3 => PartitionAccess::Allowed,
      _ => PartitionAccess::Violated,
    })
  );
  assert_eq!(slot, if position == 3 { written } else { initial });
}

#[kani::proof]
#[kani::unwind(5)]
fn a_partition_maps_linear_items_to_unique_active_grid_points() {
  let [width, height, depth]: [u32; 3] = kani::any();
  kani::assume((1..=16).contains(&width));
  kani::assume((1..=16).contains(&height));
  kani::assume((1..=4).contains(&depth));
  let origin: u32 = kani::any();
  kani::assume(origin <= 256);
  let size = width * height * depth;
  let layout = Partition {
    output_at: origin,
    dimensions: [width, height, depth],
  }
  .layout(
    origin + size,
    [width.div_ceil(8), height.div_ceil(8), depth],
    [8, 8, 1],
  )
  .unwrap();
  let first: u32 = kani::any();
  let second: u32 = kani::any();
  kani::assume(first < size && second < size);
  let Ok([x, y, z]) = layout.coordinate(first);
  assert!(x < width && y < height && z < depth);
  assert_eq!(x + y * width + z * width * height, first);
  let Ok([other_x, other_y, other_z]) = layout.coordinate(second);
  assert_eq!(
    x == other_x && y == other_y && z == other_z,
    first == second
  );
  assert!(layout.output.contains(&origin.saturating_add(first)));
}
