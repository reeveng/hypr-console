use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::ops::ControlFlow;

#[derive(Copy, Clone)]
pub(crate) enum Newline {
  Space,
  Significant,
}

pub(crate) fn consume_whitespace(pos: &mut u32, c: &[u8], newline: Newline) -> Result<(), Never> {
  let Ok(from) = index(*pos);
  let rest = match c.get(from..) {
    Some(rest) => rest,
    None => {
      return Ok(());
    }
  };
  let walked = rest.iter().try_fold(rest, |remaining, _one_byte_at_least| {
    let Ok(stepped) = after_one_space(remaining, newline);

    match stepped {
      Some(after) => ControlFlow::Continue(after),
      None => ControlFlow::Break(remaining),
    }
  });
  let left = match walked {
    ControlFlow::Break(left) | ControlFlow::Continue(left) => left,
  };
  let Ok(reached) = fitted::<_, u32>(c.len().saturating_sub(left.len()));
  *pos = reached;

  Ok(())
}

fn after_one_space(bytes: &[u8], newline: Newline) -> Result<Option<&[u8]>, Never> {
  Ok(match bytes {
    [0x20 | 0x09, rest @ ..] => Some(rest),
    [0x0a, rest @ ..] => match newline {
      Newline::Space => Some(rest),
      Newline::Significant => None,
    },
    [0x0b..=0x0d, rest @ ..] => Some(rest),
    [0xc2, 0x85, rest @ ..] => Some(rest),
    [0xe2, 0x80, 0x8e | 0x8f | 0xa8 | 0xa9, rest @ ..] => Some(rest),
    _ => None,
  })
}
