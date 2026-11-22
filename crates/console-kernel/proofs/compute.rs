//! `cargo kani -p console-kernel --lib --harness compute::proofs`.
//!
//! These checks model the integer instructions and dispatch layout in
//! compute/checksums.comp. The hardware test runs the compiled shader.

use super::*;

#[kani::proof]
#[kani::unwind(6)]
fn a_shader_word_preserves_the_cpu_checksum_recurrence() {
  let word: u32 = kani::any();
  let mut state: u32 = kani::any();
  let Ok(expected) = fnv1a::update32(state, &word.to_le_bytes());

  for shift in [0, 8, 16, 24] {
    state = (state ^ ((word >> shift) & 255)).wrapping_mul(0x0100_0193);
  }

  assert_eq!(state, expected);
}

#[kani::proof]
fn dispatched_items_read_only_their_block_and_have_distinct_output_slots() {
  const WORDS: u32 = (BLOCK_BYTES / 4) as u32;
  const WORKGROUP: u32 = 64;

  let count: u32 = kani::any();
  kani::assume(count > 0 && count <= MAX_BLOCKS as u32);
  let groups = count.div_ceil(WORKGROUP);
  assert!(groups * WORKGROUP >= count);
  assert!((groups - 1) * WORKGROUP < count);

  let item: u32 = kani::any();
  let other: u32 = kani::any();
  let word: u32 = kani::any();
  kani::assume(item < count && other < count && word < WORDS);

  let input_end = count * WORDS;
  let input = item * WORDS + word;
  let output = input_end + item;
  assert!(input < input_end);
  assert!(output >= input_end && output < input_end + count);
  assert_eq!(output == input_end + other, item == other);
  assert_eq!(input / WORDS, item);
}

#[kani::proof]
#[kani::unwind(19)]
fn the_backend_plan_reaches_one_publication_after_all_required_joins() {
  let blocks = usize::from(kani::any::<u16>()) + usize::from(kani::any::<bool>());
  let mut plan = ReductionPlan::new(blocks).unwrap();
  let mut remaining = blocks;
  let mut published = false;
  let mut joins = 0;

  for step in plan.by_ref() {
    assert!(!published);
    match step {
      ReductionStep::Join { inputs } => {
        assert_eq!(inputs, remaining);
        assert!(inputs > 1);
        remaining = (inputs + 1) / 2;
        joins += 1;
      }
      ReductionStep::Publish => {
        assert_eq!(remaining, usize::from(blocks != 0));
        published = true;
      }
    }
  }

  assert!(published && joins <= 16);
  assert_eq!(plan.next(), None);
  kani::cover!(joins == 16);
  kani::cover!(blocks == 0);
}

#[kani::proof]
fn join_lanes_stay_in_disjoint_gpu_ranges_for_either_scratch_rotation() {
  const WORDS: u32 = (BLOCK_BYTES / 4) as u32;
  const WORKGROUP: u32 = 64;
  let count: u32 = kani::any();
  kani::assume((2..=MAX_BLOCKS as u32).contains(&count));
  let first = count * WORDS;
  let second = first + count;
  let scratch = (count + 1) / 2;
  let end = second + scratch;
  let (source, capacity, destination, destination_capacity) = if kani::any() {
    (first, count, second, scratch)
  } else {
    (second, scratch, first, count)
  };

  let inputs: u32 = kani::any();
  kani::assume((2..=capacity).contains(&inputs));
  let parents = (inputs + 1) / 2;
  let groups = parents.div_ceil(WORKGROUP);
  assert!(groups * WORKGROUP >= parents && (groups - 1) * WORKGROUP < parents);

  let parent: u32 = kani::any();
  let other: u32 = kani::any();
  let child: u32 = kani::any();
  kani::assume(parent < parents && other < parents && child < inputs);
  let left = 2 * parent;
  let right = left + 1;
  let output = destination + parent;

  assert!(left < inputs);
  assert!(source + left < source + capacity);
  assert!(right < inputs || right == inputs && inputs % 2 == 1);
  assert!(child / 2 < parents);
  assert!(output < destination + destination_capacity && output < end);
  assert!(output < source || output >= source + capacity);
  assert_eq!(output == destination + other, parent == other);
  if parent < other {
    assert!(parent < 2 * other);
  }
}

#[kani::proof]
fn one_join_preserves_the_wrapping_sum_including_an_unpaired_child() {
  let [prefix, left, right]: [u32; 3] = kani::any();
  let has_right: bool = kani::any();
  let inputs = [left, right];
  let parent = joined(&inputs[..1 + usize::from(has_right)], 0);
  let serial = u64::from(prefix) + u64::from(left) + u64::from(if has_right { right } else { 0 });

  assert_eq!(prefix.wrapping_add(parent), serial as u32);
}

#[kani::proof]
#[kani::unwind(7)]
fn the_cpu_tree_matches_serial_for_empty_and_small_batches() {
  let mut words: [u32; 5] = kani::any();
  let count = usize::from(kani::any::<u8>() % 6);
  let expected = words[..count].iter().copied().fold(0, u32::wrapping_add);
  let plan = ReductionPlan::new(count).unwrap();

  assert_eq!(reduce(&mut words[..count], plan), expected);
}

#[kani::proof]
#[kani::unwind(7)]
fn a_level_with_an_odd_tail_can_finish_in_any_lane_order() {
  let children: [u32; 5] = kani::any();
  let order: [u8; 3] = kani::any();
  let [a, b, c] = order;
  kani::assume(a < 3 && b < 3 && c < 3 && a != b && a != c && b != c);

  let mut level = [0; 3];
  for lane in order.map(usize::from) {
    level[lane] = joined(&children, lane);
  }

  let expected = children.into_iter().fold(0, u32::wrapping_add);
  let plan = ReductionPlan::new(level.len()).unwrap();
  assert_eq!(reduce(&mut level, plan), expected);
}
