use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::units::{Time, TimeBase};
use std::time::Duration;

#[test]
fn a_time_base_converts_both_ways_without_losing_a_tick() -> Result<(), Error> {
  let tb1 = TimeBase::new(1, 320)?;

  let Ok(value_1) = tb1.calc_time(0);
  let Ok(value_2) = Time::new(0, 0.0);

  assert_eq!(value_1, value_2);
  let Ok(value_3) = tb1.calc_time(12_345);
  let Ok(value_4) = Time::new(38, 0.578125);

  assert_eq!(value_3, value_4);
  let Ok(value_5) = tb1.calc_time(0x0f_ffff_ffff_ffff);
  let Ok(value_6) = Time::new(14_073_748_835_532, 0.796875);

  assert_eq!(value_5, value_6);
  let Ok(value_7) = tb1.calc_time(0x10_0000_0000_0001);
  let Ok(value_8) = Time::new(14_073_748_835_532, 0.803125);

  assert_eq!(value_7, value_8);
  let Ok(value_9) = tb1.calc_time(u64::MAX);
  let Ok(value_10) = Time::new(57_646_075_230_342_348, 0.796875);

  assert_eq!(value_9, value_10);

  let tb2 = TimeBase::new(320, 1)?;

  let Ok(value_11) = tb2.calc_time(u64::MAX);
  let Ok(value_12) = Time::new(18_446_744_073_709_551_296, 0.0);

  assert_eq!(value_11, value_12);

  let Ok(value_13) = Time::new(0, 0.0);

  let Ok(value_22) = tb1.calc_timestamp(value_13);

  assert_eq!(value_22, 0);
  let Ok(value_14) = Time::new(38, 0.578125);

  let Ok(value_23) = tb1.calc_timestamp(value_14);

  assert_eq!(value_23, 12_345);
  let Ok(value_15) = Time::new(14_073_748_835_532, 0.796875);

  let Ok(value_24) = tb1.calc_timestamp(value_15);

  assert_eq!(value_24, 0x0f_ffff_ffff_ffff);
  let Ok(value_16) = Time::new(14_073_748_835_532, 0.803125);

  let Ok(value_25) = tb1.calc_timestamp(value_16);

  assert_eq!(value_25, 0x10_0000_0000_0001);
  let Ok(value_17) = Time::new(57_646_075_230_342_348, 0.796875);

  let Ok(value_26) = tb1.calc_timestamp(value_17);

  assert_eq!(value_26, u64::MAX);

  Ok(())
}

#[test]
fn a_time_base_with_a_zero_in_it_is_refused() {
  assert!(matches!(TimeBase::new(0, 1), Err(Error::DecodeError(_))));
  assert!(matches!(TimeBase::new(1, 0), Err(Error::DecodeError(_))));
}

#[test]
fn a_duration_and_a_time_are_the_same_instant() {
  let time = Time::from(Duration::from_secs_f64(38.578125));

  let Ok(value_18) = Time::new(38, 0.578125);

  assert_eq!(time, value_18);
  let Ok(value_19) = Time::new(38, 0.578125);

  assert_eq!(Duration::from(value_19), Duration::from_secs_f64(38.578125));
}

#[test]
fn a_negative_number_of_seconds_is_no_time() {
  let Ok(value_20) = Time::new(0, 0.0);

  assert_eq!(Time::from(-1.5_f64), value_20);
  let Ok(value_21) = Time::new(2, 0.25);

  assert_eq!(Time::from(2.25_f32), value_21);
}
