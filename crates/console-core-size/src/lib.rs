//! A width and height measured in the same units. [`Size::swapped`] exchanges
//! them, and [`Size::map`] applies the same conversion to each dimension.
//!
//! The area of a `Size<u32>` holds the product of any two `u32`s. The area
//! of a `Size<f64>` keeps the sign of the product. Its longer and shorter
//! sides follow [`f64::max`] and [`f64::min`] for NaN handling.

#![no_std]

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Size<T> {
  pub width: T,
  pub height: T,
}

impl<T> Size<T> {
  pub fn swapped(self) -> Result<Self, Never> {
    Ok(Self {
      width: self.height,
      height: self.width,
    })
  }

  pub fn map<U>(self, each: impl Fn(T) -> U) -> Result<Size<U>, Never> {
    Ok(Size {
      width: each(self.width),
      height: each(self.height),
    })
  }
}

impl Size<u32> {
  pub fn area(self) -> Result<u64, Never> {
    Ok(u64::from(self.width).wrapping_mul(u64::from(self.height)))
  }
}

impl Size<f64> {
  pub const fn area(self) -> Result<f64, Never> {
    Ok(self.width * self.height)
  }

  pub const fn max_side(self) -> Result<f64, Never> {
    Ok(self.width.max(self.height))
  }

  pub const fn min_side(self) -> Result<f64, Never> {
    Ok(self.width.min(self.height))
  }
}

#[cfg(test)]
mod tests {
  use super::Size;

  #[test]
  fn an_area_holds_the_full_product_of_the_pixel_dimensions() {
    let size = Size {
      width: u32::MAX,
      height: u32::MAX,
    };

    assert_eq!(size.area(), Ok(18_446_744_065_119_617_025));
  }

  #[test]
  fn a_size_changes_what_it_is_counted_in_without_changing_what_it_is() {
    let room = Size {
      width: 1280u32,
      height: 800u32,
    };
    let Ok(mapped) = room.map(f64::from);

    assert_eq!(
      mapped,
      Size {
        width: 1280.0,
        height: 800.0,
      }
    );
  }
}
