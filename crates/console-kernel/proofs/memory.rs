//! Run with `cargo kani -p console-kernel --lib --harness memory::proofs`.

use super::{CONVENTIONAL, Frames, MemoryError, MemoryMap, PAGE, Region};

#[kani::proof]
fn a_region_end_is_exact_or_reports_overflow() {
  let start: u64 = kani::any();
  let pages: u64 = kani::any();
  let mathematical_end = u128::from(start) + 4096 * u128::from(pages);

  match (Region { start, pages }).end() {
    Ok(end) => assert_eq!(u128::from(end), mathematical_end),
    Err(MemoryError::Overflow) => assert!(mathematical_end > u128::from(u64::MAX)),
    Err(_) => panic!("unexpected region error"),
  }
}

fn descriptor<const STRIDE: usize>(kind: u32, start: u64, pages: u64) -> [u8; STRIDE] {
  let mut bytes = [0; STRIDE];
  bytes[0..4].copy_from_slice(&kind.to_le_bytes());
  bytes[8..16].copy_from_slice(&start.to_le_bytes());
  bytes[24..32].copy_from_slice(&pages.to_le_bytes());

  bytes
}

#[kani::proof]
#[kani::unwind(4)]
fn memory_map_accepts_exactly_well_formed_disjoint_regions() {
  const STRIDE: usize = 48;
  let fields: [(u32, u64, u64); 3] = kani::any();
  let descriptors = fields.map(|(kind, start, pages)| descriptor::<STRIDE>(kind, start, pages));
  let ranges = fields.map(|(_, start, pages)| {
    let start = u128::from(start);
    (start, start + 4096 * u128::from(pages))
  });

  let valid = |(start, end): (u128, u128)| {
    start < end && start.is_multiple_of(4096) && end <= u128::from(u64::MAX)
  };
  let separate =
    |(start, end), (other_start, other_end)| end <= other_start || other_end <= start;
  let [first, middle, last] = ranges;
  let well_formed = valid(first) && valid(middle) && valid(last);
  let disjoint = separate(first, middle) && separate(first, last) && separate(middle, last);

  let result = MemoryMap::new(descriptors.as_flattened(), 48);
  assert_eq!(result.is_ok(), well_formed && disjoint);
  if well_formed && !disjoint {
    assert!(matches!(result, Err(MemoryError::OverlappingRegions)));
  }

  let (first_start, first_end) = first;
  let (middle_start, _) = middle;
  let [(first_kind, _, _), _, (last_kind, _, _)] = fields;
  let different_ownership = (first_kind == CONVENTIONAL) != (last_kind == CONVENTIONAL);
  kani::cover!(result.is_ok() && first_end == middle_start);
  kani::cover!(result.is_ok() && first_start > middle_start);
  // The nonconsecutive pair overlaps even though both adjacent pairs are disjoint.
  kani::cover!(
    well_formed
      && different_ownership
      && first_start > middle_start
      && separate(first, middle)
      && separate(middle, last)
      && !separate(first, last)
  );
}

#[kani::proof]
#[kani::unwind(4)]
fn reservations_advance_without_using_the_null_page_or_reserved_memory() {
  let first_pages = u64::from(kani::any::<u8>()) + 1;
  let reserved_pages = u64::from(kani::any::<u8>()) + 1;
  let last_pages = u64::from(kani::any::<u8>()) + 1;
  let first_end = first_pages * 4096;
  let reserved_start = 256 * 4096;
  let last_start = 512 * 4096;
  let last_end = last_start + last_pages * 4096;
  let descriptors = [
    descriptor::<40>(CONVENTIONAL, 0, first_pages),
    descriptor::<40>(2, reserved_start, reserved_pages),
    descriptor::<40>(CONVENTIONAL, last_start, last_pages),
  ];
  let Ok(map) = MemoryMap::new(descriptors.as_flattened(), 40) else {
    panic!("a disjoint map of nonempty, aligned regions was rejected");
  };
  let Ok(mut frames) = Frames::new(map);
  assert_eq!((frames.next, frames.region_end), (PAGE, PAGE));

  // Check one reservation from the initial cursor or from either cached region.
  let offset = (u64::from(kani::any::<u8>()) + 1) * PAGE;
  let (cursor, region_end) = match kani::any::<u8>() % 3 {
    0 => (frames.next, frames.region_end),
    1 => {
      let next = PAGE + offset;
      kani::assume(next <= first_end);
      (next, first_end)
    }
    _ => {
      let next = last_start + offset;
      kani::assume(next <= last_end);
      (next, last_end)
    }
  };
  frames.next = cursor;
  frames.region_end = region_end;

  let requested = u64::from(kani::any::<u8>());
  let size = requested * PAGE;
  let first_fits = cursor + size <= first_end;
  let last_fits = cursor.max(last_start) + size <= last_end;
  match frames.take(requested) {
    Ok(start) => {
      let (expected_start, expected_end) = if first_fits {
        (cursor, first_end)
      } else {
        assert!(last_fits);
        (cursor.max(last_start), last_end)
      };

      assert!(requested > 0);
      assert!(start >= PAGE && start.is_multiple_of(PAGE));
      assert_eq!(start, expected_start);
      assert_eq!(
        (frames.next, frames.region_end),
        (start + size, expected_end)
      );
    }
    Err(MemoryError::EmptyReservation) => {
      assert_eq!(requested, 0);
      assert_eq!((frames.next, frames.region_end), (cursor, region_end));
    }
    Err(MemoryError::OutOfFrames) => {
      assert!(requested > 0 && !first_fits && !last_fits);
      assert_eq!((frames.next, frames.region_end), (cursor, region_end));
    }
    Err(_) => panic!("unexpected reservation error"),
  }
}

#[kani::proof]
#[kani::unwind(4)]
fn misaligned_conventional_regions_cannot_enter_the_allocator() {
  let gap: u16 = kani::any();
  kani::assume(gap > 0 && u64::from(gap) < PAGE);
  let bad_start = PAGE + u64::from(gap);
  let bytes = descriptor::<40>(CONVENTIONAL, bad_start, 2);

  assert!(matches!(
    MemoryMap::new(&bytes, 40),
    Err(MemoryError::UnalignedRegion(start)) if start == bad_start
  ));
}
