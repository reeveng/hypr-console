use core::ops::Range;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineEnding {
  #[default]
  Lf,

  CrLf,

  Cr,

  LfCr,

  None,
}

impl LineEnding {
  pub const fn as_str(&self) -> Result<&'static str, console_core_never::Never> {
    {
      Ok(match self {
        Self::Lf => "\n",
        Self::CrLf => "\r\n",
        Self::Cr => "\r",
        Self::LfCr => "\n\r",
        Self::None => "",
      })
    }
  }
}

#[derive(Debug)]
pub struct LineIter<'a> {
  string: &'a str,
  start: u32,
  end: u32,
}

impl<'a> LineIter<'a> {
  pub fn new(string: &'a str) -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        string,
        start: 0,
        end: (match console_core_number_conversion::fitted::<_, u32>(string.len()) {
          Ok(value) => value,
        }),
      })
    }
  }
}

impl Iterator for LineIter<'_> {
  type Item = (Range<u32>, LineEnding);

  fn next(&mut self) -> Option<Self::Item> {
    let start = self.start;
    let Ok(first) = console_core_number_conversion::index(start);
    let Ok(last) = console_core_number_conversion::index(self.end);
    let remaining = self.string.get(first..last)?;
    let ::core::result::Result::Ok(position) =
      ::console_core_number_conversion::fixed_index(match remaining.find(['\r', '\n']) {
        Some(found) => found,
        None => {
          return match (start < self.end).then_some(()) {
            Some(()) => {
              self.start = self.end;
              Some((start..self.end, LineEnding::None))
            }
            None => None,
          };
        }
      });

    let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(
      match ::console_core_number_conversion::index(position) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );
    let end = start.saturating_add(offset);
    self.start = end;
    let Ok(end_index) = console_core_number_conversion::index(end);
    let after = self.string.get(end_index..)?;
    let (ending, length) = match after.as_bytes() {
      [b'\r', b'\n', ..] => (LineEnding::CrLf, 2),
      [b'\n', b'\r', ..] => (LineEnding::LfCr, 2),
      [b'\n', ..] => (LineEnding::Lf, 1),
      [b'\r', ..] => (LineEnding::Cr, 1),
      [..] => (LineEnding::None, 0),
    };
    self.start = end.saturating_add(length);

    Some((start..end, ending))
  }
}

#[test]
fn test_line_iter() {
  let string = "LF\nCRLF\r\nCR\rLFCR\n\rNONE";
  let Ok(mut iter) = LineIter::new(string);
  assert_eq!(iter.next(), Some((0..2, LineEnding::Lf)));
  assert_eq!(iter.next(), Some((3..7, LineEnding::CrLf)));
  assert_eq!(iter.next(), Some((9..11, LineEnding::Cr)));
  assert_eq!(iter.next(), Some((12..16, LineEnding::LfCr)));
  assert_eq!(iter.next(), Some((18..22, LineEnding::None)));
}
