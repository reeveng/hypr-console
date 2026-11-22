//! Run with `cargo kani -p console-kernel --lib --harness user_buffers`.

use super::*;

#[kani::proof]
fn accepted_ranges_are_nonempty_canonical_and_authorized_on_every_page() {
  let base: u64 = kani::any();
  let start: u64 = kani::any();
  let length: u64 = kani::any();
  let entries: [u64; 2] = kani::any();
  let write: bool = kani::any();
  let access = if write {
    FrameAccess::Write
  } else {
    FrameAccess::Read
  };

  let base_wide = u128::from(base);
  let start_wide = u128::from(start);
  let end = start_wide + u128::from(length);
  let middle = base_wide + 4096;
  let limit = middle + 4096;
  let valid_window = base > 0 && base & 0xfff == 0 && limit <= 1 << 47;
  let valid_range = length > 0 && start_wide >= base_wide && end <= limit;
  let mask = if write { 0b111 } else { 0b101 };
  let first_ok = start_wide >= middle || entries[0] & mask == mask;
  let second_ok = end <= middle || entries[1] & mask == mask;

  match UserWindow::new(base, &entries) {
    Err(error) => {
      assert_eq!(error, BufferError::Range);
      assert!(!valid_window);
    }
    Ok(window) => {
      assert!(valid_window);
      match window.authorize(UserRange { start, length }, access) {
        Err(BufferError::Range) => assert!(!valid_range),
        Err(BufferError::Permission) => {
          assert!(valid_range);
          assert!(!first_ok || !second_ok);
        }
        Ok(buffer) => {
          assert!(valid_range && first_ok && second_ok);
          assert_eq!(buffer.start, start);
          assert_eq!(u128::from(buffer.end), end);
          kani::cover!(start_wide < middle && end > middle);
        }
        _ => unreachable!(),
      }
    }
  }
}

#[kani::proof]
#[kani::unwind(6)]
fn a_copy_fault_on_either_page_preserves_the_entire_destination() {
  let start = 0x1ffe;
  let table = [0b111, 0b111];
  let window = UserWindow::new(0x1000, &table).unwrap();
  let buffer = window
    .authorize(UserRange { start, length: 4 }, FrameAccess::Read)
    .unwrap();
  let before: [u8; 4] = kani::any();
  let input: [u8; 4] = kani::any();
  let fault: usize = kani::any();
  let mut destination = before;
  let mut loaded = 0;

  let result = buffer.copy_in(
    |address| {
      assert_eq!(address, start + loaded as u64);
      let byte = input[loaded];
      if loaded == fault {
        return Err(BufferError::Fault);
      }
      loaded += 1;
      Ok(byte)
    },
    &mut destination,
  );

  if fault < 4 {
    assert_eq!(result, Err(BufferError::Fault));
    assert_eq!(destination, before);
    assert_eq!(loaded, fault);
  } else {
    assert_eq!(result, Ok(()));
    assert_eq!(destination, input);
    assert_eq!(loaded, 4);
  }

  let mut shorter: [u8; 3] = kani::any();
  let old = shorter;
  assert_eq!(
    buffer.copy_in(|_| unreachable!(), &mut shorter),
    Err(BufferError::Length)
  );
  assert_eq!(shorter, old);
}
