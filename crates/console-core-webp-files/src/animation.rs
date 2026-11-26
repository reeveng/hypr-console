//! Incremental WebP playback on an opaque, application-provided background.
//! The decoder keeps one canvas and reads only the frame that is due.

use std::cmp::Ordering;
use std::time::Duration;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::{
  Found, Picture, WebpError, affordable, container, filled, lossless, lossy, rounds, whole,
};

#[derive(Debug)]
pub enum Playback {
  Still(Picture),
  Animated(Animation),
}

#[derive(Debug)]
pub struct Animation {
  file: Vec<u8>,
  canvas: Picture,
  ground: [u8; 4],
  first: u32,
  next: u32,
  remaining: Repeats,
  dispose: Option<Rectangle<u32>>,
}

#[derive(Debug)]
enum Repeats {
  Forever,
  Finite(u16),
}

#[derive(Clone, Copy)]
enum Blend {
  Over,
  Replace,
}

impl Playback {
  pub fn open(file: Vec<u8>, ground: [u8; 3]) -> Result<Self, WebpError> {
    let [red, green, blue] = ground;
    let opaque = [red, green, blue, 255];
    let header = animation_header(&file)?;

    let (first, loops) = match header {
      Some(animation) => animation,
      None => {
        let mut still = crate::decoded(&file)?;
        let Ok(()) = flatten(&mut still, opaque);
        return Ok(Self::Still(still));
      }
    };
    let size = container::measured(&file)?;
    affordable((size, size))?;
    let pixels = u64::from(size.width).saturating_mul(u64::from(size.height));
    let bytes = filled(pixels.saturating_mul(4), 0)?;
    let mut animation = Animation {
      file,
      canvas: Picture { size, bytes },
      ground: opaque,
      first,
      next: first,
      remaining: match loops {
        0 => Repeats::Forever,
        total => Repeats::Finite(total.saturating_sub(1)),
      },
      dispose: None,
    };
    let Ok(()) = animation.clear();

    Ok(Self::Animated(animation))
  }
}

fn animation_header(bytes: &[u8]) -> Result<Option<(u32, u16)>, WebpError> {
  let body = container::riff(bytes)?;
  let first = container::chunk(body, 0)?;

  match (first.kind, first.data.first().map(|flags| flags & 2)) {
    ([b'V', b'P', b'8', b'X'], Some(2)) => {}
    _ => return Ok(None),
  }

  let animation = find_chunk(body, first.next, *b"ANIM")?;
  let header = animation.ok_or(WebpError::Corrupt)?;
  let [_, _, _, _, low, high] = header.data.first_chunk::<6>().ok_or(WebpError::Truncated)?;
  Ok(Some((header.next, u16::from_le_bytes([*low, *high]))))
}

fn find_chunk(
  bytes: &[u8],
  start: u32,
  wanted: [u8; 4],
) -> Result<Option<container::Chunk<'_>>, WebpError> {
  let mut at = start;

  for _ in bytes.chunks(8) {
    let Ok(offset) = index(at);

    match offset.cmp(&bytes.len()) {
      std::cmp::Ordering::Equal => return Ok(None),
      std::cmp::Ordering::Greater => return Err(WebpError::Truncated),
      std::cmp::Ordering::Less => {}
    }

    let found = container::chunk(bytes, at)?;

    match found.kind.cmp(&wanted) {
      Ordering::Equal => return Ok(Some(found)),
      Ordering::Less | Ordering::Greater => at = found.next,
    }
  }

  let Ok(end) = index(at);

  match end.cmp(&bytes.len()) {
    Ordering::Equal => Ok(None),
    Ordering::Less | Ordering::Greater => Err(WebpError::Truncated),
  }
}

impl Animation {
  pub fn canvas(&self) -> Result<&Picture, Never> {
    Ok(&self.canvas)
  }

  fn clear(&mut self) -> Result<(), Never> {
    for pixel in self.canvas.bytes.as_chunks_mut::<4>().0 {
      *pixel = self.ground;
    }

    self.dispose = None;
    Ok(())
  }

  fn next_frame(&self) -> Result<Option<(container::Chunk<'_>, Cycle)>, WebpError> {
    let body = container::riff(&self.file)?;
    let frame = find_chunk(body, self.next, *b"ANMF")?;

    match frame {
      Some(following) => Ok(Some((following, Cycle::Continues))),
      None => match (&self.remaining, self.next.cmp(&self.first)) {
        (_, Ordering::Equal) => Err(WebpError::Corrupt),
        (Repeats::Finite(0), Ordering::Less | Ordering::Greater) => Ok(None),
        (Repeats::Finite(1..) | Repeats::Forever, Ordering::Less | Ordering::Greater) => {
          let first = find_chunk(body, self.first, *b"ANMF")?;
          let chunk = first.ok_or(WebpError::Corrupt)?;
          Ok(Some((chunk, Cycle::Restarts)))
        }
      },
    }
  }

  pub fn advance(&mut self) -> Result<Option<Duration>, WebpError> {
    let frame = self.next_frame()?;
    let (chunk, cycle) = match frame {
      Some(taken) => taken,
      None => return Ok(None),
    };
    let [low, middle, high, flags] = chunk
      .data
      .get(12..16)
      .and_then(<[u8]>::first_chunk)
      .copied()
      .ok_or(WebpError::Truncated)?;
    let Ok(duration) = console_core_bytes::endian::u24_le([low, middle, high]);
    let image = container::frame(chunk.data)?;
    let region = Rectangle {
      origin: image.place,
      size: image.size,
    };
    check_region(region, self.canvas.size)?;
    let encoded = match image.coding {
      container::Coding::Lossy => lossy::measured(image.bitstream),
      container::Coding::Lossless => lossless::header(image.bitstream),
    };
    let encoded_size = encoded?;
    image.check_size(encoded_size)?;

    let local = container::Image {
      place: Point { x: 0, y: 0 },
      ..image
    };
    let decoded = whole(
      &Found {
        canvas: image.size,
        image: local,
      },
      &rounds::one_after_another,
    )?;
    let next = chunk.next;

    match cycle {
      Cycle::Restarts => {
        match &mut self.remaining {
          Repeats::Finite(left) => *left = left.saturating_sub(1),
          Repeats::Forever => {}
        }

        let Ok(()) = self.clear();
      }
      Cycle::Continues => match self.dispose.take() {
        Some(prior) => {
          let Ok(()) = dispose(&mut self.canvas, prior, self.ground);
        }
        None => {}
      },
    }

    let blend = match flags & 2 {
      0 => Blend::Over,
      _ => Blend::Replace,
    };
    let Ok(()) = composite(&mut self.canvas, region, &decoded, (blend, self.ground));
    self.dispose = match flags & 1 {
      0 => None,
      _ => Some(region),
    };
    self.next = next;
    Ok(Some(Duration::from_millis(u64::from(duration.max(10)))))
  }
}

enum Cycle {
  Continues,
  Restarts,
}

fn check_region(region: Rectangle<u32>, canvas: Size<u32>) -> Result<(), WebpError> {
  match (
    region.origin.x.saturating_add(region.size.width) <= canvas.width,
    region.origin.y.saturating_add(region.size.height) <= canvas.height,
  ) {
    (true, true) => Ok(()),
    _ => Err(WebpError::Corrupt),
  }
}

fn rows(
  canvas: &mut Picture,
  region: Rectangle<u32>,
) -> Result<impl Iterator<Item = &mut [[u8; 4]]>, Never> {
  let Ok(stride) = index(canvas.size.width);
  let Ok(left) = index(region.origin.x);
  let Ok(top) = index(region.origin.y);
  let Ok(width) = index(region.size.width);
  let Ok(height) = index(region.size.height);
  Ok(
    canvas
      .bytes
      .as_chunks_mut::<4>()
      .0
      .chunks_exact_mut(stride.max(1))
      .skip(top)
      .take(height)
      .filter_map(move |row| row.get_mut(left..left.saturating_add(width))),
  )
}

fn dispose(canvas: &mut Picture, region: Rectangle<u32>, ground: [u8; 4]) -> Result<(), Never> {
  let Ok(inside) = rows(canvas, region);

  for row in inside {
    row.fill(ground);
  }

  Ok(())
}

fn composite(
  canvas: &mut Picture,
  region: Rectangle<u32>,
  decoded: &[u8],
  treatment: (Blend, [u8; 4]),
) -> Result<(), Never> {
  let (blend, ground) = treatment;
  let Ok(width) = index(region.size.width);
  let Ok(inside) = rows(canvas, region);
  let source = decoded.as_chunks::<4>().0.chunks_exact(width.max(1));

  for (into, from) in inside.zip(source) {
    for (pixel, foreground) in into.iter_mut().zip(from) {
      let underneath = match blend {
        Blend::Over => *pixel,
        Blend::Replace => ground,
      };
      let Ok(composited) = over((*foreground, underneath));
      *pixel = composited;
    }
  }

  Ok(())
}

fn flatten(picture: &mut Picture, ground: [u8; 4]) -> Result<(), Never> {
  for pixel in picture.bytes.as_chunks_mut::<4>().0 {
    let Ok(composited) = over((*pixel, ground));
    *pixel = composited;
  }

  Ok(())
}

fn over(layers: ([u8; 4], [u8; 4])) -> Result<[u8; 4], Never> {
  let (source, under) = layers;
  let [red, green, blue, alpha] = source;

  match alpha {
    255 => return Ok(source),
    0 => return Ok(under),
    1..=254 => {}
  }

  let weight = u32::from(alpha);
  let behind = 255u32.saturating_sub(weight);
  let mix = |foreground: u8, background: u8| {
    let above = u32::from(foreground).saturating_mul(weight);
    let below = u32::from(background).saturating_mul(behind);
    let weighted = above.saturating_add(below).saturating_add(127);
    let Ok(channel) =
      console_core_number_conversion::fitted::<_, u8>(weighted.saturating_div(255));
    channel
  };
  let [under_red, under_green, under_blue, _] = under;

  Ok([
    mix(red, under_red),
    mix(green, under_green),
    mix(blue, under_blue),
    255,
  ])
}

#[cfg(test)]
mod tests {
  use super::*;

  const FIRST_LOOP_BYTE: u32 = 42;
  const SECOND_FRAME_CODEC_TAG: u32 = 2912;

  fn animation(bytes: Vec<u8>) -> Result<Animation, WebpError> {
    let playback = Playback::open(bytes, [17, 11, 18])?;

    match playback {
      Playback::Animated(frames) => Ok(frames),
      Playback::Still(_) => Err(WebpError::Corrupt),
    }
  }

  #[test]
  fn a_finite_loop_leaves_the_last_composited_frame_in_place() -> Result<(), WebpError> {
    let mut bytes = include_bytes!("../tests/pictures/animated.webp").to_vec();
    let Ok(offset) = index(FIRST_LOOP_BYTE);
    let loops = bytes.get_mut(offset).ok_or(WebpError::Truncated)?;
    *loops = 1;

    let mut frames = animation(bytes)?;
    let duration = Some(Duration::from_millis(100));
    let first_timing = frames.advance()?;
    assert_eq!(first_timing, duration);
    let Ok(first_canvas) = frames.canvas();
    let first = first_canvas.bytes.clone();

    let last_timing = frames.advance()?;
    assert_eq!(last_timing, duration);
    let Ok(last_canvas) = frames.canvas();
    let last = last_canvas.bytes.clone();
    assert_ne!(first, last);

    let ended = frames.advance()?;
    assert_eq!(ended, None);
    let Ok(held) = frames.canvas();
    assert_eq!(held.bytes, last);
    Ok(())
  }

  #[test]
  fn a_cut_off_frame_preserves_the_last_good_canvas() -> Result<(), WebpError> {
    let mut bytes = include_bytes!("../tests/pictures/animated.webp").to_vec();
    let Ok(offset) = index(SECOND_FRAME_CODEC_TAG);
    let discard = bytes.get_mut(offset).ok_or(WebpError::Truncated)?;
    *discard = b'!';

    let mut frames = animation(bytes)?;
    let duration = frames.advance()?;
    assert_eq!(duration, Some(Duration::from_millis(100)));
    let Ok(canvas) = frames.canvas();
    let previous = canvas.bytes.clone();
    assert_eq!(frames.advance(), Err(WebpError::Corrupt));
    let Ok(held) = frames.canvas();
    assert_eq!(held.bytes, previous);
    Ok(())
  }
}
