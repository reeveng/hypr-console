//! Delivery exploration refuses a partial result when its bound is too small.

use console_core_state_machine::{ExplorationError, interleavings};

#[test]
fn every_merge_keeps_each_producers_order() -> Result<(), ExplorationError> {
  let deliveries = interleavings(&[&[1, 2], &[3]], 3)?;

  assert_eq!(
    deliveries,
    vec![vec![1, 2, 3], vec![1, 3, 2], vec![3, 1, 2]]
  );

  Ok(())
}

#[test]
fn a_bound_too_small_is_an_error_instead_of_partial_coverage() {
  assert_eq!(
    interleavings(&[&[1, 2], &[3]], 2),
    Err(ExplorationError::Limit(2))
  );
  assert_eq!(interleavings(&[&[1]], 0), Err(ExplorationError::Limit(0)));
}

#[test]
fn no_arrivals_has_exactly_one_schedule() {
  assert_eq!(interleavings::<u32>(&[], 1), Ok(vec![Vec::new()]));
  assert_eq!(interleavings::<u32>(&[&[], &[]], 1), Ok(vec![Vec::new()]));
}

#[test]
fn equal_requests_from_different_producers_keep_their_own_schedules() {
  let streams: [&[i32]; 3] = [&[1, 2], &[1], &[]];

  assert_eq!(interleavings(&streams, 2), Err(ExplorationError::Limit(2)));
  assert_eq!(
    interleavings(&streams, 3),
    Ok(vec![vec![1, 2, 1], vec![1, 1, 2], vec![1, 1, 2]])
  );
}
