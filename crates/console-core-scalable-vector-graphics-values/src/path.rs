use crate::{Error, Stream};
use alloc::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateMode {
  Absolute,
  Relative,
}

impl From<bool> for CoordinateMode {
  fn from(flag: bool) -> Self {
    match flag.then_some(()) {
      Some(()) => Self::Absolute,
      None => Self::Relative,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcSize {
  Large,
  Small,
}

impl From<bool> for ArcSize {
  fn from(flag: bool) -> Self {
    match flag.then_some(()) {
      Some(()) => Self::Large,
      None => Self::Small,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcSweep {
  Positive,
  Negative,
}

impl From<bool> for ArcSweep {
  fn from(flag: bool) -> Self {
    match flag.then_some(()) {
      Some(()) => Self::Positive,
      None => Self::Negative,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathSegment {
  MoveTo {
    abs: CoordinateMode,
    x: f64,
    y: f64,
  },
  LineTo {
    abs: CoordinateMode,
    x: f64,
    y: f64,
  },
  HorizontalLineTo {
    abs: CoordinateMode,
    x: f64,
  },
  VerticalLineTo {
    abs: CoordinateMode,
    y: f64,
  },
  CurveTo {
    abs: CoordinateMode,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
  },
  SmoothCurveTo {
    abs: CoordinateMode,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
  },
  Quadratic {
    abs: CoordinateMode,
    x1: f64,
    y1: f64,
    x: f64,
    y: f64,
  },
  SmoothQuadratic {
    abs: CoordinateMode,
    x: f64,
    y: f64,
  },
  EllipticalArc {
    abs: CoordinateMode,
    rx: f64,
    ry: f64,
    x_axis_rotation: f64,
    large_arc: ArcSize,
    sweep: ArcSweep,
    x: f64,
    y: f64,
  },
  ClosePath {
    abs: CoordinateMode,
  },
}

impl PathSegment {
  pub fn coordinate_mode(&self) -> Result<CoordinateMode, console_core_never::Never> {
    Ok(match *self {
      Self::MoveTo { abs, .. }
      | Self::LineTo { abs, .. }
      | Self::HorizontalLineTo { abs, .. }
      | Self::VerticalLineTo { abs, .. }
      | Self::CurveTo { abs, .. }
      | Self::SmoothCurveTo { abs, .. }
      | Self::Quadratic { abs, .. }
      | Self::SmoothQuadratic { abs, .. }
      | Self::EllipticalArc { abs, .. }
      | Self::ClosePath { abs } => abs,
    })
  }

  pub fn command(&self) -> core::result::Result<u8, console_core_never::Never> {
    let Ok(mode) = self.coordinate_mode();

    Ok(match mode {
      CoordinateMode::Absolute => match *self {
        Self::MoveTo { .. } => b'M',
        Self::LineTo { .. } => b'L',
        Self::HorizontalLineTo { .. } => b'H',
        Self::VerticalLineTo { .. } => b'V',
        Self::CurveTo { .. } => b'C',
        Self::SmoothCurveTo { .. } => b'S',
        Self::Quadratic { .. } => b'Q',
        Self::SmoothQuadratic { .. } => b'T',
        Self::EllipticalArc { .. } => b'A',
        Self::ClosePath { .. } => b'Z',
      },
      CoordinateMode::Relative => match *self {
        Self::MoveTo { .. } => b'm',
        Self::LineTo { .. } => b'l',
        Self::HorizontalLineTo { .. } => b'h',
        Self::VerticalLineTo { .. } => b'v',
        Self::CurveTo { .. } => b'c',
        Self::SmoothCurveTo { .. } => b's',
        Self::Quadratic { .. } => b'q',
        Self::SmoothQuadratic { .. } => b't',
        Self::EllipticalArc { .. } => b'a',
        Self::ClosePath { .. } => b'z',
      },
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PathParser<'a> {
  stream: Stream<'a>,
  prev_cmd: Option<u8>,
}

impl<'a> From<&'a str> for PathParser<'a> {
  #[inline]
  fn from(v: &'a str) -> Self {
    PathParser {
      stream: Stream::from(v),
      prev_cmd: None,
    }
  }
}

impl Iterator for PathParser<'_> {
  type Item = Result<PathSegment, Error>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    let s = &mut self.stream;

    let Ok(()) = s.skip_spaces();

    let Ok(at_end) = s.at_end();

    match at_end {
      crate::End::Reached => {
        return None;
      }
      crate::End::Pending => {}
    }

    let res = next_impl(s, &mut self.prev_cmd);

    match res {
      Err(ref _invalid_path_ends_stream) => {
        let Ok(()) = s.jump_to_end();
      }
      ::core::result::Result::Ok(_) => {}
    }

    Some(res)
  }
}

fn next_impl(s: &mut Stream<'_>, prev_cmd: &mut Option<u8>) -> Result<PathSegment, Error> {
  let Ok(start) = s.pos();

  let first_char = s.curr_byte()?;

  match (*prev_cmd, first_char) {
    (None, b'M' | b'm') | (Some(_), _) => {}
    (None, _) => {
      let Ok(position) = s.calc_char_pos_at(start);

      return Err(Error::UnexpectedData(position));
    }
  }

  let Ok(command_kind) = is_cmd(first_char);
  let (cmd, next_previous) = match command_kind {
    crate::IsCmdAnswer::Yes => {
      let Ok(()) = s.advance(1);

      (first_char, first_char)
    }
    crate::IsCmdAnswer::No => {
      let Ok(number_start) = is_number_start(first_char);

      match (number_start, *prev_cmd) {
        (crate::IsNumberStartAnswer::Yes, Some(b'M')) => (b'L', b'M'),
        (crate::IsNumberStartAnswer::Yes, Some(b'm')) => (b'l', b'm'),
        (crate::IsNumberStartAnswer::Yes, Some(b'Z' | b'z') | None)
        | (crate::IsNumberStartAnswer::No, _) => {
          let Ok(position) = s.calc_char_pos_at(start);

          return Err(Error::UnexpectedData(position));
        }
        (crate::IsNumberStartAnswer::Yes, Some(command)) => (command, command),
      }
    }
  };

  let Ok(cmdl) = to_relative(cmd);
  let Ok(mode) = command_mode(cmd);
  let token = match cmdl {
    b'm' => {
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::MoveTo { abs: mode, x, y }
    }
    b'l' => {
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::LineTo { abs: mode, x, y }
    }
    b'h' => {
      let x = s.parse_list_number()?;

      PathSegment::HorizontalLineTo { abs: mode, x }
    }
    b'v' => {
      let y = s.parse_list_number()?;

      PathSegment::VerticalLineTo { abs: mode, y }
    }
    b'c' => {
      let x1 = s.parse_list_number()?;
      let y1 = s.parse_list_number()?;
      let x2 = s.parse_list_number()?;
      let y2 = s.parse_list_number()?;
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::CurveTo {
        abs: mode,
        x1,
        y1,
        x2,
        y2,
        x,
        y,
      }
    }
    b's' => {
      let x2 = s.parse_list_number()?;
      let y2 = s.parse_list_number()?;
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::SmoothCurveTo {
        abs: mode,
        x2,
        y2,
        x,
        y,
      }
    }
    b'q' => {
      let x1 = s.parse_list_number()?;
      let y1 = s.parse_list_number()?;
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::Quadratic {
        abs: mode,
        x1,
        y1,
        x,
        y,
      }
    }
    b't' => {
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::SmoothQuadratic { abs: mode, x, y }
    }
    b'a' => {
      let rx = s.parse_list_number()?;
      let ry = s.parse_list_number()?;
      let x_axis_rotation = s.parse_list_number()?;
      let checked_value = parse_flag(s)?;
      let large_arc = match checked_value {
        ArcFlag::One => ArcSize::Large,
        ArcFlag::Zero => ArcSize::Small,
      };
      let checked_value_2 = parse_flag(s)?;
      let sweep = match checked_value_2 {
        ArcFlag::One => ArcSweep::Positive,
        ArcFlag::Zero => ArcSweep::Negative,
      };
      let x = s.parse_list_number()?;
      let y = s.parse_list_number()?;

      PathSegment::EllipticalArc {
        abs: mode,
        rx,
        ry,
        x_axis_rotation,
        large_arc,
        sweep,
        x,
        y,
      }
    }
    b'z' => PathSegment::ClosePath { abs: mode },
    _ => {
      let Ok(position) = s.calc_char_pos_at(start);

      return Err(Error::UnexpectedData(position));
    }
  };

  *prev_cmd = Some(next_previous);

  Ok(token)
}

#[inline]
fn is_cmd(c: u8) -> Result<crate::IsCmdAnswer, console_core_never::Never> {
  Ok(match c {
    b'M' | b'm' | b'Z' | b'z' | b'L' | b'l' | b'H' | b'h' | b'V' | b'v' | b'C' | b'c' | b'S'
    | b's' | b'Q' | b'q' | b'T' | b't' | b'A' | b'a' => crate::IsCmdAnswer::Yes,
    _ => crate::IsCmdAnswer::No,
  })
}

#[inline]
fn command_mode(c: u8) -> Result<CoordinateMode, console_core_never::Never> {
  Ok(match c {
    b'M' | b'Z' | b'L' | b'H' | b'V' | b'C' | b'S' | b'Q' | b'T' | b'A' => {
      CoordinateMode::Absolute
    }
    _ => CoordinateMode::Relative,
  })
}

#[inline]
fn to_relative(c: u8) -> core::result::Result<u8, console_core_never::Never> {
  Ok(match c {
    b'M' => b'm',
    b'Z' => b'z',
    b'L' => b'l',
    b'H' => b'h',
    b'V' => b'v',
    b'C' => b'c',
    b'S' => b's',
    b'Q' => b'q',
    b'T' => b't',
    b'A' => b'a',
    _ => c,
  })
}

#[inline]
fn is_number_start(
  c: u8,
) -> core::result::Result<crate::IsNumberStartAnswer, console_core_never::Never> {
  Ok(match c {
    b'0'..=b'9' | b'.' | b'-' | b'+' => crate::IsNumberStartAnswer::Yes,
    _ => crate::IsNumberStartAnswer::No,
  })
}

#[derive(Clone, Copy, Debug)]
enum ArcFlag {
  Zero,
  One,
}

fn parse_flag(s: &mut Stream<'_>) -> Result<ArcFlag, Error> {
  let Ok(()) = s.skip_spaces();

  let checked_value = s.curr_byte()?;
  let flag = match checked_value {
    b'0' => ArcFlag::Zero,
    b'1' => ArcFlag::One,
    _ => {
      let Ok(start) = s.pos();
      let Ok(position) = s.calc_char_pos_at(start);

      return Err(Error::UnexpectedData(position));
    }
  };

  let Ok(()) = s.advance(1);
  let Ok(()) = s.parse_list_separator();
  let Ok(()) = s.skip_spaces();

  Ok(flag)
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test {
        ($name:ident, $text:expr, $( $seg:expr ),*) => (
            #[test]
            fn $name() {
                let mut s = PathParser::from($text);
                $(
                    assert_eq!(s.next(), Some(Ok($seg)));
                )*

                assert!(!matches!(s.next(), Some(Ok(_))));
            }
        )
    }

    test!(null, "", );
    test!(not_a_path, "q", );
    test!(not_a_move_to, "L 20 30", );
    test!(stop_on_err_1, "M 10 20 L 30 40 L 50",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 }
    );

    test!(move_to_1, "M 10 20", PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 });
    test!(move_to_2, "m 10 20", PathSegment::MoveTo { abs: CoordinateMode::Relative, x: 10.0, y: 20.0 });
    test!(move_to_3, "M 10 20 30 40 50 60",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 50.0, y: 60.0 }
    );
    test!(move_to_4, "M 10 20 30 40 50 60 M 70 80 90 100 110 120",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 50.0, y: 60.0 },
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 70.0, y: 80.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 90.0, y: 100.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 110.0, y: 120.0 }
    );

    test!(arc_to_1, "M 10 20 A 5 5 30 1 1 20 20",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::EllipticalArc {
            abs: CoordinateMode::Absolute,
            rx: 5.0, ry: 5.0,
            x_axis_rotation: 30.0,
            large_arc: ArcSize::Large, sweep: ArcSweep::Positive,
            x: 20.0, y: 20.0
        }
    );

    test!(arc_to_2, "M 10 20 a 5 5 30 0 0 20 20",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::EllipticalArc {
            abs: CoordinateMode::Relative,
            rx: 5.0, ry: 5.0,
            x_axis_rotation: 30.0,
            large_arc: ArcSize::Small, sweep: ArcSweep::Negative,
            x: 20.0, y: 20.0
        }
    );

    test!(arc_to_10, "M10-20A5.5.3-4 010-.1",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: -20.0 },
        PathSegment::EllipticalArc {
            abs: CoordinateMode::Absolute,
            rx: 5.5, ry: 0.3,
            x_axis_rotation: -4.0,
            large_arc: ArcSize::Small, sweep: ArcSweep::Positive,
            x: 0.0, y: -0.1
        }
    );

    test!(separator_1, "M 10 20 L 5 15 C 10 20 30 40 50 60",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 5.0, y: 15.0 },
        PathSegment::CurveTo {
            abs: CoordinateMode::Absolute,
            x1: 10.0, y1: 20.0,
            x2: 30.0, y2: 40.0,
            x:  50.0, y:  60.0,
        }
    );

    test!(separator_2, "M 10, 20 L 5, 15 C 10, 20 30, 40 50, 60",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 5.0, y: 15.0 },
        PathSegment::CurveTo {
            abs: CoordinateMode::Absolute,
            x1: 10.0, y1: 20.0,
            x2: 30.0, y2: 40.0,
            x:  50.0, y:  60.0,
        }
    );

    test!(separator_3, "M 10,20 L 5,15 C 10,20 30,40 50,60",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 5.0, y: 15.0 },
        PathSegment::CurveTo {
            abs: CoordinateMode::Absolute,
            x1: 10.0, y1: 20.0,
            x2: 30.0, y2: 40.0,
            x:  50.0, y:  60.0,
        }
    );

    test!(separator_4, "M10, 20 L5, 15 C10, 20 30 40 50 60",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 5.0, y: 15.0 },
        PathSegment::CurveTo {
            abs: CoordinateMode::Absolute,
            x1: 10.0, y1: 20.0,
            x2: 30.0, y2: 40.0,
            x:  50.0, y:  60.0,
        }
    );

    test!(separator_5, "M10 20V30H40V50H60Z",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y: 30.0 },
        PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x: 40.0 },
        PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y: 50.0 },
        PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x: 60.0 },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute }
    );

    test!(all_segments_1, "M 10 20 L 30 40 H 50 V 60 C 70 80 90 100 110 120 S 130 140 150 160
        Q 170 180 190 200 T 210 220 A 50 50 30 1 1 230 240 Z",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 },
        PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x: 50.0 },
        PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y: 60.0 },
        PathSegment::CurveTo {
            abs: CoordinateMode::Absolute,
            x1:  70.0, y1:  80.0,
            x2:  90.0, y2: 100.0,
            x:  110.0, y:  120.0,
        },
        PathSegment::SmoothCurveTo {
            abs: CoordinateMode::Absolute,
            x2: 130.0, y2: 140.0,
            x:  150.0, y:  160.0,
        },
        PathSegment::Quadratic {
            abs: CoordinateMode::Absolute,
            x1: 170.0, y1: 180.0,
            x:  190.0, y:  200.0,
        },
        PathSegment::SmoothQuadratic { abs: CoordinateMode::Absolute, x: 210.0, y: 220.0 },
        PathSegment::EllipticalArc {
            abs: CoordinateMode::Absolute,
            rx: 50.0, ry: 50.0,
            x_axis_rotation: 30.0,
            large_arc: ArcSize::Large, sweep: ArcSweep::Positive,
            x: 230.0, y: 240.0
        },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute }
    );

    test!(all_segments_2, "m 10 20 l 30 40 h 50 v 60 c 70 80 90 100 110 120 s 130 140 150 160
        q 170 180 190 200 t 210 220 a 50 50 30 1 1 230 240 z",
        PathSegment::MoveTo { abs: CoordinateMode::Relative, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Relative, x: 30.0, y: 40.0 },
        PathSegment::HorizontalLineTo { abs: CoordinateMode::Relative, x: 50.0 },
        PathSegment::VerticalLineTo { abs: CoordinateMode::Relative, y: 60.0 },
        PathSegment::CurveTo {
            abs: CoordinateMode::Relative,
            x1:  70.0, y1:  80.0,
            x2:  90.0, y2: 100.0,
            x:  110.0, y:  120.0,
        },
        PathSegment::SmoothCurveTo {
            abs: CoordinateMode::Relative,
            x2: 130.0, y2: 140.0,
            x:  150.0, y:  160.0,
        },
        PathSegment::Quadratic {
            abs: CoordinateMode::Relative,
            x1: 170.0, y1: 180.0,
            x:  190.0, y:  200.0,
        },
        PathSegment::SmoothQuadratic { abs: CoordinateMode::Relative, x: 210.0, y: 220.0 },
        PathSegment::EllipticalArc {
            abs: CoordinateMode::Relative,
            rx: 50.0, ry: 50.0,
            x_axis_rotation: 30.0,
            large_arc: ArcSize::Large, sweep: ArcSweep::Positive,
            x: 230.0, y: 240.0
        },
        PathSegment::ClosePath { abs: CoordinateMode::Relative }
    );

    test!(close_path_1, "M10 20 L 30 40 ZM 100 200 L 300 400",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute },
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 100.0, y: 200.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 300.0, y: 400.0 }
    );

    test!(close_path_2, "M10 20 L 30 40 zM 100 200 L 300 400",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 },
        PathSegment::ClosePath { abs: CoordinateMode::Relative },
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 100.0, y: 200.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 300.0, y: 400.0 }
    );

    test!(close_path_3, "M10 20 L 30 40 Z Z Z",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 10.0, y: 20.0 },
        PathSegment::LineTo { abs: CoordinateMode::Absolute, x: 30.0, y: 40.0 },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute }
    );

    test!(invalid_1, "M\t.", );

    test!(invalid_2, "M 0 0 Z 2",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 0.0, y: 0.0 },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute }
    );

    test!(invalid_3, "M 0 0 Z H 10",
        PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 0.0, y: 0.0 },
        PathSegment::ClosePath { abs: CoordinateMode::Absolute },
        PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x: 10.0 }
    );

    #[test]
    fn test_helper_functions() {
        assert_eq!(PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 0.0, y: 0.0 }.coordinate_mode(), Ok(CoordinateMode::Absolute));
        assert_eq!(PathSegment::MoveTo { abs: CoordinateMode::Relative, x: 0.0, y: 0.0 }.coordinate_mode(), Ok(CoordinateMode::Relative));
        assert_eq!(PathSegment::MoveTo { abs: CoordinateMode::Absolute, x: 0.0, y: 0.0 }.command(), Ok(b'M'));
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SimplePathSegment {
  MoveTo {
    x: f64,
    y: f64,
  },
  LineTo {
    x: f64,
    y: f64,
  },
  CurveTo {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
  },
  Quadratic {
    x1: f64,
    y1: f64,
    x: f64,
    y: f64,
  },
  ClosePath,
}

#[derive(Clone, Debug)]
pub struct SimplifyingPathParser<'a> {
  parser: PathParser<'a>,

  previous_move: kurbo::Point,
  previous_control: kurbo::Point,
  previous_point: kurbo::Point,

  prev_seg: PathSegment,
  prev_simple_seg: Option<SimplePathSegment>,

  buffer: VecDeque<SimplePathSegment>,
}

impl<'a> From<&'a str> for SimplifyingPathParser<'a> {
  #[inline]
  fn from(v: &'a str) -> Self {
    SimplifyingPathParser {
      parser: PathParser::from(v),
      previous_move: kurbo::Point::ORIGIN,
      previous_control: kurbo::Point::ORIGIN,
      previous_point: kurbo::Point::ORIGIN,
      prev_seg: PathSegment::MoveTo {
        abs: CoordinateMode::Absolute,
        x: 0.0,
        y: 0.0,
      },
      prev_simple_seg: None,
      buffer: VecDeque::new(),
    }
  }
}

impl Iterator for SimplifyingPathParser<'_> {
  type Item = Result<SimplePathSegment, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.buffer.pop_front() {
      Some(segment) => return Some(Ok(segment)),
      None => {}
    }

    let Ok(remaining) = self.parser.stream.remaining();

    for _ in 0..remaining {
      let next = self.parser.next()?;
      let segment = match next {
        Ok(segment) => segment,
        Err(fault) => return Some(Err(fault)),
      };
      let Ok(()) = self.append(segment);

      match self.buffer.pop_front() {
        Some(segment_2) => return Some(Ok(segment_2)),
        None => {}
      }
    }

    None
  }
}

impl SimplifyingPathParser<'_> {
  fn append(&mut self, segment: PathSegment) -> Result<(), console_core_never::Never> {
    match (self.prev_simple_seg, segment) {
      (
        Some(SimplePathSegment::ClosePath),
        PathSegment::LineTo { .. }
        | PathSegment::HorizontalLineTo { .. }
        | PathSegment::VerticalLineTo { .. }
        | PathSegment::CurveTo { .. }
        | PathSegment::SmoothCurveTo { .. }
        | PathSegment::Quadratic { .. }
        | PathSegment::SmoothQuadratic { .. }
        | PathSegment::EllipticalArc { .. },
      ) => {
        let implicit = SimplePathSegment::MoveTo {
          x: self.previous_move.x,
          y: self.previous_move.y,
        };
        self.buffer.push_back(implicit);
        self.prev_simple_seg = Some(implicit);
      }
      (
        Some(SimplePathSegment::ClosePath),
        PathSegment::MoveTo { .. } | PathSegment::ClosePath { .. },
      )
      | (
        Some(
          SimplePathSegment::MoveTo { .. }
          | SimplePathSegment::LineTo { .. }
          | SimplePathSegment::CurveTo { .. }
          | SimplePathSegment::Quadratic { .. },
        ),
        _,
      )
      | (None, _) => {}
    }

    match segment {
      PathSegment::MoveTo { abs, mut x, mut y } => {
        match abs {
          CoordinateMode::Relative => match self.prev_simple_seg {
            Some(SimplePathSegment::ClosePath) => {
              x += self.previous_move.x;
              y += self.previous_move.y;
            }
            Some(
              SimplePathSegment::MoveTo { .. }
              | SimplePathSegment::LineTo { .. }
              | SimplePathSegment::CurveTo { .. }
              | SimplePathSegment::Quadratic { .. },
            )
            | None => {
              x += self.previous_point.x;
              y += self.previous_point.y;
            }
          },
          CoordinateMode::Absolute => {}
        }

        self.buffer.push_back(SimplePathSegment::MoveTo { x, y });
        self.prev_seg = segment;
      }
      PathSegment::LineTo { abs, mut x, mut y } => {
        match abs {
          CoordinateMode::Relative => {
            x += self.previous_point.x;
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        self.buffer.push_back(SimplePathSegment::LineTo { x, y });
        self.prev_seg = segment;
      }
      PathSegment::HorizontalLineTo { abs, mut x } => {
        match abs {
          CoordinateMode::Relative => {
            x += self.previous_point.x;
          }
          CoordinateMode::Absolute => {}
        }

        self.buffer.push_back(SimplePathSegment::LineTo {
          x,
          y: self.previous_point.y,
        });
        self.prev_seg = segment;
      }
      PathSegment::VerticalLineTo { abs, mut y } => {
        match abs {
          CoordinateMode::Relative => {
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        self.buffer.push_back(SimplePathSegment::LineTo {
          x: self.previous_point.x,
          y,
        });
        self.prev_seg = segment;
      }
      PathSegment::CurveTo {
        abs,
        mut x1,
        mut y1,
        mut x2,
        mut y2,
        mut x,
        mut y,
      } => {
        match abs {
          CoordinateMode::Relative => {
            x1 += self.previous_point.x;
            y1 += self.previous_point.y;
            x2 += self.previous_point.x;
            y2 += self.previous_point.y;
            x += self.previous_point.x;
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        self.buffer.push_back(SimplePathSegment::CurveTo {
          x1,
          y1,
          x2,
          y2,
          x,
          y,
        });

        self.prev_seg = PathSegment::CurveTo {
          abs: CoordinateMode::Absolute,
          x1,
          y1,
          x2,
          y2,
          x,
          y,
        };
      }
      PathSegment::SmoothCurveTo {
        abs,
        mut x2,
        mut y2,
        mut x,
        mut y,
      } => {
        let (x1, y1) = match self.prev_seg {
          PathSegment::CurveTo {
            x2: x2_2,
            y2: y2_2,
            x: x_2,
            y: y_2,
            ..
          }
          | PathSegment::SmoothCurveTo {
            x2: x2_2,
            y2: y2_2,
            x: x_2,
            y: y_2,
            ..
          } => (x_2 * 2.0 - x2_2, y_2 * 2.0 - y2_2),
          PathSegment::MoveTo { .. }
          | PathSegment::LineTo { .. }
          | PathSegment::HorizontalLineTo { .. }
          | PathSegment::VerticalLineTo { .. }
          | PathSegment::Quadratic { .. }
          | PathSegment::SmoothQuadratic { .. }
          | PathSegment::EllipticalArc { .. }
          | PathSegment::ClosePath { .. } => (self.previous_point.x, self.previous_point.y),
        };

        match abs {
          CoordinateMode::Relative => {
            x2 += self.previous_point.x;
            y2 += self.previous_point.y;
            x += self.previous_point.x;
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        self.buffer.push_back(SimplePathSegment::CurveTo {
          x1,
          y1,
          x2,
          y2,
          x,
          y,
        });

        self.prev_seg = PathSegment::SmoothCurveTo {
          abs: CoordinateMode::Absolute,
          x2,
          y2,
          x,
          y,
        };
      }
      PathSegment::Quadratic {
        abs,
        mut x1,
        mut y1,
        mut x,
        mut y,
      } => {
        match abs {
          CoordinateMode::Relative => {
            x1 += self.previous_point.x;
            y1 += self.previous_point.y;
            x += self.previous_point.x;
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        self
          .buffer
          .push_back(SimplePathSegment::Quadratic { x1, y1, x, y });

        self.prev_seg = PathSegment::Quadratic {
          abs: CoordinateMode::Absolute,
          x1,
          y1,
          x,
          y,
        };
      }
      PathSegment::SmoothQuadratic { abs, mut x, mut y } => {
        let (x1, y1) = match self.prev_seg {
          PathSegment::Quadratic {
            x1,
            y1,
            x: x_3,
            y: y_3,
            ..
          } => (x_3 * 2.0 - x1, y_3 * 2.0 - y1),
          PathSegment::SmoothQuadratic { x: x_4, y: y_4, .. } => (
            kurbo::bezier::reflected_control_doubled(x_4, self.previous_control.x),
            kurbo::bezier::reflected_control_doubled(y_4, self.previous_control.y),
          ),
          PathSegment::MoveTo { .. }
          | PathSegment::LineTo { .. }
          | PathSegment::HorizontalLineTo { .. }
          | PathSegment::VerticalLineTo { .. }
          | PathSegment::CurveTo { .. }
          | PathSegment::SmoothCurveTo { .. }
          | PathSegment::EllipticalArc { .. }
          | PathSegment::ClosePath { .. } => (self.previous_point.x, self.previous_point.y),
        };

        self.previous_control.x = x1;
        self.previous_control.y = y1;

        match abs {
          CoordinateMode::Relative => {
            x += self.previous_point.x;
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        self
          .buffer
          .push_back(SimplePathSegment::Quadratic { x1, y1, x, y });

        self.prev_seg = PathSegment::SmoothQuadratic {
          abs: CoordinateMode::Absolute,
          x,
          y,
        };
      }
      PathSegment::EllipticalArc {
        abs,
        rx,
        ry,
        x_axis_rotation,
        large_arc,
        sweep,
        mut x,
        mut y,
      } => {
        match abs {
          CoordinateMode::Relative => {
            x += self.previous_point.x;
            y += self.previous_point.y;
          }
          CoordinateMode::Absolute => {}
        }

        let svg_arc = kurbo::SvgArc {
          from: kurbo::Point::new(self.previous_point.x, self.previous_point.y),
          to: kurbo::Point::new(x, y),
          radii: kurbo::Vec2::new(rx, ry),
          x_rotation: x_axis_rotation.to_radians(),
          large_arc: matches!(large_arc, ArcSize::Large),
          sweep: matches!(sweep, ArcSweep::Positive),
        };

        let Ok(()) = self.append_arc(&svg_arc);

        self.prev_seg = segment;
      }
      PathSegment::ClosePath { .. } => {
        match self.prev_simple_seg {
          Some(SimplePathSegment::ClosePath) => {}
          Some(
            SimplePathSegment::MoveTo { .. }
            | SimplePathSegment::LineTo { .. }
            | SimplePathSegment::CurveTo { .. }
            | SimplePathSegment::Quadratic { .. },
          )
          | None => {
            self.buffer.push_back(SimplePathSegment::ClosePath);
          }
        }

        self.prev_seg = segment;
      }
    }

    match self.buffer.back() {
      Some(new_segment) => {
        self.prev_simple_seg = Some(*new_segment);

        match *new_segment {
          SimplePathSegment::MoveTo { x, y } => {
            self.previous_point.x = x;
            self.previous_point.y = y;
            self.previous_move.x = self.previous_point.x;
            self.previous_move.y = self.previous_point.y;
          }
          SimplePathSegment::LineTo { x, y } => {
            self.previous_point.x = x;
            self.previous_point.y = y;
          }
          SimplePathSegment::CurveTo { x, y, .. } => {
            self.previous_point.x = x;
            self.previous_point.y = y;
          }
          SimplePathSegment::Quadratic { x, y, .. } => {
            self.previous_point.x = x;
            self.previous_point.y = y;
          }
          SimplePathSegment::ClosePath => {
            self.previous_point.x = self.previous_move.x;
            self.previous_point.y = self.previous_move.y;
          }
        }
      }
      None => {}
    }

    Ok(())
  }

  fn append_arc(&mut self, svg: &kurbo::SvgArc) -> Result<(), console_core_never::Never> {
    let arc = match kurbo::Arc::from_svg_arc(svg) {
      Some(arc) => arc,
      None => {
        self.buffer.push_back(SimplePathSegment::LineTo {
          x: svg.to.x,
          y: svg.to.y,
        });

        return Ok(());
      }
    };

    for segment in arc.append_iter(0.1) {
      match segment {
        kurbo::PathEl::CurveTo(first, second, end) => {
          self.buffer.push_back(SimplePathSegment::CurveTo {
            x1: first.x,
            y1: first.y,
            x2: second.x,
            y2: second.y,
            x: end.x,
            y: end.y,
          });
        }
        kurbo::PathEl::MoveTo(_)
        | kurbo::PathEl::LineTo(_)
        | kurbo::PathEl::QuadTo(..)
        | kurbo::PathEl::ClosePath => break,
      }
    }

    Ok(())
  }
}

#[rustfmt::skip]
#[cfg(test)]
mod simple_tests {
    use super::*;
    use console_core_never::Never;

    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "each compact upstream fixture generates a separately named test with its own input and expected SVG value"
      )
    )]
    macro_rules! test {
        ($name:ident, $text:expr, $( $seg:expr ),*) => (
            #[test]
            fn $name() {
                let mut s = SimplifyingPathParser::from($text);
                $(
                    assert_eq!(s.next(), Some(Ok($seg)));
                )*

                assert!(!matches!(s.next(), Some(Ok(_))));
            }
        )
    }

    test!(ignore_duplicated_close_paths, "M 10 20 L 30 40 Z Z Z Z",
        SimplePathSegment::MoveTo { x: 10.0, y: 20.0 },
        SimplePathSegment::LineTo { x: 30.0, y: 40.0 },
        SimplePathSegment::ClosePath
    );

    #[test]
    fn many_duplicate_closes_preserve_the_implicit_move() {
        let mut text = alloc::string::String::from("M 10 20 Z");
        text.push_str(&" Z".repeat(10_000));
        text.push_str(" L 50 60");
        let segments: alloc::vec::Vec<_> = SimplifyingPathParser::from(text.as_str()).collect();

        assert_eq!(segments, [
            Ok(SimplePathSegment::MoveTo { x: 10.0, y: 20.0 }),
            Ok(SimplePathSegment::ClosePath),
            Ok(SimplePathSegment::MoveTo { x: 10.0, y: 20.0 }),
            Ok(SimplePathSegment::LineTo { x: 50.0, y: 60.0 }),
        ]);
    }

    test!(relative_move_to, "m 30 40 110 120 -20 -130",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::LineTo { x: 140.0, y: 160.0 },
        SimplePathSegment::LineTo { x: 120.0, y: 30.0 }
    );

    test!(smooth_curve_to_after_move_to, "M 30 40 S 171 45 180 155",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::CurveTo { x1: 30.0, y1: 40.0, x2: 171.0, y2: 45.0, x: 180.0, y: 155.0 }
    );

    test!(smooth_curve_to_after_curve_to, "M 30 40 C 16 137 171 45 100 90 S 171 45 180 155",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::CurveTo { x1: 16.0, y1: 137.0, x2: 171.0, y2: 45.0, x: 100.0, y: 90.0 },
        SimplePathSegment::CurveTo { x1: 29.0, y1: 135.0, x2: 171.0, y2: 45.0, x: 180.0, y: 155.0 }
    );

    test!(relative_smooth_curve_to_after_arc_to, "M 1 5 A 5 5 0 0 1 3 1 s 3 2 8 2",
        SimplePathSegment::MoveTo { x: 1.0, y: 5.0 },
        SimplePathSegment::CurveTo {
            x1: 1.0, y1: 3.4262134833347355,
            x2: 1.7409707866677877, y2: 1.9442719099991592,
            x: 2.9999999999999996, y: 1.0000000000000004
        },
        SimplePathSegment::CurveTo {
            x1: 2.9999999999999996, y1: 1.0000000000000004,
            x2: 6.0, y2: 3.0000000000000004,
            x: 11.0, y: 3.0000000000000004
        }
    );

    test!(smooth_quadratic_after_move_to, "M 30 40 T 180 155",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::Quadratic { x1: 30.0, y1: 40.0, x: 180.0, y: 155.0 }
    );

    test!(smooth_quadratic_after_quadratic, "M 30 40 Q 171 45 100 90 T 160 180",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::Quadratic { x1: 171.0, y1: 45.0, x: 100.0, y: 90.0 },
        SimplePathSegment::Quadratic { x1: 29.0, y1: 135.0, x: 160.0, y: 180.0 }
    );

    test!(relative_smooth_quadratic_after_quadratic, "M 30 40 Q 171 45 100 90 t 60 80",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::Quadratic { x1: 171.0, y1: 45.0, x: 100.0, y: 90.0 },
        SimplePathSegment::Quadratic { x1: 29.0, y1: 135.0, x: 160.0, y: 170.0 }
    );

    test!(relative_smooth_quadratic_after_relative_quadratic, "M 30 40 q 171 45 50 40 t 60 80",
        SimplePathSegment::MoveTo { x: 30.0, y: 40.0 },
        SimplePathSegment::Quadratic { x1: 201.0, y1: 85.0, x: 80.0, y: 80.0 },
        SimplePathSegment::Quadratic { x1: -41.0, y1: 75.0, x: 140.0, y: 160.0 }
    );

    test!(smooth_quadratic_after_smooth_quadratic, "M 30 30 T 40 140 T 170 30",
        SimplePathSegment::MoveTo { x: 30.0, y: 30.0 },
        SimplePathSegment::Quadratic { x1: 30.0, y1: 30.0, x: 40.0, y: 140.0 },
        SimplePathSegment::Quadratic { x1: 50.0, y1: 250.0, x: 170.0, y: 30.0 }
    );

    test!(smooth_quadratic_after_relative_smooth_quadratic, "M 30 30 T 40 140 t 100 -30",
        SimplePathSegment::MoveTo { x: 30.0, y: 30.0 },
        SimplePathSegment::Quadratic { x1: 30.0, y1: 30.0, x: 40.0, y: 140.0 },
        SimplePathSegment::Quadratic { x1: 50.0, y1: 250.0, x: 140.0, y: 110.0 }
    );

    test!(smooth_quadratic_after_relative_quadratic, "M 30 30 T 40 140 q 30 100 120 -30",
        SimplePathSegment::MoveTo { x: 30.0, y: 30.0 },
        SimplePathSegment::Quadratic { x1: 30.0, y1: 30.0, x: 40.0, y: 140.0 },
        SimplePathSegment::Quadratic { x1: 70.0, y1: 240.0, x: 160.0, y: 110.0 }
    );

    test!(smooth_quadratic_after_relative_smooth_curve_to, "M 30 30 T 40 170 s 90 -20 90 -90",
        SimplePathSegment::MoveTo { x: 30.0, y: 30.0 },
        SimplePathSegment::Quadratic { x1: 30.0, y1: 30.0, x: 40.0, y: 170.0 },
        SimplePathSegment::CurveTo { x1: 40.0, y1: 170.0, x2: 130.0, y2: 150.0, x: 130.0, y: 80.0 }
    );

    test!(quadratic_after_smooth_quadratic, "M 30 30 T 40 140 Q 80 180 170 30",
        SimplePathSegment::MoveTo { x: 30.0, y: 30.0 },
        SimplePathSegment::Quadratic { x1: 30.0, y1: 30.0, x: 40.0, y: 140.0 },
        SimplePathSegment::Quadratic { x1: 80.0, y1: 180.0, x: 170.0, y: 30.0 }
    );

    test!(relative_smooth_quadratic_to_after_arc_to, "M 1 5 A 5 5 0 0 1 3 1 t 8 2",
        SimplePathSegment::MoveTo { x: 1.0, y: 5.0 },
        SimplePathSegment::CurveTo {
            x1: 1.0, y1: 3.4262134833347355,
            x2: 1.7409707866677877, y2: 1.9442719099991592,
            x: 2.9999999999999996, y: 1.0000000000000004
        },
        SimplePathSegment::Quadratic {
            x1: 2.9999999999999996, y1: 1.0000000000000004,
            x: 11.0, y: 3.0000000000000004
        }
    );

    test!(implicit_move_to_after_close_path, "M 10 20 L 30 40 Z L 50 60",
        SimplePathSegment::MoveTo { x: 10.0, y: 20.0 },
        SimplePathSegment::LineTo { x: 30.0, y: 40.0 },
        SimplePathSegment::ClosePath,
        SimplePathSegment::MoveTo { x: 10.0, y: 20.0 },
        SimplePathSegment::LineTo { x: 50.0, y: 60.0 }
    );

    fn rounded(segment: Option<SimplePathSegment>) -> Result<Option<[f64; 6]>, Never> {
        Ok(match segment {
            Some(SimplePathSegment::CurveTo { x1, y1, x2, y2, x, y }) => {
                Some([x1.round(), y1.round(), x2.round(), y2.round(), x.round(), y.round()])
            }
            Some(SimplePathSegment::MoveTo { .. } | SimplePathSegment::LineTo { .. } | SimplePathSegment::Quadratic { .. } | SimplePathSegment::ClosePath) => None,
            None => None,
        })
    }

    #[test]
    fn arc_to() -> Result<(), Error> {
        let mut s = SimplifyingPathParser::from("M 30 40 A 40 30 20 1 1 150 100");
        assert_eq!(s.next(), Some(Ok(SimplePathSegment::MoveTo { x: 30.0, y: 40.0 })));
        let curve1 = s.next().transpose()?;
        let curve2 = s.next().transpose()?;

        assert!(!matches!(s.next(), Some(Ok(_))));

        assert_eq!(rounded(curve1), Ok(Some([45.0, 16.0, 84.0, 10.0, 117.0, 27.0])));
        assert_eq!(rounded(curve2), Ok(Some([150.0, 43.0, 165.0, 76.0, 150.0, 100.0])));

        Ok(())
    }
}
