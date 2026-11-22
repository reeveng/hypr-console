// edition:2024

pub fn independent_widths(
  dst: &mut [u8],
  src: &[u8],
  destination_width: usize,
  source_width: usize,
) -> Result<(), ()> {
  (*(<[_]>::get_mut(&mut *dst, ..destination_width).ok_or(())?))
    .copy_from_slice(&*(<[_]>::get(&*src, ..source_width).ok_or(())?));
  Ok(())
}

pub fn unmatched_offsets(
  dst: &mut [u8],
  src: &[u8],
  a: usize,
  b: usize,
) -> Result<(), ()> {
  (*(<[_]>::get_mut(&mut *dst, a..b.wrapping_add(2)).ok_or(())?))
    .copy_from_slice(&*(<[_]>::get(&*src, ..2).ok_or(())?));
  Ok(())
}

fn main() {}
