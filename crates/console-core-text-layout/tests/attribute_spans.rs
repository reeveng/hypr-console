use console_core_text_layout::{Attrs, AttrsList, Family, FamilyOwned};

fn attrs(metadata: u32) -> Result<Attrs<'static>, console_core_never::Never> {
  {
    let Ok(mut attrs) = Attrs::new();
    attrs.metadata = metadata;
    Ok(attrs)
  }
}

fn assert_spans(list: &AttrsList, expected: &[u32]) -> Result<(), console_core_never::Never> {
  {
    for (position, metadata) in expected.iter().enumerate() {
      let Ok(index) = console_core_number_conversion::fitted::<_, u32>(position);
      let Ok(found) = list.get_span(index);
      assert_eq!(found.metadata, *metadata, "index {index}");
    }

    let Ok(spans) = list.spans();

    for (range, _) in &spans {
      assert!(range.start < range.end);
    }

    for [(left, left_attrs), (right, right_attrs)] in spans.array_windows::<2>() {
      assert!(left.end <= right.start);
      let separated = left.end != right.start;
      let distinct_attrs = left_attrs != right_attrs;

      assert!(separated || distinct_attrs);
    }

    Ok(())
  }
}

#[test]
fn overlapping_spans_match_per_character_attributes() -> Result<(), Box<dyn std::error::Error>> {
  let ranges: Vec<_> = (0..=6)
    .flat_map(|start| (start..=6).map(move |end| start..end))
    .collect();

  for first in &ranges {
    for second in &ranges {
      for second_attrs in [1, 2] {
        check_overlap([(first.clone(), 1), (second.clone(), second_attrs)])?;
      }
    }
  }

  Ok(())
}

fn check_overlap(
  ranges: [(core::ops::Range<u32>, u32); 2],
) -> Result<(), Box<dyn std::error::Error>> {
  let Ok(defaults) = attrs(0);
  let Ok(mut list) = AttrsList::new(&defaults);
  let mut expected = [0; 8];

  for (range, metadata) in ranges {
    let Ok(indices) = console_core_number_conversion::indices(range.clone());
    let slice = expected
      .get_mut(indices)
      .ok_or("Attribute fixture index is outside its array")?;
    slice.fill(metadata);
    let Ok(span_attrs) = attrs(metadata);
    let Ok(()) = list.add_span(range, &span_attrs);
    let Ok(()) = assert_spans(&list, &expected);
  }

  for split in 0..=8 {
    let mut left = list.clone();
    let Ok(right) = left.split_off(split);
    let Ok(position) = console_core_number_conversion::index(split);
    let (before, after) = expected
      .split_at_checked(position)
      .ok_or("Attribute fixture split exceeds its array")?;
    let Ok(()) = assert_spans(&left, before);
    let Ok(()) = assert_spans(&right, after);
    let Ok(left_end) = left.get_span(split);
    assert_eq!(left_end.metadata, 0);
  }

  Ok(())
}

#[test]
#[allow(
  clippy::reversed_empty_ranges,
  reason = "invalid attribute ranges must leave existing spans unchanged"
)]
fn spans_preserve_defaults_and_handle_the_last_index() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(mut list) = AttrsList::new(&match attrs(7) {
    Ok(value) => value,
  });
  let Ok(()) = list.add_span(
    u32::MAX.saturating_sub(3)..u32::MAX,
    &match attrs(9) {
      Ok(value) => value,
    },
  );
  let Ok(()) = list.add_span(
    4..2,
    &match attrs(1) {
      Ok(value) => value,
    },
  );
  let Ok(before) = list.get_span(u32::MAX.saturating_sub(4));
  let Ok(inside) = list.get_span(u32::MAX.saturating_sub(1));
  let Ok(after) = list.get_span(u32::MAX);
  assert_eq!(
    (before.metadata, inside.metadata, after.metadata),
    (7, 9, 7)
  );
  let Ok(right) = list.split_off(u32::MAX.saturating_sub(2));

  let Ok(()) = assert_spans(&right, &[9, 9, 7]);

  let Ok(()) = list.clear_spans();

  let Ok(()) = assert_spans(&list, &[7; 8]);

  Ok(())
}

#[test]
fn family_names_preserve_content_when_cloned() -> Result<(), Box<dyn std::error::Error>> {
  for name in [
    "Inter",
    "A font family name longer than the old inline string storage",
  ] {
    let Ok(family) = FamilyOwned::new(Family::Name(name));
    let cloned = family.clone();
    let Ok(borrowed) = cloned.as_family();
    assert_eq!(borrowed, Family::Name(name));
    let Ok(independent) = FamilyOwned::new(Family::Name(name));
    assert_eq!(family, independent);
    assert_eq!(family, cloned);
  }

  Ok(())
}
