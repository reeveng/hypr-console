use console_core_number_conversion::indices;

use super::{InputDeletion, deletion};

#[kani::proof]
fn deletion_is_exact_and_never_splits_utf8() {
  let text = "a界é😀z";
  let caret = kani::any::<u8>();
  let before: u32 = kani::any();
  let after: u32 = kani::any();
  let result = deletion(text, u64::from(caret), InputDeletion { before, after });
  let offsets = [0, 1, 4, 6, 10, 11];
  let start = i64::from(caret).saturating_sub(i64::from(before));
  let end = i64::from(caret).saturating_add(i64::from(after));
  let valid =
    offsets.contains(&start) && offsets.contains(&i64::from(caret)) && offsets.contains(&end);
  assert_eq!(result.is_ok(), valid);

  match result {
    Ok(range) => {
      let Ok(native_range) = indices(range.clone());

      assert!(text.get(native_range).is_some());
      assert_eq!(i64::try_from(range.start), Ok(start));
      assert_eq!(i64::try_from(range.end), Ok(end));
    }
    Err(_) => {}
  }
}
