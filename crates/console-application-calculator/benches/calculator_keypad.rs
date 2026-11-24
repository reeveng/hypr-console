use std::hint::black_box;

use console_panel::card::Card;
use console_panel::page::{ButtonPress, Face, Nowhere, Row};
use criterion::Criterion;

#[derive(Debug)]
enum FixtureError {
  MissingPage,
  MissingKey(String),
}

impl std::fmt::Display for FixtureError {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MissingPage => out.write_str("calculator opens without a page"),
      Self::MissingKey(label) => write!(out, "calculator key {label} is missing"),
    }
  }
}

impl std::error::Error for FixtureError {}

fn key(rows: &[Row], label: &str) -> Result<ButtonPress, FixtureError> {
  rows
    .iter()
    .filter_map(|row| row.buttons.as_ref())
    .flat_map(|buttons| &buttons.presses)
    .find(|button| matches!(button.face, Face::Written(text) if text == label))
    .cloned()
    .ok_or_else(|| FixtureError::MissingKey(label.to_owned()))
}

fn calculator_keypad(criterion: &mut Criterion) -> Result<(), FixtureError> {
  let Ok(Card { build, done, .. }) = console_application_calculator::card(&[]);
  let page = build().pop().ok_or(FixtureError::MissingPage)?;
  let Ok(rows) = page.rows.read();

  for digit in ["1", "2", "3"] {
    let button = key(&rows, digit)?;
    let _ = (button.does)(&Nowhere);
  }

  let sign = key(&rows, "\u{b1}")?;
  let _ = (sign.does)(&Nowhere);
  let Ok(updated) = page.rows.read();
  let display = updated
    .first()
    .and_then(|row| row.headline.as_ref())
    .map(|headline| headline.big.as_str());
  assert_eq!(display, Some("-123"));

  criterion.bench_function("calculator/keypad_sign_and_rows", |bencher| {
    bencher.iter(|| {
      let _ = (black_box(&sign).does)(&Nowhere);
      let Ok(updated_2) = page.rows.read();
      updated_2
    });
  });

  let Ok(()) = done();

  Ok(())
}

fn main() -> Result<(), FixtureError> {
  let mut criterion = Criterion::default().configure_from_args();
  calculator_keypad(&mut criterion)?;
  criterion.final_summary();

  Ok(())
}
