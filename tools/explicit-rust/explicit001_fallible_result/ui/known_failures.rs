// run-rustfix
// edition:2024

fn float(text: &str) -> Option<f64> {
  text.parse::<f64>().ok()
}

fn invalid_boolean(text: &str) -> bool {
  text.parse::<bool>().is_err()
}

fn pair(bytes: &[u8]) -> Option<&[u8; 2]> {
  <&[u8; 2]>::try_from(bytes).ok()
}

fn position(sorted: &[i32], target: i32, comparisons: &mut Vec<i32>) -> Option<usize> {
  sorted
    .binary_search_by(|value| {
      comparisons.push(*value);
      value.cmp(&target)
    })
    .ok()
    .map(|position| position + 1)
}

fn contains(sorted: &[i32], target: i32, observed: &mut bool) -> bool {
  sorted.binary_search(&target).is_ok() && {
    *observed = true;
    true
  }
}

fn main() {
  assert_eq!(float("1.25"), Some(1.25));
  assert_eq!(float("bad"), None);
  assert!(!invalid_boolean("true"));
  assert!(invalid_boolean("other"));
  let bytes = [21, 34];
  assert!(std::ptr::eq(pair(&bytes).unwrap(), &bytes));
  assert_eq!(pair(&bytes[..1]), None);
  let sorted = [1, 3, 3, 7];
  for target in [3, 5] {
    let mut expected_comparisons = Vec::new();
    let expected = match sorted.binary_search_by(|value| {
      expected_comparisons.push(*value);
      value.cmp(&target)
    }) {
      Ok(index) => Some(index + 1),
      Err(_insertion_position) => None,
    };
    let mut comparisons = Vec::new();
    assert_eq!(position(&sorted, target, &mut comparisons), expected);
    assert_eq!(comparisons, expected_comparisons);
  }
  let mut observed = false;
  assert!(!contains(&sorted, 5, &mut observed));
  assert!(!observed);
  assert!(contains(&sorted, 3, &mut observed));
  assert!(observed);
}
