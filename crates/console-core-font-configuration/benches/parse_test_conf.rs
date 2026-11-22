use console_core_font_configuration::{Bases, Error, FontConfig, parse_config_parts};
use std::time::{Duration, Instant};

const RUNS: u32 = 100;
const CONFIGURATION: &str = "test-conf/fonts.conf";

fn main() -> Result<(), Error> {
  let began = Instant::now();

  for _run in 0..RUNS {
    let text = std::fs::read_to_string(CONFIGURATION)?;
    let parts = parse_config_parts(&text)?;
    let _measured = std::hint::black_box(parts);
  }

  let parsing = began.elapsed();
  let began_2 = Instant::now();

  for _run in 0..RUNS {
    let mut config = FontConfig::default();
    config.merge_config(CONFIGURATION, &Bases::default())?;
    let _measured = std::hint::black_box(config);
  }

  let merging = began_2.elapsed();

  let each_parse = match parsing.checked_div(RUNS) {
    Some(each) => each,
    None => Duration::ZERO,
  };

  let each_merge = match merging.checked_div(RUNS) {
    Some(each) => each,
    None => Duration::ZERO,
  };

  println!("parse_full: {each_parse:?} a run, over {RUNS} runs");
  println!("merge_full: {each_merge:?} a run, over {RUNS} runs");

  Ok(())
}
