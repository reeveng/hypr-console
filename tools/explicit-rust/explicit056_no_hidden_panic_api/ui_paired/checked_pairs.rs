// run-rustfix
// edition:2024

#[derive(Debug, Eq, PartialEq)]
enum Bounds {
  Destination,
  Source,
}

fn copy_fixed(
  dst: &mut [u8],
  src: &[u8],
  offset: usize,
) -> Result<(), Bounds> {
  (*{
    let checked = <[_]>::get_mut(&mut *dst, ..2).ok_or(Bounds::Destination)?;
    checked
  })
  .copy_from_slice(&*(<[_]>::get(&*src, offset..offset.wrapping_add(2))
    .ok_or(Bounds::Source)?));
  Ok(())
}

fn copy_width(
  dst: &mut [u8],
  src: &[u8],
  offset: usize,
  width: usize,
) -> Result<(), Bounds> {
  (*(<[_]>::get_mut(&mut *dst, ..width).ok_or(Bounds::Destination)?))
    .copy_from_slice(&*(<[_]>::get(&*src, offset..offset.wrapping_add(width))
    .ok_or(Bounds::Source)?));
  Ok(())
}

fn copy_ending(
  dst: &mut [u8],
  src: &[u8],
  end: usize,
  width: usize,
) -> Result<(), Bounds> {
  (*(<[_]>::get_mut(&mut *dst, ..width).ok_or(Bounds::Destination)?))
    .copy_from_slice(&*(<[_]>::get(&*src, end.wrapping_sub(width)..end)
    .ok_or(Bounds::Source)?));
  Ok(())
}

fn copy_after_offset(
  dst: &mut [u8],
  src: &[u8],
  offset: usize,
  width: usize,
) -> Result<(), Bounds> {
  (*(<[_]>::get_mut(&mut *dst, ..width).ok_or(Bounds::Destination)?))
    .copy_from_slice(&*(<[_]>::get(
      &*src,
      offset.wrapping_add(1)..offset.wrapping_add(1).wrapping_add(width),
    )
    .ok_or(Bounds::Source)?));
  Ok(())
}

fn main() {
  let mut dst = [0; 4];
  let src = [10, 20, 30, 40];
  assert_eq!(copy_fixed(&mut dst, &src, 2), Ok(()));
  assert_eq!(dst, [30, 40, 0, 0]);
  assert_eq!(copy_width(&mut dst, &src, 1, 3), Ok(()));
  assert_eq!(dst, [20, 30, 40, 0]);
  assert_eq!(copy_ending(&mut dst, &src, 3, 2), Ok(()));
  assert_eq!(copy_after_offset(&mut dst, &src, 0, 1), Ok(()));
  assert_eq!(dst, [20, 30, 40, 0]);

  assert_eq!(copy_width(&mut dst, &src, 4, 0), Ok(()));
  assert_eq!(copy_ending(&mut dst, &src, 3, 0), Ok(()));
  assert_eq!(copy_ending(&mut dst, &src, 1, 2), Err(Bounds::Source));
  assert_eq!(copy_width(&mut dst, &src, usize::MAX, 1), Err(Bounds::Source));
  assert_eq!(copy_fixed(&mut dst, &src, usize::MAX), Err(Bounds::Source));
  assert_eq!(copy_fixed(&mut dst, &src[..1], 0), Err(Bounds::Source));
  assert_eq!(copy_fixed(&mut dst[..1], &src[..1], 0), Err(Bounds::Destination));
  assert_eq!(dst, [20, 30, 40, 0]);
}
