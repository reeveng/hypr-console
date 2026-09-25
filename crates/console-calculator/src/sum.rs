//! What a calculator's keys come to, with no screen in it.
//!
//! The keys are the iPhone's, and so is what they mean: an operator waits for
//! the number after it, a second operator finishes the first sum and waits
//! again, equals finishes whatever is waiting, and an operator pressed twice in
//! a row is a change of mind rather than a sum with nothing on one side. That
//! is immediate execution, left to right -- 2 + 3 × 4 is 20 -- because that is
//! what every pocket calculator anyone has held does, and a person checking a
//! bill is not writing an expression.
//!
//! **A number is an `f64`, said to twelve places.** 0.1 + 0.2 is
//! 0.30000000000000004 underneath, and nobody wants to read that, so what is
//! shown is rounded to the digits a double can be trusted with. A decimal type
//! would answer that exactly and was not written, because the display has
//! room for fewer digits than the error ever reaches.
//!
//! Dividing by nothing says Error rather than infinity, and the next key
//! starts again from it.

use console_core_never::Never;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Operator {
    #[words(says = "+")]
    Add,
    #[words(says = "\u{2212}")]
    Subtract,
    #[words(says = "\u{d7}")]
    Multiply,
    #[words(says = "\u{f7}")]
    Divide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Digit(u8),
    Point,
    Operator(Operator),
    Equals,
    Clear,
    Sign,
    Percent,
    Backspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typing {
    Fresh,
    Along,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sum {
    entry: String,
    waiting: Option<(f64, Operator)>,
    typing: Typing,
    failed: Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failed {
    Yes,
    No,
}

const LONGEST: u32 = 15;

const ERROR: &str = "Error";

impl Default for Sum {
    fn default() -> Self {
        Sum { entry: "0".to_string(), waiting: None, typing: Typing::Fresh, failed: Failed::No }
    }
}

impl Sum {
    pub fn shown(&self) -> Result<String, Never> {
        Ok(match self.failed {
            Failed::Yes => ERROR.to_string(),
            Failed::No => self.entry.clone(),
        })
    }

    pub fn above(&self) -> Result<String, Never> {
        Ok(match self.waiting {
            Some((held, operator)) => {
                let Ok(held) = said(held);
                let Ok(operator) = operator.says();

                format!("{held} {operator}")
            }
            None => String::new(),
        })
    }

    pub fn lit(&self) -> Result<Option<Operator>, Never> {
        Ok(match (self.waiting, self.typing) {
            (Some((_, operator)), Typing::Fresh) => Some(operator),
            (Some(_), Typing::Along) | (None, _) => None,
        })
    }

    pub fn pressed(&self, key: Key) -> Result<Sum, Never> {
        let from = match self.failed {
            Failed::Yes => Sum::default(),
            Failed::No => self.clone(),
        };

        match key {
            Key::Digit(digit) => from.typed(&digit.min(9).to_string()),
            Key::Point => from.pointed(),
            Key::Operator(operator) => from.waiting_on(operator),
            Key::Equals => from.finished(),
            Key::Clear => Ok(Sum::default()),
            Key::Sign => from.signed(),
            Key::Percent => from.percent(),
            Key::Backspace => from.rubbed_out(),
        }
    }

    fn typed(self, digit: &str) -> Result<Sum, Never> {
        let Ok(long) = console_core_number_conversion::fitted::<_, u32>(self.entry.chars().count());
        let entry = match (self.typing, self.entry.as_str()) {
            (Typing::Fresh, _) | (Typing::Along, "0") => digit.to_string(),
            (Typing::Along, "-0") => format!("-{digit}"),
            (Typing::Along, was) => match long >= LONGEST {
                true => was.to_string(),
                false => format!("{was}{digit}"),
            },
        };

        Ok(Sum { entry, typing: Typing::Along, ..self })
    }

    fn pointed(self) -> Result<Sum, Never> {
        let entry = match (self.typing, self.entry.contains('.')) {
            (Typing::Fresh, _) => "0.".to_string(),
            (Typing::Along, true) => self.entry.clone(),
            (Typing::Along, false) => format!("{}.", self.entry),
        };

        Ok(Sum { entry, typing: Typing::Along, ..self })
    }

    fn value(&self) -> Result<f64, Never> {
        Ok(match self.entry.parse::<f64>() {
            Ok(value) => value,
            Err(_not_a_number) => 0.0,
        })
    }

    fn waiting_on(self, operator: Operator) -> Result<Sum, Never> {
        match (self.waiting, self.typing) {
            (Some((held, _)), Typing::Fresh) => Ok(Sum { waiting: Some((held, operator)), ..self }),
            (Some(_), Typing::Along) => {
                let Ok(done) = self.finished();

                match done.failed {
                    Failed::Yes => Ok(done),
                    Failed::No => {
                        let Ok(value) = done.value();

                        Ok(Sum { waiting: Some((value, operator)), typing: Typing::Fresh, ..done })
                    }
                }
            }
            (None, _) => {
                let Ok(value) = self.value();

                Ok(Sum { waiting: Some((value, operator)), typing: Typing::Fresh, ..self })
            }
        }
    }

    fn finished(self) -> Result<Sum, Never> {
        let (held, operator) = match self.waiting {
            Some(waiting) => waiting,
            None => return Ok(Sum { typing: Typing::Fresh, ..self }),
        };
        let Ok(value) = self.value();
        let Ok(answer) = worked((held, operator), value);

        Ok(match answer {
            Some(answer) => {
                let Ok(entry) = said(answer);

                Sum { entry, waiting: None, typing: Typing::Fresh, failed: Failed::No }
            }
            None => Sum { waiting: None, typing: Typing::Fresh, failed: Failed::Yes, ..Sum::default() },
        })
    }

    fn signed(self) -> Result<Sum, Never> {
        let entry = match self.entry.strip_prefix('-') {
            Some(positive) => positive.to_string(),
            None => format!("-{}", self.entry),
        };

        Ok(Sum { entry, ..self })
    }

    fn rubbed_out(self) -> Result<Sum, Never> {
        let mut shorter = self.entry.clone();

        let _ = shorter.pop();

        let entry = match (self.typing, shorter.as_str()) {
            (Typing::Fresh, _) => self.entry.clone(),
            (Typing::Along, "" | "-") => "0".to_string(),
            (Typing::Along, _) => shorter,
        };

        Ok(Sum { entry, ..self })
    }

    fn percent(self) -> Result<Sum, Never> {
        let Ok(value) = self.value();
        let Ok(entry) = said(value / 100.0);

        Ok(Sum { entry, typing: Typing::Fresh, ..self })
    }
}

fn worked((held, operator): (f64, Operator), value: f64) -> Result<Option<f64>, Never> {
    let answer = match operator {
        Operator::Add => held + value,
        Operator::Subtract => held - value,
        Operator::Multiply => held * value,
        Operator::Divide => match value == 0.0 {
            true => return Ok(None),
            false => held / value,
        },
    };

    Ok(match answer.is_finite() {
        true => Some(answer),
        false => None,
    })
}

const TRUSTED: i32 = 12;

pub fn said(value: f64) -> Result<String, Never> {
    match value == 0.0 {
        true => return Ok("0".to_string()),
        false => {},
    }

    let Ok(whole_digits) = console_core_number_conversion::toward_zero_i32(value.abs().log10().floor());
    let places = TRUSTED.saturating_sub(whole_digits.saturating_add(1));

    match places {
        0.. => {
            let Ok(places) = console_core_number_conversion::index(places.unsigned_abs());
            let written = format!("{value:.places$}");

            Ok(match written.contains('.') {
                true => written.trim_end_matches('0').trim_end_matches('.').to_string(),
                false => written,
            })
        }
        _ => Ok(format!("{value:e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed(keys: &[Key]) -> Sum {
        keys.iter().fold(Sum::default(), |sum, key| {
            let Ok(next) = sum.pressed(*key);

            next
        })
    }

    fn shows(keys: &[Key]) -> String {
        let Ok(shown) = keyed(keys).shown();

        shown
    }

    use Key::{Backspace, Clear, Digit, Equals, Percent, Point, Sign};

    const ADD: Key = Key::Operator(Operator::Add);
    const SUBTRACT: Key = Key::Operator(Operator::Subtract);
    const MULTIPLY: Key = Key::Operator(Operator::Multiply);
    const DIVIDE: Key = Key::Operator(Operator::Divide);

    #[test]
    fn digits_are_typed_left_to_right_and_a_leading_zero_goes() {
        assert_eq!(shows(&[]), "0");
        assert_eq!(shows(&[Digit(0), Digit(4), Digit(2)]), "42");
    }

    #[test]
    fn a_sum_is_finished_by_equals() {
        assert_eq!(shows(&[Digit(1), Digit(2), ADD, Digit(3), Equals]), "15");
        assert_eq!(shows(&[Digit(7), SUBTRACT, Digit(9), Equals]), "-2");
        assert_eq!(shows(&[Digit(6), MULTIPLY, Digit(7), Equals]), "42");
        assert_eq!(shows(&[Digit(1), DIVIDE, Digit(4), Equals]), "0.25");
    }

    #[test]
    fn a_second_operator_finishes_the_first_sum_left_to_right() {
        assert_eq!(shows(&[Digit(2), ADD, Digit(3), MULTIPLY]), "5");
        assert_eq!(shows(&[Digit(2), ADD, Digit(3), MULTIPLY, Digit(4), Equals]), "20");
    }

    #[test]
    fn two_operators_in_a_row_are_a_change_of_mind() {
        assert_eq!(shows(&[Digit(8), ADD, SUBTRACT, Digit(3), Equals]), "5");
        assert_eq!(keyed(&[Digit(8), ADD, SUBTRACT]).above(), Ok("8 \u{2212}".to_string()));
    }

    #[test]
    fn what_is_waiting_is_said_above_the_number() {
        assert_eq!(keyed(&[Digit(1), Digit(2), ADD]).above(), Ok("12 +".to_string()));
        assert_eq!(keyed(&[Digit(1), ADD, Digit(2), Equals]).above(), Ok(String::new()));
    }

    #[test]
    fn a_tenth_and_a_fifth_are_three_tenths_on_the_screen() {
        assert_eq!(shows(&[Point, Digit(1), ADD, Point, Digit(2), Equals]), "0.3");
    }

    #[test]
    fn a_point_is_typed_once_and_starts_a_fresh_number_at_nought() {
        assert_eq!(shows(&[Digit(1), Point, Point, Digit(5)]), "1.5");
        assert_eq!(shows(&[Digit(1), ADD, Point, Digit(5), Equals]), "1.5");
    }

    #[test]
    fn dividing_by_nothing_is_an_error_and_the_next_key_starts_again() {
        assert_eq!(shows(&[Digit(5), DIVIDE, Digit(0), Equals]), "Error");
        assert_eq!(shows(&[Digit(5), DIVIDE, Digit(0), Equals, Digit(3)]), "3");
    }

    #[test]
    fn sign_and_percent_change_the_number_in_front() {
        assert_eq!(shows(&[Digit(5), Sign]), "-5");
        assert_eq!(shows(&[Digit(5), Sign, Sign]), "5");
        assert_eq!(shows(&[Digit(5), Digit(0), Percent]), "0.5");
        assert_eq!(shows(&[Digit(9), ADD, Digit(1), Sign, Equals]), "8");
    }

    #[test]
    fn the_operator_waiting_for_a_number_is_the_one_lit() {
        assert_eq!(keyed(&[Digit(2), MULTIPLY]).lit(), Ok(Some(Operator::Multiply)));
        assert_eq!(keyed(&[Digit(2), MULTIPLY, Digit(3)]).lit(), Ok(None));
    }

    #[test]
    fn clear_forgets_everything() {
        assert_eq!(keyed(&[Digit(9), ADD, Digit(1), Clear]), Sum::default());
    }

    #[test]
    fn a_number_is_not_typed_past_what_the_screen_holds() {
        let long = vec![Digit(9); 40];
        let Ok(kept) = console_core_number_conversion::fitted::<_, u32>(shows(&long).chars().count());

        assert_eq!(kept, LONGEST);
    }

    #[test]
    fn a_huge_answer_is_said_in_powers_of_ten() {
        let Ok(said) = said(1.5e20);

        assert_eq!(said, "1.5e20");
    }

    #[test]
    fn backspace_takes_back_the_last_digit_typed_and_never_an_answer() {
        assert_eq!(shows(&[Digit(4), Digit(2), Backspace]), "4");
        assert_eq!(shows(&[Digit(4), Backspace]), "0");
        assert_eq!(shows(&[Digit(7), Sign, Backspace, Backspace]), "0");
        assert_eq!(shows(&[Digit(6), MULTIPLY, Digit(7), Equals, Backspace]), "42");
    }
}
