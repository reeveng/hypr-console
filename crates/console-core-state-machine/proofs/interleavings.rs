//! `cargo kani -p console-core-state-machine --lib --harness interleavings`.

use super::*;
use std::cmp::Ordering;

#[kani::proof]
#[kani::unwind(8)]
fn two_streams_visit_every_fifo_merge_once_and_honor_the_limit() {
  let expected = [
    [0, 0, 1, 1],
    [0, 1, 0, 1],
    [0, 1, 1, 0],
    [1, 0, 0, 1],
    [1, 0, 1, 0],
    [1, 1, 0, 0],
  ];
  let mut order = expected[0];
  let limit: u8 = kani::any();
  kani::assume(limit <= 7);
  let maximum = u32::from(limit);
  let mut visited = 0;

  let result = explore(&mut order, maximum, &mut visited, |count, order| {
    assert_eq!(order.len(), 4);
    for (position, &producer) in order.iter().enumerate() {
      assert_eq!(producer, expected[*count][position]);
    }
    *count += 1;
    Ok(())
  });

  assert_eq!(visited, usize::from(limit.min(6)));
  assert_eq!(
    result,
    match limit.cmp(&6) {
      Ordering::Less => Err(ExplorationError::Limit(maximum)),
      Ordering::Equal | Ordering::Greater => Ok(()),
    }
  );
  kani::cover!(limit == 6);
  kani::cover!(limit == 5);
}

fn encoded(order: [usize; 4]) -> (u16, u16) {
  let mut lexicographic = 0;
  let mut counts = 0;

  for producer in order {
    assert!(producer < 3);
    lexicographic = (lexicographic << 2) | (producer as u16);
    counts += 1_u16 << (producer * 3);
  }

  (lexicographic, counts)
}

#[kani::proof]
#[kani::unwind(7)]
fn a_successor_is_the_least_greater_order_of_the_same_producers() {
  let original: [u8; 4] = kani::any();
  let candidate: [u8; 4] = kani::any();
  kani::assume(
    original
      .iter()
      .chain(&candidate)
      .all(|producer| *producer < 3),
  );
  let original = original.map(usize::from);
  let (before, counts) = encoded(original);
  let (compared, candidate_counts) = encoded(candidate.map(usize::from));
  kani::assume(counts == candidate_counts);
  let mut order = original;

  let Ok(progress) = next_order(&mut order);
  let (next, next_counts) = encoded(order);

  assert_eq!(next_counts, counts);
  match progress {
    Permutation::Advanced => assert!(before < next),
    Permutation::Exhausted => assert_eq!(next, before),
  }

  match before.cmp(&compared) {
    Ordering::Less => assert!(matches!(progress, Permutation::Advanced) && next <= compared),
    Ordering::Equal | Ordering::Greater => {}
  }

  kani::cover!(matches!(progress, Permutation::Advanced) && original.contains(&2));
  kani::cover!(matches!(progress, Permutation::Exhausted));
}
