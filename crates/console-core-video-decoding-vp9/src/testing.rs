use console_core_never::Never;

pub(crate) struct Xorshift(pub u32);

impl Xorshift {
  pub(crate) fn next(&mut self) -> Result<u32, Never> {
    self.0 ^= self.0.wrapping_shl(13);
    self.0 ^= self.0.wrapping_shr(17);
    self.0 ^= self.0.wrapping_shl(5);
    Ok(self.0)
  }

  pub(crate) fn byte(&mut self) -> Result<u8, Never> {
    let Ok(value) = self.next();
    console_core_number_conversion::wrapped(value)
  }
}
