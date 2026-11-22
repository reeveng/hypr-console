#[inline]
pub const fn rb_flag(x: u32) -> u32 {
  1 << x
}

#[inline]
pub fn rb_flag_unsafe(x: u32) -> u32 {
  match x < 32 {
    true => 1 << x,
    false => 0,
  }
}

#[inline]
pub fn rb_flag_range(x: u32, y: u32) -> u32 {
  u32::from((x < y)) + rb_flag(y + 1) - rb_flag(x)
}

#[inline]
pub const fn rb_flag64(x: u32) -> u64 {
  1 << x as u64
}

#[inline]
pub fn rb_flag64_unsafe(x: u32) -> u64 {
  match x < 64 {
    true => {
      1 << ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(x);
        migrated_value
      })
    }
    false => 0,
  }
}

pub const fn HB_CODEPOINT_ENCODE3(x: u32, y: u32, z: u32) -> u64 {
  ((x as u64) << 42) | ((y as u64) << 21) | (z as u64)
}
pub const fn HB_CODEPOINT_DECODE3_1(v: u64) -> u32 {
  (v >> 42) as u32
}
pub const fn HB_CODEPOINT_DECODE3_2(v: u64) -> u32 {
  ((v >> 21) & 0x001F_FFFF) as u32
}
pub const fn HB_CODEPOINT_DECODE3_3(v: u64) -> u32 {
  (v & 0x001F_FFFF) as u32
}

pub const fn HB_CODEPOINT_ENCODE3_11_7_14(x: u32, y: u32, z: u32) -> u32 {
  ((x & 0x07FF) << 21) | ((y & 0x007F) << 14) | (z & 0x3FFF)
}
pub const fn HB_CODEPOINT_DECODE3_11_7_14_1(v: u32) -> u32 {
  v >> 21
}
pub const fn HB_CODEPOINT_DECODE3_11_7_14_2(v: u32) -> u32 {
  ((v >> 14) & 0x007F) | 0x0300
}
pub const fn HB_CODEPOINT_DECODE3_11_7_14_3(v: u32) -> u32 {
  v & 0x3FFF
}
