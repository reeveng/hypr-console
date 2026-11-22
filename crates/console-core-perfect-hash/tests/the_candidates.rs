use console_core_perfect_hash::Map;

#[test]
fn candidate_lookup_requires_the_owners_membership_check() {
  static MAP: Map<[u8], u8> = {
    let Ok(map) = Map::new(7, &[(0, 0)], &[17]);

    map
  };
  assert_eq!(MAP.candidate(b"known"), Ok(Some(&17)));
  assert_eq!(MAP.candidate(b"unknown"), Ok(Some(&17)));
}

#[test]
fn empty_layouts_have_no_candidate() {
  static NO_BUCKETS: Map<u32, u8> = {
    let Ok(map) = Map::new(0, &[], &[17]);

    map
  };
  static NO_VALUES: Map<u32, u8> = {
    let Ok(map) = Map::new(0, &[(0, 0)], &[]);

    map
  };
  assert_eq!(NO_BUCKETS.candidate(&42), Ok(None));
  assert_eq!(NO_VALUES.candidate(&42), Ok(None));
}
