use core::cmp::Ordering;

use console_core_font_types::{F2Dot14, F4Dot12, F6Dot10, F26Dot6, F48Dot16, Fixed};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "these bounds describe the representable numeric range of a fixed-point format without assigning a physical unit"
  )
)]
struct Range {
  least: f64,
  most: f64,
}

impl Range {
  fn of(ends: [i64; 2], one: i64) -> Result<Range, Never> {
    let [least, most] = ends;
    let Ok(least_2) = least.float();
    let Ok(most_2) = most.float();
    let Ok(one_2) = one.float();

    Ok(Range {
      least: (least_2 - 0.5) / one_2,
      most: (most_2 + 0.5) / one_2,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the test compares a numeric conversion input with the representable bounds of its format"
    )
  )]
  fn holds(&self, value: f64) -> Result<Option<f64>, Never> {
    Ok(
      match (
        self.least.partial_cmp(&value),
        value.partial_cmp(&self.most),
      ) {
        (Some(Ordering::Less), Some(Ordering::Less)) => Some(value),
        (_, _) => None,
      },
    )
  }
}

struct Written<'a> {
  raw: i64,
  text: &'a str,
  fract_bits: u32,
}

fn check_shortest(written: Written<'_>, read: &impl Fn(f64) -> Option<i64>) -> Result<(), Never> {
  let Written {
    raw,
    text,
    fract_bits,
  } = written;
  let places = match text.split_once('.') {
    Some((_, fraction)) => fitted::<_, u32>(fraction.len()),
    None => Ok(0),
  };
  let Ok(places_2) = places;

  assert_ne!(text, "-0");
  assert_eq!(
    text.parse::<f64>().map(read),
    Ok(Some(raw)),
    "{text} does not read back"
  );

  match places_2 {
    0 => {}
    _fewer_would_do => {
      let pow = 10i128.saturating_pow(places_2.saturating_sub(1));
      let floor = i128::from(raw)
        .saturating_mul(pow)
        .checked_div_euclid(1i128.wrapping_shl(fract_bits));

      for shorter in floor
        .into_iter()
        .flat_map(|below| [below, below.saturating_add(1)])
      {
        let Ok(shorter_2) = fitted::<_, i64>(shorter).and_then(|whole| whole.float());
        let Ok(pow_2) = fitted::<_, i64>(pow).and_then(|whole| whole.float());

        assert_ne!(
          read(shorter_2 / pow_2),
          Some(raw),
          "{text} could be {}",
          shorter_2 / pow_2
        );
      }
    }
  }

  Ok(())
}

fn sampled() -> Result<impl Iterator<Item = i32>, Never> {
  Ok(
    (i32::MIN..=i32::MIN.wrapping_add(2000))
      .chain(-200_000..=200_000)
      .chain((i32::MIN..=i32::MAX).step_by(65_521))
      .chain(i32::MAX.wrapping_sub(2000)..=i32::MAX),
  )
}

#[test]
fn every_two_dot_fourteen_prints_its_shortest_decimal() {
  let Ok(range) = Range::of([-32768, 32767], 0x4000);
  let read = |value: f64| {
    let Ok(held) = range.holds(value);

    held.map(
      |inside| match F2Dot14::from_f64(inside).and_then(F2Dot14::to_bits) {
        Ok(bits) => i64::from(bits),
      },
    )
  };

  for raw in i16::MIN..=i16::MAX {
    let Ok(text) = F2Dot14::from_bits(raw).map(|value| value.to_string());
    let Ok(()) = check_shortest(
      Written {
        raw: i64::from(raw),
        text: &text,
        fract_bits: 14,
      },
      &read,
    );
  }
}

#[test]
fn every_four_dot_twelve_prints_its_shortest_decimal() {
  let Ok(range) = Range::of([-32768, 32767], 0x1000);
  let read = |value: f64| {
    let Ok(held) = range.holds(value);

    held.map(
      |inside| match F4Dot12::from_f64(inside).and_then(F4Dot12::to_bits) {
        Ok(bits) => i64::from(bits),
      },
    )
  };

  for raw in i16::MIN..=i16::MAX {
    let Ok(text) = F4Dot12::from_bits(raw).map(|value| value.to_string());
    let Ok(()) = check_shortest(
      Written {
        raw: i64::from(raw),
        text: &text,
        fract_bits: 12,
      },
      &read,
    );
  }
}

#[test]
fn every_six_dot_ten_prints_its_shortest_decimal() {
  let Ok(range) = Range::of([-32768, 32767], 0x0400);
  let read = |value: f64| {
    let Ok(held) = range.holds(value);

    held.map(
      |inside| match F6Dot10::from_f64(inside).and_then(F6Dot10::to_bits) {
        Ok(bits) => i64::from(bits),
      },
    )
  };

  for raw in i16::MIN..=i16::MAX {
    let Ok(text) = F6Dot10::from_bits(raw).map(|value| value.to_string());
    let Ok(()) = check_shortest(
      Written {
        raw: i64::from(raw),
        text: &text,
        fract_bits: 10,
      },
      &read,
    );
  }
}

#[test]
fn a_sample_of_sixteen_dot_sixteen_prints_its_shortest_decimal() {
  let Ok(range) = Range::of([i64::from(i32::MIN), i64::from(i32::MAX)], 0x0001_0000);
  let read = |value: f64| {
    let Ok(held) = range.holds(value);

    held.map(
      |inside| match Fixed::from_f64(inside).and_then(Fixed::to_bits) {
        Ok(bits) => i64::from(bits),
      },
    )
  };
  let Ok(raws) = sampled();

  for raw in raws {
    let Ok(text) = Fixed::from_bits(raw).map(|value| value.to_string());
    let Ok(()) = check_shortest(
      Written {
        raw: i64::from(raw),
        text: &text,
        fract_bits: 16,
      },
      &read,
    );
  }
}

#[test]
fn a_sample_of_twenty_six_dot_six_prints_its_shortest_decimal() {
  let Ok(range) = Range::of([i64::from(i32::MIN), i64::from(i32::MAX)], 64);
  let read = |value: f64| {
    let Ok(held) = range.holds(value);

    held.map(
      |inside| match F26Dot6::from_f64(inside).and_then(F26Dot6::to_bits) {
        Ok(bits) => i64::from(bits),
      },
    )
  };
  let Ok(raws) = sampled();

  for raw in raws {
    let Ok(text) = F26Dot6::from_bits(raw).map(|value| value.to_string());
    let Ok(()) = check_shortest(
      Written {
        raw: i64::from(raw),
        text: &text,
        fract_bits: 6,
      },
      &read,
    );
  }
}

#[test]
fn the_ends_and_the_small_steps_print_as_fonttools_writes_them() {
  let two_dot: [(i16, &str); 9] = [
    (9830, "0.6"),
    (-10139, "-0.61884"),
    (0x4000, "1"),
    (-0x4000, "-1"),
    (0, "0"),
    (i16::MIN, "-2"),
    (i16::MAX, "1.99994"),
    (1, "0.00006"),
    (-1, "-0.00006"),
  ];
  let sixteen: [(i32, &str); 6] = [
    (0x03e8_0000, "1000"),
    (0x7fff_8000, "32767.5"),
    (i32::MAX, "32767.99998"),
    (i32::MIN, "-32768"),
    (1, "0.00002"),
    (-1, "-0.00002"),
  ];
  let twenty_six: [(i32, &str); 9] = [
    (1, "0.02"),
    (-3, "-0.05"),
    (32, "0.5"),
    (65, "1.02"),
    (-96, "-1.5"),
    (6416, "100.25"),
    (-448, "-7"),
    (i32::MIN, "-33554432"),
    (i32::MAX, "33554431.98"),
  ];

  for (raw, text) in two_dot {
    assert_eq!(
      F2Dot14::from_bits(raw).map(|value| value.to_string()),
      Ok(String::from(text))
    );
  }

  for (raw, text) in sixteen {
    assert_eq!(
      Fixed::from_bits(raw).map(|value| value.to_string()),
      Ok(String::from(text))
    );
  }

  for (raw, text) in twenty_six {
    assert_eq!(
      F26Dot6::from_bits(raw).map(|value| value.to_string()),
      Ok(String::from(text))
    );
  }
}

#[test]
fn a_float_taken_in_prints_as_the_decimal_it_was() {
  assert_eq!(
    F2Dot14::from_f32(0.5).map(|value| value.to_string()),
    Ok(String::from("0.5"))
  );
  assert_eq!(
    F2Dot14::from_f32(-0.25).map(|value| value.to_string()),
    Ok(String::from("-0.25"))
  );
  assert_eq!(
    F4Dot12::from_f32(0.6).map(|value| value.to_string()),
    Ok(String::from("0.6"))
  );
  assert_eq!(F4Dot12::MIN.to_string(), "-8");
  assert_eq!(F4Dot12::MAX.to_string(), "7.9998");
  assert_eq!(
    F6Dot10::from_f32(-0.6).map(|value| value.to_string()),
    Ok(String::from("-0.6"))
  );
  assert_eq!(F6Dot10::MIN.to_string(), "-32");
  assert_eq!(F6Dot10::MAX.to_string(), "31.999");
  assert_eq!(
    Fixed::from_f64(0.6).map(|value| value.to_string()),
    Ok(String::from("0.6"))
  );
  assert_eq!(
    Fixed::from_f64(-0.6).map(|value| value.to_string()),
    Ok(String::from("-0.6"))
  );
  assert_eq!(
    Fixed::from_f64(-1.5).map(|value| value.to_string()),
    Ok(String::from("-1.5"))
  );
  assert_eq!(
    F48Dot16::from_f64(0.6).map(|value| value.to_string()),
    Ok(String::from("0.6"))
  );
  assert_eq!(
    F48Dot16::from_i64(0x0100_0000_0000).map(|value| value.to_string()),
    Ok(String::from("1099511627776"))
  );
  assert_eq!(
    F48Dot16::from_f64(-1_099_511_627_776.5).map(|value| value.to_string()),
    Ok(String::from("-1099511627776.5"))
  );
  assert_eq!(F48Dot16::MAX.to_string(), "140737488355327.99998");
  assert_eq!(F48Dot16::MIN.to_string(), "-140737488355328");
  assert_eq!(
    F48Dot16::from_bits(i64::MIN.wrapping_add(1)).map(|value| value.to_string()),
    Ok(String::from("-140737488355327.99998"))
  );
}

#[test]
fn the_formatter_flags_pad_and_sign_the_shortest_decimal() {
  let Ok(value) = F2Dot14::from_bits(9830);
  let Ok(minus) = F2Dot14::from_f32(-1.5);

  assert_eq!(format!("{value:.2}"), "0.60");
  assert_eq!(format!("{value:.2?}"), "0.60");
  assert_eq!(format!("{value:>8}"), "     0.6");
  assert_eq!(format!("{value:8}"), "     0.6");
  assert_eq!(format!("{value:<8}|"), "0.6     |");
  assert_eq!(format!("{value:*^9}"), "***0.6***");
  assert_eq!(format!("{value:08}"), "000000.6");
  assert_eq!(format!("{value:+}"), "+0.6");
  assert_eq!(format!("{value:?}"), "0.6");

  for (fixed, float) in [(minus, -1.5f64), (F2Dot14::ONE, 1.0)] {
    assert_eq!(format!("{fixed}"), format!("{float}"));
    assert_eq!(format!("{fixed:.3}"), format!("{float:.3}"));
    assert_eq!(format!("{fixed:>8}"), format!("{float:>8}"));
    assert_eq!(format!("{fixed:<8}"), format!("{float:<8}"));
    assert_eq!(format!("{fixed:08}"), format!("{float:08}"));
    assert_eq!(format!("{fixed:+09}"), format!("{float:+09}"));
  }
}
