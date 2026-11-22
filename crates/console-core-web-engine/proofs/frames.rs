//! `cargo kani -p console-core-web-engine --lib --harness proofs`.

use super::*;

#[kani::proof]
fn invalid_dimensions_or_overflow_never_make_a_frame_layout() {
  let width: u32 = kani::any();
  let height: u32 = kani::any();
  let layout = FrameLayout::rgba(Size { width, height });
  let pixels = u64::from(width) * u64::from(height);
  let fits = pixels <= u64::MAX / 4;
  assert_eq!(layout.is_ok(), width > 0 && height > 0 && fits);
  if let Ok(accepted) = layout {
    assert_eq!(accepted.stride, u64::from(width) * 4);
    assert_eq!(accepted.bytes, pixels << 2);
  }
  kani::cover!(layout.is_ok());
  kani::cover!(!fits);
}

#[kani::proof]
fn frame_bytes_have_exactly_the_claimed_size_and_are_borrowed_in_place() {
  let width = u32::from(kani::any::<u8>());
  let height = u32::from(kani::any::<u8>());
  let length = usize::from(kani::any::<u8>() % 33);
  let backing: [u8; 32] = kani::any();
  let offered = &backing[..length];
  let expected = u64::from(width) * u64::from(height) * 4;
  let display =
    FrameLayout::rgba(Size { width, height }).and_then(|layout| layout.pixels(offered));
  assert_eq!(display.is_ok(), expected > 0 && expected == length as u64);
  if let Ok(pixels) = display {
    assert_eq!(pixels.len() * 4, length);
    assert!(core::ptr::eq(
      pixels.as_ptr().cast::<u8>(),
      offered.as_ptr()
    ));
  }
  kani::cover!(display.is_ok());
  kani::cover!(expected > 0 && expected != length as u64);
}
