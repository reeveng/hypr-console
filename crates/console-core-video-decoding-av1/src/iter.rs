#![forbid(unsafe_code)]

pub fn wrapping_iter<I, T>(
  iter: I,
  start: u64,
) -> core::result::Result<impl Iterator<Item = T>, console_core_never::Never>
where
  I: Iterator<Item = T> + Clone,
{
  let Ok(start_index) = console_core_number_conversion::wrapped_index(start);
  Ok(iter.clone().skip(start_index).chain(iter.take(start_index)))
}
