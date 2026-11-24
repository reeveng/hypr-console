//! Monitor declarations and the Lua configuration used to apply them.

use std::fmt;

use console_core_never::Never;
use console_core_screen_orientation::{ParseError, Transform};
use console_core_size::Size;

use super::quote;
use crate::Script;
use crate::setup::OutputScale;

const MONITOR: &str = "hl.monitor";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenDeclaration {
  pub mode: Size<u32>,
  pub refresh: u32,
  pub scale: OutputScale,
  pub transform: Transform,
}

impl ScreenDeclaration {
  pub fn read(lua: &str) -> Result<Self, Undeclared> {
    let Ok(found) = value_between(
      lua,
      Named(MONITOR),
      Wrapped {
        open: '{',
        close: '}',
      },
    );

    let block = found.ok_or(Undeclared::NoScreen)?;

    let Ok(said) = after(&block, Named("mode"));

    let mode = said.ok_or(Undeclared::NoMode)?;
    let (wide, rest) = mode.split_once('x').ok_or(Undeclared::ModeShape)?;
    let (tall, refresh) = rest.split_once('@').ok_or(Undeclared::NoRefresh)?;
    let wide_2 = number(wide)?;
    let tall_2 = number(tall)?;
    let refresh_2 = number(refresh)?;
    let Ok(found_2) = after(&block, Named("scale"));

    let said_2 = found_2.ok_or(Undeclared::NoScale)?;
    let scale = said_2.parse().map_err(|_| Undeclared::ScaleNotANumber)?;

    let Ok(found_3) = after(&block, Named("transform"));

    let turn = found_3.ok_or(Undeclared::NoTransform)?;
    let numbered = number(&turn)?;
    let transform = Transform::of(i64::from(numbered)).map_err(Undeclared::NotATransform)?;

    Ok(Self {
      mode: Size {
        width: wide_2,
        height: tall_2,
      },
      refresh: refresh_2,
      scale: OutputScale(scale),
      transform,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undeclared {
  NoScreen,
  NoMode,
  ModeShape,
  NoRefresh,
  NoScale,
  ScaleNotANumber,
  NoTransform,
  NotATransform(ParseError),
  NotAWholeNumber(String),
}

impl fmt::Display for Undeclared {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Undeclared::NoScreen => write!(to, "the compositor's file declares no screen"),
      Undeclared::NoMode => write!(to, "the screen says nothing about its mode"),
      Undeclared::ModeShape => write!(to, "that mode is not WIDTHxHEIGHT"),
      Undeclared::NoRefresh => write!(to, "that mode names no refresh"),
      Undeclared::NoScale => write!(to, "the screen says nothing about its scale"),
      Undeclared::ScaleNotANumber => write!(to, "that scale is not a number"),
      Undeclared::NoTransform => write!(to, "the screen says nothing about its transform"),
      Undeclared::NotATransform(fault) => write!(to, "{fault}"),
      Undeclared::NotAWholeNumber(said) => write!(to, "{said:?} is not a whole number"),
    }
  }
}

impl std::error::Error for Undeclared {}

struct Wrapped {
  open: char,
  close: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Named<'a>(&'a str);

fn value_between(text: &str, name: Named<'_>, wrapped: Wrapped) -> Result<Option<String>, Never> {
  let Wrapped { open, close } = wrapped;
  let name_2 = name.0;

  Ok(
    text
      .split_once(name_2)
      .and_then(|(_, after_name)| after_name.split_once(open))
      .and_then(|(_, inside)| inside.split_once(close))
      .map(|(between, _)| between.to_string()),
  )
}

fn after(block: &str, name: Named<'_>) -> Result<Option<String>, Never> {
  let name_2 = name.0;

  let after_name = match block.split_once(name_2) {
    Some((_, after_name)) => after_name,
    None => return Ok(None),
  };

  let rest = after_name.trim_start();

  let valued = match rest.strip_prefix('=') {
    Some(valued) => valued,
    None => return Ok(None),
  };

  let rest_2 = valued.trim_start();
  let said: String = match rest_2.strip_prefix('"') {
    Some(quoted) => quoted
      .chars()
      .take_while(|character| *character != '"')
      .collect(),
    None => rest_2
      .chars()
      .take_while(|character| character.is_ascii_digit() || *character == '.')
      .collect(),
  };

  Ok(match said.as_str() {
    "" => None,
    _nonempty => Some(said),
  })
}

fn number(said: &str) -> Result<u32, Undeclared> {
  said
    .trim()
    .parse()
    .map_err(|_| Undeclared::NotAWholeNumber(said.to_string()))
}

#[derive(Debug, Clone, Copy)]
pub struct MonitorRule<'a> {
  pub output: &'a str,
  pub size: Size<u32>,
  pub refresh: Option<u32>,
  pub scale: OutputScale,
  pub transform: Transform,
}

impl MonitorRule<'_> {
  pub fn configuration(self) -> Result<Script, Never> {
    let Ok(monitor) = self.lua();
    let Ok(touch) = self.touch();

    Script::from_lua(format!("{monitor} {touch}"))
  }

  pub fn lua(self) -> Result<String, Never> {
    let Ok(output) = quote(self.output);
    let Ok(transform) = self.transform.number();
    let Size { width, height } = self.size;
    let mode = match self.refresh {
      Some(refresh) => format!("{width}x{height}@{refresh}"),
      None => format!("{width}x{height}"),
    };
    let scale = self.scale;

    Ok(format!(
      r#"{MONITOR}({{ output = {output}, mode = "{mode}", position = "auto", scale = {scale}, transform = {transform} }})"#
    ))
  }

  pub fn touch(self) -> Result<String, Never> {
    let Ok(transform) = self.transform.number();

    touch_transform(self.output, transform)
  }
}

pub(crate) fn touch_transform(output: &str, transform: i64) -> Result<String, Never> {
  let Ok(output_2) = quote(output);

  Ok(format!(
    "hl.config({{ input = {{ touchdevice = {{ output = {output}, transform = {transform} }} }} }})",
    output = output_2
  ))
}

pub fn disable_monitor(output: &str) -> Result<String, Never> {
  let Ok(output_2) = quote(output);

  Ok(format!(
    "{MONITOR}({{ output = {output}, disabled = true }})",
    output = output_2
  ))
}
