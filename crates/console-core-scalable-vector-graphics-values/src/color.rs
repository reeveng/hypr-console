use crate::{Error, Stream, colors};

#[cfg(not(feature = "std"))]
use kurbo::common::FloatFuncs;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "every RGBA byte combination is a valid color and FromStr checks the CSS color syntax"
  )
)]
pub struct Color {
  pub red: u8,
  pub green: u8,
  pub blue: u8,
  pub alpha: u8,
}

impl Color {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public constructor follows CSS red, green, blue and alpha channel order"
    )
  )]
  pub fn new_rgb(
    red: u8,
    green: u8,
    blue: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      red,
      green,
      blue,
      alpha: 255,
    })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public constructor follows CSS red, green, blue and alpha channel order"
    )
  )]
  pub fn new_rgba(
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      red,
      green,
      blue,
      alpha,
    })
  }

  #[inline]
  pub fn black() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(color) = Self::new_rgb(0, 0, 0);

    Ok(color)
  }

  #[inline]
  pub fn white() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(color) = Self::new_rgb(255, 255, 255);

    Ok(color)
  }

  #[inline]
  pub fn gray() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(color) = Self::new_rgb(128, 128, 128);

    Ok(color)
  }

  #[inline]
  pub fn red() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(color) = Self::new_rgb(255, 0, 0);

    Ok(color)
  }

  #[inline]
  pub fn green() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(color) = Self::new_rgb(0, 128, 0);

    Ok(color)
  }

  #[inline]
  pub fn blue() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(color) = Self::new_rgb(0, 0, 255);

    Ok(color)
  }
}

impl core::str::FromStr for Color {
  type Err = Error;

  fn from_str(text: &str) -> Result<Self, Error> {
    let mut s = Stream::from(text);
    let color = s.parse_color()?;

    let Ok(()) = s.skip_spaces();
    let Ok(at_end) = s.at_end();

    match ::core::primitive::bool::then_some(!matches!(at_end, crate::End::Reached), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(calc_char_pos) = s.calc_char_pos();
        return Err(Error::UnexpectedData(calc_char_pos));
      }
      ::core::option::Option::None => {}
    }

    Ok(color)
  }
}

impl Stream<'_> {
  pub fn try_parse_color(&mut self) -> Result<Option<Color>, console_core_never::Never> {
    let mut trial = *self;

    Ok(match trial.parse_color() {
      Ok(color) => {
        *self = trial;
        Some(color)
      }
      Err(_color_not_present_here) => None,
    })
  }

  pub fn parse_color(&mut self) -> Result<Color, Error> {
    let Ok(()) = self.skip_spaces();
    let first = self.curr_byte()?;

    match first {
      b'#' => {
        let Ok(()) = self.advance(1);
        let Ok(digits) = self.consume_bytes(|_, byte| byte.is_ascii_hexdigit());

        hex_color(digits.as_bytes())
      }
      _ => {
        let Ok(ident) = self.consume_ascii_ident();
        let name = ident.to_ascii_lowercase();

        match name.as_str() {
          "rgb" | "rgba" => self.parse_rgb(),
          "hsl" | "hsla" => self.parse_hsl(),
          _ => {
            let Ok(named) = colors::from_str(&name);

            named.ok_or(Error::InvalidValue)
          }
        }
      }
    }
  }

  fn parse_rgb(&mut self) -> Result<Color, Error> {
    self.consume_byte(b'(')?;
    let red = self.parse_number()?;
    let Ok(percent) = self.take_prefix(&[(b"%", ())]);
    let Ok(()) = self.skip_spaces();
    let Ok(()) = self.parse_list_separator();
    let rgb = match percent {
      Some(()) => {
        let green = self.parse_list_number_or_percent()?;
        let blue = self.parse_list_number_or_percent()?;

        [(red / 100.0) * 255.0, green * 255.0, blue * 255.0]
      }
      None => {
        let green = self.parse_list_number()?;
        let blue = self.parse_list_number()?;

        [red, green, blue]
      }
    };
    let [red_2, green, blue] = rgb.map(|amount| {
      let Ok(channel) = console_core_number_conversion::toward_zero_u8(amount.round());

      channel
    });
    let alpha = self.parse_alpha()?;

    Ok(Color {
      red: red_2,
      green,
      blue,
      alpha,
    })
  }

  fn parse_hsl(&mut self) -> Result<Color, Error> {
    use console_core_number_conversion::nearest_f32_from_f64;

    self.consume_byte(b'(')?;
    let degrees = self.parse_list_number()?;
    let rotation = ((degrees % 360.0) + 360.0) % 360.0;
    let saturation = self.parse_list_number_or_percent()?;
    let lightness = self.parse_list_number_or_percent()?;
    let Ok(hue) = nearest_f32_from_f64(rotation);
    let Ok(chroma) = nearest_f32_from_f64(saturation.clamp(0.0, 1.0));
    let Ok(level) = nearest_f32_from_f64(lightness.clamp(0.0, 1.0));
    let Ok(mut color) = hsl_to_rgb(Hsl {
      hue: Hue(hue / 60.0),
      saturation: Ratio(chroma),
      lightness: Ratio(level),
    });
    let checked_value = self.parse_alpha()?;
    color.alpha = checked_value;

    Ok(color)
  }

  fn parse_alpha(&mut self) -> Result<u8, Error> {
    let Ok(()) = self.skip_spaces();
    let Ok(close) = self.is_curr_byte_eq(b')');
    let alpha = match close {
      crate::IsCurrByteEqAnswer::Yes => 255,
      crate::IsCurrByteEqAnswer::No => {
        let opacity = self.parse_list_number()?;
        let Ok(alpha) = console_core_number_conversion::toward_zero_u8((opacity * 255.0).round());

        alpha
      }
    };
    let Ok(()) = self.skip_spaces();
    self.consume_byte(b')')?;

    Ok(alpha)
  }
}

fn hex_color(digits: &[u8]) -> Result<Color, Error> {
  let [red, green, blue, alpha] = match digits {
    [r, g, b] => [
      {
        let Ok(digit) = short_hex(*r);
        digit
      },
      {
        let Ok(digit) = short_hex(*g);
        digit
      },
      {
        let Ok(digit) = short_hex(*b);
        digit
      },
      255,
    ],
    [r, g, b, a] => [
      {
        let Ok(digit) = short_hex(*r);
        digit
      },
      {
        let Ok(digit) = short_hex(*g);
        digit
      },
      {
        let Ok(digit) = short_hex(*b);
        digit
      },
      {
        let Ok(digit) = short_hex(*a);
        digit
      },
    ],
    [a, b, c, d, e, f] => [
      {
        let Ok(digit) = hex_pair([*a, *b]);
        digit
      },
      {
        let Ok(digit) = hex_pair([*c, *d]);
        digit
      },
      {
        let Ok(digit) = hex_pair([*e, *f]);
        digit
      },
      255,
    ],
    [a, b, c, d, e, f, g, h] => [
      {
        let Ok(digit) = hex_pair([*a, *b]);
        digit
      },
      {
        let Ok(digit) = hex_pair([*c, *d]);
        digit
      },
      {
        let Ok(digit) = hex_pair([*e, *f]);
        digit
      },
      {
        let Ok(digit) = hex_pair([*g, *h]);
        digit
      },
    ],
    _ => return Err(Error::InvalidValue),
  };

  Ok(Color {
    red,
    green,
    blue,
    alpha,
  })
}

#[inline]
fn from_hex(c: u8) -> core::result::Result<u8, console_core_never::Never> {
  Ok(match console_core_bytes::hex::decode_digit(c) {
    Ok(digit) => digit,
    Err(_not_hex) => 0,
  })
}

#[inline]
fn short_hex(c: u8) -> Result<u8, console_core_never::Never> {
  let Ok(h) = from_hex(c);

  Ok(u8::wrapping_shl(h, 4_u32) | h)
}

#[inline]
fn hex_pair([high, low]: [u8; 2]) -> Result<u8, console_core_never::Never> {
  let Ok(h) = from_hex(high);
  let Ok(l) = from_hex(low);

  Ok(u8::wrapping_shl(h, 4_u32) | l)
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the coordinate is degrees divided by sixty, giving six sectors per turn"
  )
)]
struct Hue(f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner dimensionless ratio is measured from zero to one"
  )
)]
struct Ratio(f32);

struct Hsl {
  hue: Hue,
  saturation: Ratio,
  lightness: Ratio,
}

struct ChannelRange {
  low: Ratio,
  high: Ratio,
}

fn hsl_to_rgb(hsl: Hsl) -> Result<Color, console_core_never::Never> {
  let Hsl {
    hue: Hue(hue),
    saturation: Ratio(saturation),
    lightness: Ratio(lightness),
  } = hsl;
  let high = match (lightness <= 0.5).then_some(()) {
    Some(()) => lightness * (saturation + 1.0),
    None => lightness + saturation - (lightness * saturation),
  };
  let range = ChannelRange {
    low: Ratio(lightness * 2.0 - high),
    high: Ratio(high),
  };
  let [red, green, blue] = [hue + 2.0, hue, hue - 2.0].map(|angle| {
    let Ok(level) = hue_to_rgb(&range, Hue(angle));
    let Ok(byte) =
      console_core_number_conversion::toward_zero_u8(f64::from((level * 255.0).round()));

    byte
  });

  Color::new_rgb(red, green, blue)
}

fn hue_to_rgb(range: &ChannelRange, Hue(mut hue): Hue) -> Result<f32, console_core_never::Never> {
  let (t1, t2) = (range.low.0, range.high.0);

  match ::core::primitive::bool::then_some(hue < 0.0, ()) {
    ::core::option::Option::Some(()) => {
      hue += 6.0;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(hue >= 6.0, ()) {
    ::core::option::Option::Some(()) => {
      hue -= 6.0;
    }
    ::core::option::Option::None => {}
  }

  Ok(match ::core::primitive::bool::then_some(hue < 1.0, ()) {
    ::core::option::Option::Some(()) => (t2 - t1) * hue + t1,
    ::core::option::Option::None => match ::core::primitive::bool::then_some(hue < 3.0, ()) {
      ::core::option::Option::Some(()) => t2,
      ::core::option::Option::None => match ::core::primitive::bool::then_some(hue < 4.0, ()) {
        ::core::option::Option::Some(()) => (t2 - t1) * (4.0 - hue) + t1,
        ::core::option::Option::None => t1,
      },
    },
  })
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use core::str::FromStr;
    use crate::Color;

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test {
        ($name:ident, $text:expr, $color:expr) => {
            #[test]
            fn $name() {
                assert_eq!(Color::from_str($text), Ok($color));
            }
        };
    }

    test!(
        rrggbb,
        "#ff0000",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rrggbb_upper,
        "#FF0000",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rgb_hex,
        "#f00",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rrggbbaa,
        "#ff0000ff",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rrggbbaa_upper,
        "#FF0000FF",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rgba_hex,
        "#f00f",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rrggbb_spaced,
        "  #ff0000  ",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        rgb_numeric,
        "rgb(254, 203, 231)",
        Color { red: 254, green: 203, blue: 231, alpha: 255 }
    );

    test!(
        rgb_numeric_spaced,
        " rgb( 77 , 77 , 77 ) ",
        Color { red: 77, green: 77, blue: 77, alpha: 255 }
    );

    test!(
        rgb_percentage,
        "rgb(50%, 50%, 50%)",
        Color { red: 128, green: 128, blue: 128, alpha: 255 }
    );

    test!(
        rgb_percentage_overflow,
        "rgb(140%, -10%, 130%)",
        Color { red: 255, green: 0, blue: 255, alpha: 255 }
    );

    test!(
        rgb_percentage_float,
        "rgb(33.333%,46.666%,93.333%)",
        Color { red: 85, green: 119, blue: 238, alpha: 255 }
    );

    test!(
        rgb_numeric_upper_case,
        "RGB(254, 203, 231)",
        Color { red: 254, green: 203, blue: 231, alpha: 255 }
    );

    test!(
        rgb_numeric_mixed_case,
        "RgB(254, 203, 231)",
        Color { red: 254, green: 203, blue: 231, alpha: 255 }
    );

    test!(
        rgb_numeric_red_float,
        "rgb(3.141592653, 110, 201)",
        Color { red: 3, green: 110, blue: 201, alpha: 255 }
    );

    test!(
        rgb_numeric_green_float,
        "rgb(254, 150.829521289232389, 210)",
        Color { red: 254, green: 151, blue: 210, alpha: 255 }
    );

    test!(
        rgb_numeric_blue_float,
        "rgb(96, 255, 0.2)",
        Color { red: 96, green: 255, blue: 0, alpha: 255 }
    );

    test!(
        rgb_numeric_all_float,
        "rgb(0.0, 129.82, 231.092)",
        Color { red: 0, green: 130, blue: 231, alpha: 255 }
    );

    test!(
        rgb_numeric_all_float_with_alpha,
        "rgb(0.0, 129.82, 231.092, 0.5)",
        Color { red: 0, green: 130, blue: 231, alpha: 128 }
    );

    test!(
        rgb_numeric_all_float_overflow,
        "rgb(290.2, 255.9, 300.0)",
        Color { red: 255, green: 255, blue: 255, alpha: 255 }
    );

    test!(
        name_red,
        "red",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        name_red_spaced,
        " red ",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        name_red_upper_case,
        "RED",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        name_red_mixed_case,
        "ReD",
        Color { red: 255, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        name_cornflowerblue,
        "cornflowerblue",
        Color { red: 100, green: 149, blue: 237, alpha: 255 }
    );

    test!(
        transparent,
        "transparent",
        Color { red: 0, green: 0, blue: 0, alpha: 0 }
    );

    test!(
        rgba_half,
        "rgba(10, 20, 30, 0.5)",
        Color { red: 10, green: 20, blue: 30, alpha: 128 }
    );

    test!(
        rgba_numeric_red_float,
        "rgba(3.141592653, 110, 201, 1.0)",
        Color { red: 3, green: 110, blue: 201, alpha: 255 }
    );

    test!(
        rgba_numeric_all_float,
        "rgba(0.0, 129.82, 231.092, 1.5)",
        Color { red: 0, green: 130, blue: 231, alpha: 255 }
    );

    test!(
        rgba_negative,
        "rgba(10, 20, 30, -2)",
        Color { red: 10, green: 20, blue: 30, alpha: 0 }
    );

    test!(
        rgba_large_alpha,
        "rgba(10, 20, 30, 2)",
        Color { red: 10, green: 20, blue: 30, alpha: 255 }
    );

    test!(
        rgb_with_alpha,
        "rgb(10, 20, 30, 0.5)",
        Color { red: 10, green: 20, blue: 30, alpha: 128 }
    );

    test!(
        hsl_green,
        "hsl(120, 100%, 75%)",
        Color { red: 128, green: 255, blue: 128, alpha: 255 }
    );

    test!(
        hsl_yellow,
        "hsl(60, 100%, 50%)",
        Color { red: 255, green: 255, blue: 0, alpha: 255 }
    );

    test!(
        hsl_hue_360,
        "hsl(360, 100%, 100%)",
        Color { red: 255, green: 255, blue: 255, alpha: 255 }
    );

    test!(
        hsl_out_of_bounds,
        "hsl(800, 150%, -50%)",
        Color { red: 0, green: 0, blue: 0, alpha: 255 }
    );

    test!(
        hsla_green,
        "hsla(120, 100%, 75%, 0.5)",
        Color { red: 128, green: 255, blue: 128, alpha: 128 }
    );

    test!(
        hsl_with_alpha,
        "hsl(120, 100%, 75%, 0.5)",
        Color { red: 128, green: 255, blue: 128, alpha: 128 }
    );

    test!(
        hsl_to_rgb_red_round_up,
        "hsl(230, 57%, 54%)",
        Color { red: 71, green: 93, blue: 205, alpha: 255 }
    );

    test!(
        hsl_with_hue_float,
        "hsl(120.152, 100%, 75%)",
        Color { red: 128, green: 255, blue: 128, alpha: 255 }
    );

    test!(
        hsla_with_hue_float,
        "hsla(120.152, 100%, 75%, 0.5)",
        Color { red: 128, green: 255, blue: 128, alpha: 128 }
    );

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test_err {
        ($name:ident, $text:expr, $err:expr) => {
            #[test]
            fn $name() {
                assert_eq!(Color::from_str($text).map_err(|fault| fault.to_string()), Err($err.to_string()));
            }
        };
    }

    test_err!(
        not_a_color_1,
        "text",
        "invalid value"
    );

    test_err!(
        icc_color_not_supported_1,
        "#CD853F icc-color(acmecmyk, 0.11, 0.48, 0.83, 0.00)",
        "unexpected data at position 9"
    );

    test_err!(
        icc_color_not_supported_2,
        "red icc-color(acmecmyk, 0.11, 0.48, 0.83, 0.00)",
        "unexpected data at position 5"
    );

    test_err!(
        invalid_input_1,
        "rgb(-0\x0d",
        "unexpected end of stream"
    );

    test_err!(
        invalid_input_2,
        "#9ߞpx! ;",
        "invalid value"
    );

    test_err!(
        rgba_with_percent_alpha,
        "rgba(10, 20, 30, 5%)",
        "expected ')' not '%' at position 19"
    );

    test_err!(
        rgb_mixed_units,
        "rgb(140%, -10mm, 130pt)",
        "invalid number at position 14"
    );
}
