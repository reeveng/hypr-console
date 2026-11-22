//! A table's version, in the two shapes OpenType writes it.
//!
//! Most tables write a major and a minor number as two `u16`s, which is [`MajorMinor`]. A few of
//! the oldest -- `maxp`, `post`, `vhea` -- write a 16.16 number whose minor part is a single
//! decimal digit held in the top four bits of the fraction, so version 2.5 is `0x0002_5000` rather
//! than two and a half; that is [`Version16Dot16`], and a minor number past nine is held at nine
//! when one is made, because a digit is all there is room for.
//!
//! A reader asks a version that is [`Compatible`] for its [`Compatibility`] with the one a field
//! arrived in: the same major number and at least the minor one is compatible, which is the
//! specification's rule for what a newer minor version may add.

use core::cmp::Ordering;

use console_core_never::Never;

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct Version16Dot16(u32);

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(C)]
pub struct MajorMinor {
  pub major: u16,
  pub minor: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Compatibility {
  Compatible,
  Incompatible,
}

pub trait Compatible<Rhs = Self>: Sized {
  fn compatibility(&self, other: Rhs) -> Result<Compatibility, Never>;
}

impl Version16Dot16 {
  pub const VERSION_0_5: Version16Dot16 = Version16Dot16(0x0000_5000);

  pub const VERSION_1_0: Version16Dot16 = Version16Dot16(0x0001_0000);

  pub const VERSION_1_1: Version16Dot16 = Version16Dot16(0x0001_1000);

  pub const VERSION_2_0: Version16Dot16 = Version16Dot16(0x0002_0000);

  pub const VERSION_2_5: Version16Dot16 = Version16Dot16(0x0002_5000);

  pub const VERSION_3_0: Version16Dot16 = Version16Dot16(0x0003_0000);

  pub const fn new(version: MajorMinor) -> Result<Self, Never> {
    let MajorMinor { major, minor } = version;
    let digit = match minor {
      0..=9 => minor,
      10.. => 9,
    };
    let [high, low] = major.to_be_bytes();
    let [_, digit_byte] = digit.to_be_bytes();
    let whole = u32::from_be_bytes([high, low, 0, 0]);
    let fraction = u32::from_be_bytes([0, 0, digit_byte, 0]).wrapping_shl(4);

    Ok(Self(whole | fraction))
  }

  pub const fn to_major_minor(self) -> Result<(u16, u16), Never> {
    let [first, second, third, fourth] = self.0.to_be_bytes();
    let major = u16::from_be_bytes([first, second]);
    let minor = u16::from_be_bytes([third, fourth]).wrapping_shr(12);

    Ok((major, minor))
  }

  #[inline]
  pub const fn to_be_bytes(self) -> Result<[u8; 4], Never> {
    Ok(self.0.to_be_bytes())
  }
}

crate::raw::newtype_scalar!(Version16Dot16, [u8; 4]);

impl MajorMinor {
  pub const VERSION_1_0: MajorMinor = MajorMinor { major: 1, minor: 0 };

  pub const VERSION_1_1: MajorMinor = MajorMinor { major: 1, minor: 1 };

  pub const VERSION_1_2: MajorMinor = MajorMinor { major: 1, minor: 2 };

  pub const VERSION_1_3: MajorMinor = MajorMinor { major: 1, minor: 3 };

  pub const VERSION_2_0: MajorMinor = MajorMinor { major: 2, minor: 0 };

  #[inline]
  pub const fn to_be_bytes(self) -> Result<[u8; 4], Never> {
    let [first, second] = self.major.to_be_bytes();
    let [third, fourth] = self.minor.to_be_bytes();

    Ok([first, second, third, fourth])
  }
}

impl crate::Scalar for MajorMinor {
  type Raw = [u8; 4];

  fn from_raw(raw: Self::Raw) -> Result<Self, Never> {
    let [first, second, third, fourth] = raw;
    let major = u16::from_be_bytes([first, second]);
    let minor = u16::from_be_bytes([third, fourth]);

    Ok(Self { major, minor })
  }

  fn to_raw(self) -> Result<Self::Raw, Never> {
    self.to_be_bytes()
  }
}

impl Compatible for Version16Dot16 {
  #[inline]
  fn compatibility(&self, other: Self) -> Result<Compatibility, Never> {
    let Ok((major, minor)) = self.to_major_minor();
    let Ok((other_major, other_minor)) = other.to_major_minor();

    MajorMinor { major, minor }.compatibility(MajorMinor {
      major: other_major,
      minor: other_minor,
    })
  }
}

impl Compatible<(u16, u16)> for Version16Dot16 {
  fn compatibility(&self, other: (u16, u16)) -> Result<Compatibility, Never> {
    let (major, minor) = other;
    let Ok(version) = Version16Dot16::new(MajorMinor { major, minor });

    self.compatibility(version)
  }
}

impl Compatible for MajorMinor {
  #[inline]
  fn compatibility(&self, other: Self) -> Result<Compatibility, Never> {
    Ok(
      match (self.major.cmp(&other.major), self.minor.cmp(&other.minor)) {
        (Ordering::Equal, Ordering::Equal | Ordering::Greater) => Compatibility::Compatible,
        (_, _) => Compatibility::Incompatible,
      },
    )
  }
}

impl Compatible<(u16, u16)> for MajorMinor {
  fn compatibility(&self, other: (u16, u16)) -> Result<Compatibility, Never> {
    let (major, minor) = other;

    self.compatibility(MajorMinor { major, minor })
  }
}

impl Compatible for u16 {
  #[inline]
  fn compatibility(&self, other: Self) -> Result<Compatibility, Never> {
    Ok(match self.cmp(&other) {
      Ordering::Equal | Ordering::Greater => Compatibility::Compatible,
      Ordering::Less => Compatibility::Incompatible,
    })
  }
}

impl core::fmt::Debug for Version16Dot16 {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    write!(f, "Version16Dot16({:08x})", self.0)
  }
}

impl core::fmt::Display for Version16Dot16 {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    let Ok((major, minor)) = self.to_major_minor();

    write!(f, "{major}.{minor}")
  }
}

impl core::fmt::Display for MajorMinor {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    let MajorMinor { major, minor } = self;

    write!(f, "{major}.{minor}")
  }
}
