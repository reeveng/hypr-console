//! Every pair of sample formats converted over a fixed set of inputs, and what
//! came out folded into one number per pair. The numbers were taken from the
//! converters as they were before they were rewritten under the workspace's
//! rules, so a rewrite that moves one sample of one pair is red here. The one
//! exception is a double narrowed to a single, which is exercised over finite
//! values only: a NaN or an infinity there now comes out as zero or the largest
//! single, where it used to carry through.

use console_core_audio_samples::{FromSample, Sample, i24, u24};
use console_core_never::Never;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const EDGES: [u32; 29] = [
  0,
  1,
  2,
  0x7f,
  0x80,
  0x81,
  0xff,
  0x100,
  0x7fff,
  0x8000,
  0x8001,
  0xffff,
  0x1_0000,
  0x7f_ffff,
  0x80_0000,
  0x80_0001,
  0xff_ffff,
  0x100_0000,
  0x100_0001,
  0x7fff_ffff,
  0x8000_0000,
  0x8000_0001,
  0xffff_fffe,
  0xffff_ffff,
  0xff00_0000,
  0x00ff_ffff,
  0xff80_0000,
  0xff7f_ffff,
  0xfeff_ffff,
];

const FLOATS: [f64; 27] = [
  0.0,
  -0.0,
  1.0,
  -1.0,
  0.5,
  -0.5,
  0.25,
  1.000_000_1,
  -1.000_000_1,
  1.5,
  -1.5,
  2.0,
  -2.0,
  1e10,
  -1e10,
  1e-10,
  -1e-10,
  f64::MIN_POSITIVE,
  f64::NAN,
  f64::INFINITY,
  f64::NEG_INFINITY,
  0.999_999_9,
  -0.999_999_9,
  127.0 / 128.0,
  32_767.0 / 32_768.0,
  8_388_607.0 / 8_388_608.0,
  0.003_906_25,
];

trait Bits {
  fn bits(self) -> u64;
}

impl Bits for u8 {
  fn bits(self) -> u64 {
    u64::from(self)
  }
}

impl Bits for i8 {
  fn bits(self) -> u64 {
    u64::from(self.cast_unsigned())
  }
}

impl Bits for u16 {
  fn bits(self) -> u64 {
    u64::from(self)
  }
}

impl Bits for i16 {
  fn bits(self) -> u64 {
    u64::from(self.cast_unsigned())
  }
}

impl Bits for u24 {
  fn bits(self) -> u64 {
    u64::from(self.0)
  }
}

impl Bits for i24 {
  fn bits(self) -> u64 {
    u64::from(self.0.cast_unsigned())
  }
}

impl Bits for u32 {
  fn bits(self) -> u64 {
    u64::from(self)
  }
}

impl Bits for i32 {
  fn bits(self) -> u64 {
    u64::from(self.cast_unsigned())
  }
}

impl Bits for f32 {
  fn bits(self) -> u64 {
    u64::from(self.to_bits())
  }
}

impl Bits for f64 {
  fn bits(self) -> u64 {
    self.to_bits()
  }
}

fn folded<T: Bits>(outputs: impl Iterator<Item = T>) -> Result<u64, Never> {
  Ok(outputs.fold(FNV_OFFSET, |hash, output| {
    (hash ^ output.bits()).wrapping_mul(FNV_PRIME)
  }))
}

fn digest<F: Copy, T: FromSample<F> + Bits>(inputs: &[F]) -> Result<u64, Never> {
  folded(inputs.iter().map(|input| T::from_sample(*input)))
}

fn clamped_digest<S: Sample + Bits>(inputs: &[S]) -> Result<u64, Never> {
  folded(inputs.iter().map(|input| input.clamped()))
}

fn stream(seed: u64, count: u32) -> Result<Vec<u64>, Never> {
  Ok(
    (0..count)
      .scan(seed, |state, _| {
        *state = state
          .wrapping_mul(6_364_136_223_846_793_005)
          .wrapping_add(1_442_695_040_888_963_407);

        Some(state.wrapping_shr(16))
      })
      .collect(),
  )
}

fn words(seed: u64, count: u32) -> Result<Vec<u32>, Never> {
  let Ok(states) = stream(seed, count);

  Ok(
    states
      .into_iter()
      .map(|state| {
        let [_, _, _, _, a, b, c, d] = state.to_be_bytes();

        u32::from_be_bytes([a, b, c, d])
      })
      .collect(),
  )
}

fn unit_floats(seed: u64, count: u32) -> Result<Vec<f64>, Never> {
  let Ok(states) = stream(seed, count);

  Ok(
    states
      .into_iter()
      .map(|state| {
        let [_, _, _, _, _, a, b, c] = state.to_be_bytes();

        (f64::from(u32::from_be_bytes([0, a, b, c])) / 8_388_608.0 - 1.0) * 1.5
      })
      .collect(),
  )
}

struct Inputs {
  u8s: Vec<u8>,
  i8s: Vec<i8>,
  u16s: Vec<u16>,
  i16s: Vec<i16>,
  u24s: Vec<u24>,
  i24s: Vec<i24>,
  u32s: Vec<u32>,
  i32s: Vec<i32>,
  f32s: Vec<f32>,
  f64s: Vec<f64>,
  finite_f64s: Vec<f64>,
}

fn inputs() -> Result<Inputs, Never> {
  let Ok(random) = words(7, 6000);
  let Ok(unit) = unit_floats(11, 6000);
  let wide: Vec<u32> = EDGES.into_iter().chain(random).collect();
  let mut floats: Vec<f64> = FLOATS.to_vec();

  floats.extend((0..=4096u32).map(|step| f64::from(step) / 4096.0 - 0.5));
  floats.extend((0..=512u32).flat_map(|step| {
    [
      (f64::from(step) + 0.5) / 128.0 - 2.0,
      (f64::from(step) + 0.5) / 32_768.0 - 1.0,
    ]
  }));
  floats.extend(unit);

  let f32s = floats
    .iter()
    .map(|value| match value.classify() {
      core::num::FpCategory::Nan => f32::NAN,
      core::num::FpCategory::Infinite => {
        f32::from_bits(0x7f80_0000 | u32::from(*value < 0.0).wrapping_shl(31))
      }
      core::num::FpCategory::Zero
      | core::num::FpCategory::Subnormal
      | core::num::FpCategory::Normal => f32::from_sample(*value),
    })
    .collect();

  Ok(Inputs {
    u8s: (0..=u8::MAX).collect(),
    i8s: (i8::MIN..=i8::MAX).collect(),
    u16s: (0..=u16::MAX).collect(),
    i16s: (i16::MIN..=i16::MAX).collect(),
    u24s: wide.iter().map(|word| u24(*word & 0x1ff_ffff)).collect(),
    i24s: wide
      .iter()
      .map(|word| i24(word.cast_signed().wrapping_shr(7)))
      .collect(),
    u32s: wide.clone(),
    i32s: wide.iter().map(|word| word.cast_signed()).collect(),
    f32s,
    finite_f64s: floats
      .iter()
      .copied()
      .filter(|value| value.is_finite())
      .collect(),
    f64s: floats,
  })
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the conversion matrix uses the same destination type for the generic call and its golden row label, keeping the names tied to the types under test"
  )
)]
macro_rules! rows_into {
  ($rows:ident, $inputs:ident, $to:ty) => {
    let Ok(hash) = digest::<u8, $to>(&$inputs.u8s);
    $rows.push((concat!("u8->", stringify!($to)), hash));
    let Ok(hash) = digest::<i8, $to>(&$inputs.i8s);
    $rows.push((concat!("i8->", stringify!($to)), hash));
    let Ok(hash) = digest::<u16, $to>(&$inputs.u16s);
    $rows.push((concat!("u16->", stringify!($to)), hash));
    let Ok(hash) = digest::<i16, $to>(&$inputs.i16s);
    $rows.push((concat!("i16->", stringify!($to)), hash));
    let Ok(hash) = digest::<u24, $to>(&$inputs.u24s);
    $rows.push((concat!("u24->", stringify!($to)), hash));
    let Ok(hash) = digest::<i24, $to>(&$inputs.i24s);
    $rows.push((concat!("i24->", stringify!($to)), hash));
    let Ok(hash) = digest::<u32, $to>(&$inputs.u32s);
    $rows.push((concat!("u32->", stringify!($to)), hash));
    let Ok(hash) = digest::<i32, $to>(&$inputs.i32s);
    $rows.push((concat!("i32->", stringify!($to)), hash));
    let Ok(hash) = digest::<f32, $to>(&$inputs.f32s);
    $rows.push((concat!("f32->", stringify!($to)), hash));
    let Ok(hash) = digest::<f64, $to>(&$inputs.f64s);
    $rows.push((concat!("f64->", stringify!($to)), hash));
  };
}

fn rows(inputs: &Inputs) -> Result<Vec<(&'static str, u64)>, Never> {
  let mut rows = Vec::new();

  rows_into!(rows, inputs, u8);
  rows_into!(rows, inputs, i8);
  rows_into!(rows, inputs, u16);
  rows_into!(rows, inputs, i16);
  rows_into!(rows, inputs, u24);
  rows_into!(rows, inputs, i24);
  rows_into!(rows, inputs, u32);
  rows_into!(rows, inputs, i32);
  rows_into!(rows, inputs, f32);
  rows_into!(rows, inputs, f64);

  rows.retain(|(name, _)| *name != "f64->f32");
  let Ok(hash) = digest::<f64, f32>(&inputs.finite_f64s);
  rows.push(("f64->f32 finite", hash));
  let Ok(hash_2) = clamped_digest(&inputs.u24s);
  rows.push(("clamped u24", hash_2));
  let Ok(hash_3) = clamped_digest(&inputs.i24s);
  rows.push(("clamped i24", hash_3));
  let Ok(hash_4) = clamped_digest(&inputs.f32s);
  rows.push(("clamped f32", hash_4));
  let Ok(hash_5) = clamped_digest(&inputs.f64s);
  rows.push(("clamped f64", hash_5));
  let Ok(hash_6) = folded(inputs.i32s.iter().map(|word| i24::from(*word)));
  rows.push(("i24 from i32", hash_6));
  let Ok(hash_7) = folded(inputs.u32s.iter().map(|word| u24::from(*word)));
  rows.push(("u24 from u32", hash_7));

  Ok(rows)
}

#[test]
fn every_pair_converts_as_it_did_before_the_rewrite() {
  let Ok(sampled) = inputs();
  let Ok(found) = rows(&sampled);
  let wrong: Vec<&str> = found
    .iter()
    .filter(|(name, digest)| {
      GOLDEN
        .iter()
        .find(|(wanted, _)| wanted == name)
        .map(|(_, expected)| expected)
        != Some(digest)
    })
    .map(|(name, _)| *name)
    .collect();

  assert_eq!(wrong, Vec::<&str>::new(), "{found:#x?}");
  assert_eq!(found.len(), GOLDEN.len());
}

const GOLDEN: &[(&str, u64)] = &[
  ("u8->u8", 0x4242dc5249c33625),
  ("i8->u8", 0x4242dc5249c33625),
  ("u16->u8", 0x9e731e58b77ca325),
  ("i16->u8", 0x9e731e58b77ca325),
  ("u24->u8", 0x76de50d09cdd54f6),
  ("i24->u8", 0x4f516b0ef208053b),
  ("u32->u8", 0x09f18221d9c459a1),
  ("i32->u8", 0xfb217cbcd5dff721),
  ("f32->u8", 0xc8a011de0d762bf9),
  ("f64->u8", 0xb9728ac1d38613ca),
  ("u8->i8", 0xe6169983deb2d625),
  ("i8->i8", 0xe6169983deb2d625),
  ("u16->i8", 0xc5bbf7d0b77ca325),
  ("i16->i8", 0xc5bbf7d0b77ca325),
  ("u24->i8", 0x0d926253dc5bc676),
  ("i24->i8", 0xee289603c16f20bb),
  ("u32->i8", 0xfb217cbcd5dff721),
  ("i32->i8", 0x09f18221d9c459a1),
  ("f32->i8", 0x5540755c566cb292),
  ("f64->i8", 0x5540755c566cb292),
  ("u8->u16", 0x0e2af0024ad34f25),
  ("i8->u16", 0x0e2af0024ad34f25),
  ("u16->u16", 0xbf31ec86759b2325),
  ("i16->u16", 0xbf31ec86759b2325),
  ("u24->u16", 0x4fc5ccd4714c3292),
  ("i24->u16", 0x01203283812cf565),
  ("u32->u16", 0x9ac98464ffe3d69b),
  ("i32->u16", 0x740404accfdc569b),
  ("f32->u16", 0x29cdc5e4fa400c67),
  ("f64->u16", 0x9034fcdfaf004bcf),
  ("u8->i16", 0x26b98a6bfc6f4f25),
  ("i8->i16", 0x26b98a6bfc6f4f25),
  ("u16->i16", 0x270b3ea41b2b2325),
  ("i16->i16", 0x270b3ea41b2b2325),
  ("u24->i16", 0xa8a0f5343f40b292),
  ("i24->i16", 0x07c002177cd57565),
  ("u32->i16", 0x740404accfdc569b),
  ("i32->i16", 0x9ac98464ffe3d69b),
  ("f32->i16", 0x7d048cc4ddd8a742),
  ("f64->i16", 0x7d048cc4ddd8a742),
  ("u8->u24", 0xbe2f122330f3b725),
  ("i8->u24", 0xbe2f122330f3b725),
  ("u16->u24", 0xc4c88ec3832e2325),
  ("i16->u24", 0xc4c88ec3832e2325),
  ("u24->u24", 0x29e7dbe2e8d17780),
  ("i24->u24", 0xdc13393cec92c98b),
  ("u32->u24", 0xc85f08765fdb6f30),
  ("i32->u24", 0xac1c3b14505b6f30),
  ("f32->u24", 0x23cf5b9f999cbbde),
  ("f64->u24", 0xd98767268ef23ed8),
  ("u8->i24", 0x5094f1ef58f3b725),
  ("i8->i24", 0x5094f1ef58f3b725),
  ("u16->i24", 0xfda23ae0892e2325),
  ("i16->i24", 0xfda23ae0892e2325),
  ("u24->i24", 0xbf6bc6166f1757d6),
  ("i24->i24", 0x3b1f2ac5896db8fe),
  ("u32->i24", 0x4219002c785b6f30),
  ("i32->i24", 0xcc615ed916db6f30),
  ("f32->i24", 0xbb79c9087e508005),
  ("f64->i24", 0xbb79c9087e508005),
  ("u8->u32", 0xc871a366c16bb725),
  ("i8->u32", 0xc871a366c16bb725),
  ("u16->u32", 0x93eccfede36e2325),
  ("i16->u32", 0x93eccfede36e2325),
  ("u24->u32", 0x5224a450c9b8882f),
  ("i24->u32", 0x6999bfb01af27f2f),
  ("u32->u32", 0xf58637bf94d17780),
  ("i32->u32", 0x488ccb1214d17780),
  ("f32->u32", 0x9108e77ae5193a10),
  ("f64->u32", 0x232ad7638f62a176),
  ("u8->i32", 0x8b55a316c16bb725),
  ("i8->i32", 0x8b55a316c16bb725),
  ("u16->i32", 0xae9fe0bce36e2325),
  ("i16->i32", 0xae9fe0bce36e2325),
  ("u24->i32", 0xa784ac7249b8882f),
  ("i24->i32", 0x3fe8707c9af27f2f),
  ("u32->i32", 0x488ccb1214d17780),
  ("i32->i32", 0xf58637bf94d17780),
  ("f32->i32", 0x1bea83758b443d90),
  ("f64->i32", 0x571bfb7c22ec6594),
  ("u8->f32", 0x78acba6307dbb725),
  ("i8->f32", 0x78acba6307dbb725),
  ("u16->f32", 0x1f16350fc1312325),
  ("i16->f32", 0x1f16350fc1312325),
  ("u24->f32", 0x08ead65a498b98bb),
  ("i24->f32", 0x379999c678f2149f),
  ("u32->f32", 0xa6affd43e1a06d14),
  ("i32->f32", 0x57b5b74410070428),
  ("f32->f32", 0x2df7ee2faa8a5a81),
  ("u8->f64", 0x83c7c658736bb725),
  ("i8->f64", 0x83c7c658736bb725),
  ("u16->f64", 0xaae0032ea5b62325),
  ("i16->f64", 0xaae0032ea5b62325),
  ("u24->f64", 0xb7e5495c0840ab2f),
  ("i24->f64", 0x42f86cae8840ab2f),
  ("u32->f64", 0x4c2738648e80ab2f),
  ("i32->f64", 0x2efcf95b82c0ab2f),
  ("f32->f64", 0x1b5304f5caabf0dd),
  ("f64->f64", 0xf90ce6761719c483),
  ("f64->f32 finite", 0x9e171d9e400863ef),
  ("clamped u24", 0x18ede122e99757d6),
  ("clamped i24", 0x3bad2c5ee712c98b),
  ("clamped f32", 0x1428622256f708e7),
  ("clamped f64", 0x1a22f4982f20ab19),
  ("i24 from i32", 0x81b2d1a2da7a5241),
  ("u24 from u32", 0x6a638ab4014e2dea),
];
