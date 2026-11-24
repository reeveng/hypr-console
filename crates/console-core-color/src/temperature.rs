//! RGB channel gains for a warm or cool display white point.
//!
//! The black-body approximation uses the fit by Tanner Helland.
//! https://tannerhelland.com/2012/09/18/convert-temperature-rgb-algorithm-code.html

use console_core_never::Never;

pub const MIN_KELVIN: u32 = 1_000;
pub const MAX_KELVIN: u32 = 20_000;

pub fn white_balance(kelvin: u32) -> Result<[f64; 3], Never> {
  let hundreds = f64::from(kelvin.clamp(MIN_KELVIN, MAX_KELVIN)) / 100.0;
  let [red, green, blue] = match (hundreds <= 66.0).then_some(()) {
    Some(()) => [
      255.0,
      99.4708025861 * hundreds.ln() - 161.1195681661,
      match (hundreds <= 19.0).then_some(()) {
        Some(()) => 0.0,
        None => 138.5177312231 * (hundreds - 10.0).ln() - 305.0447927307,
      },
    ],
    None => [
      329.698727446 * (hundreds - 60.0).powf(-0.1332047592),
      288.1221695283 * (hundreds - 60.0).powf(-0.0755148492),
      255.0,
    ],
  };

  Ok([red, green, blue].map(|channel| channel.clamp(0.0, 255.0) / 255.0))
}
