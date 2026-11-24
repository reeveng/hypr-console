//! Local calendar boundaries keep charts aligned on daylight-saving days.

use std::io::Write;
use std::process::Stdio;

use console_core_external_programs::Program;
use console_panel::activity::Period;

use crate::Result;

const DAYS: u32 = 30;
pub const CURRENT_DAY: u32 = DAYS.saturating_sub(1);

pub struct Day {
  pub start: u64,
  pub end: u64,
  pub iso: String,
  pub weekday: String,
  pub month: String,
  pub number: String,
  pub title: String,
}

impl Day {
  pub fn ticks(&self, hours: impl IntoIterator<Item = u8>) -> Result<Vec<(u64, String)>> {
    let times = hours
      .into_iter()
      .map(|hour| format!("{} {hour:02}:00", self.iso));
    let output = dates(times, "%s|%H:%M")?;
    output
      .lines()
      .map(|line| {
        let (time, label) = line.split_once('|').ok_or("calendar tick unavailable")?;
        let checked_value = time.parse()?;
        Ok((checked_value, label.to_owned()))
      })
      .collect()
  }
}

pub fn local_days(at: u64) -> Result<Vec<Day>> {
  let today = dates([format!("@{at}")], "%F")?;
  let today_2 = today.trim();
  let lines = (0..DAYS)
    .rev()
    .map(|offset| format!("{today} 00:00 {offset} days ago", today = today_2))
    .chain([format!("{today} 00:00 1 day", today = today_2)]);
  let output = dates(lines, "%s|%F|%a|%b|%-d|%A, %B %-d")?;
  let mut days = output
    .lines()
    .map(|line| {
      let fields: Vec<_> = line.split('|').collect();

      let [seconds, iso, weekday, month, number, title] = match <[&str; 6]>::try_from(fields) {
        Ok(fields_2) => fields_2,
        Err(_fields) => return Err(Box::from("calendar day unavailable")),
      };

      let start = seconds.parse()?;
      Ok(Day {
        start,
        end: 0,
        iso: String::from(iso),
        weekday: String::from(weekday),
        month: String::from(month),
        number: String::from(number),
        title: String::from(title),
      })
    })
    .collect::<Result<Vec<_>>>()?;

  let received = u32::try_from(days.len())?;

  match bool::then_some(received == DAYS.saturating_add(1), ()) {
    Some(()) => {}
    None => return Err(Box::from("calendar has an incomplete date range")),
  }

  let sentinel = days.pop().ok_or("calendar end unavailable")?;
  let mut end = sentinel.start;

  for day in days.iter_mut().rev() {
    day.end = end;
    end = day.start;
  }

  Ok(days)
}

pub fn time_range(Period { start, end }: Period) -> Result<String> {
  let output = dates([format!("@{start}"), format!("@{end}")], "%H:%M")?;
  Ok(output.lines().collect::<Vec<_>>().join("–"))
}

fn dates(operands: impl IntoIterator<Item = String>, format: &str) -> Result<String> {
  let Ok(mut command) = Program::Date.command();
  let mut process = command
    .args(["-f", "-", &format!("+{format}")])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

  match process.stdin.take() {
    Some(mut input) => {
      for operand in operands {
        writeln!(input, "{operand}")?;
      }
    }
    None => {}
  }

  let output = process.wait_with_output()?;

  match bool::then_some(!output.status.success(), ()) {
    Some(()) => {
      let message = String::from_utf8(output.stderr)?;
      return Err(Box::from(message));
    }
    None => {}
  }

  Ok({
    let checked_value = String::from_utf8(output.stdout)?;
    checked_value
  })
}
