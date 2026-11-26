use std::hint::black_box;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, wrapped};
use console_core_shapes::{Picture, Pixels, Shape};
use console_draw_painting::{Cannot, Frame, onto, over_picture};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

const VIEWPORT: Size<u32> = Size {
  width: 1280,
  height: 800,
};
const DEVICE_1_5X: Size<u32> = Size {
  width: 1920,
  height: 1200,
};
const DEVICE_2_5X: Size<u32> = Size {
  width: 3200,
  height: 2000,
};

enum PictureFormat {
  Opaque,
  TranslucentPadded,
}

fn picture_pixels(format: PictureFormat) -> Result<Pixels, Never> {
  let (alpha, padding): (u8, &[u8]) = match format {
    PictureFormat::Opaque => (255, &[]),
    PictureFormat::TranslucentPadded => (128, &[0; 64]),
  };
  let mut bytes = Vec::new();

  for row in 0..VIEWPORT.height {
    for column in 0..VIEWPORT.width {
      let Ok(red) = wrapped(column.wrapping_rem(256));
      let Ok(green) = wrapped(row.wrapping_rem(256));
      bytes.extend_from_slice(&[red, green, 97, alpha]);
    }

    bytes.extend_from_slice(padding);
  }

  let Ok(padding_width) = fitted::<_, u32>(padding.len());

  Ok(Pixels {
    width: VIEWPORT.width,
    height: VIEWPORT.height,
    stride: VIEWPORT
      .width
      .saturating_mul(4)
      .saturating_add(padding_width),
    bytes,
  })
}

#[derive(Clone, Copy)]
enum Layers<'a> {
  Opaque(&'a Pixels),
  Translucent(&'a [Shape]),
}

impl Layers<'_> {
  fn paint(self, surface: &mut [u8], frame: Frame) -> Result<(), Cannot> {
    match self {
      Self::Opaque(pixels) => {
        let Ok(picture) = Picture::rectangular(Point::default(), VIEWPORT, black_box(pixels));
        over_picture(black_box(surface), frame, picture, &[])
      }
      Self::Translucent(shapes) => onto(black_box(surface), frame, black_box(shapes)),
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "Criterion invokes each benchmark group through a callback that returns unit"
  )
)]
fn picture_painting(criterion: &mut Criterion) {
  let Ok(opaque) = picture_pixels(PictureFormat::Opaque);
  let Ok(translucent) = picture_pixels(PictureFormat::TranslucentPadded);
  let Ok(picture) = Picture::rectangular(Point::default(), VIEWPORT, translucent);
  let shapes = [Shape::Picture(picture)];
  let mut group = criterion.benchmark_group("picture_painting");

  for (name, device, layers) in [
    ("opaque_1x", VIEWPORT, Layers::Opaque(&opaque)),
    ("opaque_1_5x", DEVICE_1_5X, Layers::Opaque(&opaque)),
    (
      "translucent_padded_1x",
      VIEWPORT,
      Layers::Translucent(&shapes),
    ),
    (
      "translucent_padded_1_5x",
      DEVICE_1_5X,
      Layers::Translucent(&shapes),
    ),
    (
      "translucent_padded_2_5x",
      DEVICE_2_5X,
      Layers::Translucent(&shapes),
    ),
  ] {
    let frame = Frame {
      device,
      points: VIEWPORT,
    };
    let pixel_count = u64::from(device.width).saturating_mul(u64::from(device.height));
    let Ok(byte_count) = index(pixel_count.saturating_mul(4));
    let empty = || vec![0; byte_count.min(25_600_000)];
    let prepared = layers.paint(&mut empty(), frame);
    assert!(
      matches!(prepared, Ok(())),
      "{name}: failed to paint the test frame"
    );
    group.throughput(Throughput::Elements(pixel_count));
    group.bench_function(name, |bencher| {
      bencher.iter_batched_ref(
        empty,
        |destination| layers.paint(destination, frame),
        BatchSize::PerIteration,
      );
    });
  }

  group.finish();
}

criterion_group!(benches, picture_painting);
criterion_main!(benches);
