#![deny(unsafe_op_in_unsafe_fn)]

use crate::include::common::intops::inv_recenter;
use crate::include::common::intops::ulog2;
use crate::src::c_arc::CArc;
use crate::src::cpu::CpuFlags;
use crate::src::error::{Rav1dError, Rav1dResult};
use console_core_number_conversion::{fixed_index, index};
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ops::Deref;
use std::ops::DerefMut;

pub struct Rav1dMsacDSPContext {}

impl Rav1dMsacDSPContext {
  pub const fn default() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {})
  }

  pub const fn new(_flags: CpuFlags) -> core::result::Result<Self, console_core_never::Never> {
    Self::default()
  }
}

impl Default for Rav1dMsacDSPContext {
  fn default() -> Self {
    let Ok(rav1d_msac_dsp_context) = Self::default();

    rav1d_msac_dsp_context
  }
}

#[cfg(target_pointer_width = "32")]
type EcWin = u32;
#[cfg(target_pointer_width = "64")]
type EcWin = u64;

#[derive(Default)]
struct MsacAsmContextBuf {
  pos: u64,
}

#[derive(Default)]
pub struct MsacAsmContext {
  buf: MsacAsmContextBuf,
  pub dif: EcWin,
  pub rng: c_uint,
  pub cnt: c_int,
  updates: CdfUpdates,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum CdfUpdates {
  Allowed,
  #[default]
  Frozen,
}

#[derive(Default)]
pub struct MsacContext {
  asm: MsacAsmContext,
  data: Option<CArc<[u8]>>,
  failure: Option<Rav1dError>,
}

impl Deref for MsacContext {
  type Target = MsacAsmContext;

  fn deref(&self) -> &Self::Target {
    &self.asm
  }
}

impl DerefMut for MsacContext {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.asm
  }
}

impl MsacContext {
  pub(crate) fn check(&self) -> Rav1dResult {
    match self.failure {
      Some(error) => Err(error),
      None => Ok(()),
    }
  }

  #[inline(always)]
  fn adapt_cdf(&mut self, cdf: &mut [u16], symbol: DecodedSymbol) -> Result<(), console_core_never::Never> {
    match (self.updates, self.failure) {
      (CdfUpdates::Allowed, None) => match update_observations(cdf, symbol) {
        Ok(()) => {}
        Err(error) => self.failure = Some(error),
      },
      (CdfUpdates::Allowed, Some(_latched_error)) => {}
      (CdfUpdates::Frozen, _failure) => {}
    }

    Ok(())
  }

  pub(crate) fn trailing_bytes(&self, displacement: i32) -> Rav1dResult<&[u8]> {
    let storage = self.data.as_deref().ok_or(Rav1dError::EINVAL)?;
    let start = self.buf.pos.checked_add_signed(i64::from(displacement)).ok_or(Rav1dError::EINVAL)?;
    let Ok(position) = index(start);
    storage.get(position..).ok_or(Rav1dError::EINVAL)
  }
}

#[inline]
pub fn rav1d_msac_decode_bools(
  s: &mut MsacContext,
  n: u8,
) -> core::result::Result<c_uint, console_core_never::Never> {
  Ok({
    let mut v = 0;

    for _ in 0..n {
      v = u32::wrapping_shl(v, 1_u32) | u32::from(matches!(rav1d_msac_decode_bool_equi(s), Ok(EntropyBit::One)));
    }

    v
  })
}

#[inline]
pub fn rav1d_msac_decode_uniform(
  s: &mut MsacContext,
  n: c_uint,
) -> core::result::Result<c_int, console_core_never::Never> {
  Ok({
    assert!(n > 0);
    let l = u8::wrapping_add(
      {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(ulog2(n));
        numeric_value
      },
      1,
    );
    assert!(l > 1);
    let m = u32::wrapping_sub(
      u32::wrapping_shl(1, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(l);
        numeric_value
      }),
      n,
    );
    let Ok(v) = rav1d_msac_decode_bools(s, u8::wrapping_sub(l, 1));

    {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(match ::core::primitive::bool::then_some(v < m, ()) {
        ::core::option::Option::Some(()) => v,
        ::core::option::Option::None => u32::wrapping_add(
          u32::wrapping_sub(u32::wrapping_shl(v, 1_u32), m),
          u32::from(matches!(rav1d_msac_decode_bool_equi(s), Ok(EntropyBit::One))),
        ),
      });
      numeric_value
    }
  })
}

const EC_PROB_SHIFT: c_uint = 6;
const EC_MIN_PROB: c_uint = 4;
const _: () = assert!(EC_MIN_PROB <= (1 << EC_PROB_SHIFT) / 16);

const EC_WIN_SIZE: u32 = EcWin::BITS;
const EC_WIN_SHIFT: u32 = EC_WIN_SIZE.wrapping_sub(16);

#[derive(Clone, Copy)]
struct DecodedSymbol {
  threshold_count: u8,
  value: u8,
}

struct CdfUpdate {
  symbol: DecodedSymbol,
  rate: u16,
  observations: u16,
}

#[inline(always)]
fn update_observations(cdf: &mut [u16], symbol: DecodedSymbol) -> Rav1dResult {
  let Ok(counter_position) = index(symbol.threshold_count);
  let observations = ::core::result::Result::<&_, _>::copied(cdf.get(counter_position).ok_or(Rav1dError::EINVAL))?;
  let rate = 4u16.wrapping_add(observations.wrapping_shr(4)).wrapping_add(u16::from(symbol.threshold_count > 2));
  update_cdf(cdf, CdfUpdate { symbol, rate, observations })
}

#[inline(always)]
fn update_cdf(cdf: &mut [u16], update: CdfUpdate) -> Rav1dResult {
  let CdfUpdate { symbol, rate, observations } = update;
  let Ok(counter_position) = index(symbol.threshold_count);
  let table = cdf.get_mut(..=counter_position).ok_or(Rav1dError::EINVAL)?;

  #[cfg(target_arch = "x86_64")]
  {
    use archmage::SimdToken as _;

    match (symbol.threshold_count == 3 && rate < 16).then_some(()) {
      Some(()) => match archmage::X64V1Token::summon() {
        Some(token) => {
          let packed = table.first_chunk_mut::<4>().ok_or(Rav1dError::EINVAL)?;
          let packed_update = CdfUpdate {
            symbol: DecodedSymbol { threshold_count: 3, value: symbol.value.min(3) },
            rate,
            observations,
          };
          let Ok(()) = update_cdf3_simd(token, packed, packed_update);
          return Ok(());
        }
        None => {}
      },
      None => {}
    }
  }

  let (counter, probabilities) = table.split_last_mut().ok_or(Rav1dError::EINVAL)?;
  let shift = u32::from(rate);

  for (probability, index) in probabilities.iter_mut().zip(0u8..) {
    let mask = u16::from(index < symbol.value).wrapping_neg();
    let increase = 32768u16.wrapping_sub(*probability).wrapping_shr(shift);
    let decrease = probability.wrapping_shr(shift);
    *probability = probability.wrapping_add(increase & mask).wrapping_sub(decrease & !mask);
  }

  *counter = observations.wrapping_add(u16::from(observations < 32));
  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[archmage::arcane]
fn update_cdf3_simd(
  _token: archmage::X64V1Token,
  cdf: &mut [u16; 4],
  CdfUpdate { symbol, rate, observations }: CdfUpdate,
) -> Result<(), console_core_never::Never> {
  use core::arch::x86_64::*;

  let decoded = symbol.value;
  let bytes: [u8; 8] = zerocopy::transmute!(*cdf);
  let probabilities = _mm_cvtsi64_si128(i64::from_le_bytes(bytes));
  let below = _mm_cmpgt_epi16(_mm_set1_epi16(i16::from(decoded)), _mm_setr_epi16(0, 1, 2, 3, 4, 5, 6, 7));
  let shift = _mm_cvtsi32_si128(i32::from(rate));
  let up = _mm_srl_epi16(_mm_sub_epi16(_mm_set1_epi16(i16::MIN), probabilities), shift);
  let down = _mm_srl_epi16(probabilities, shift);
  let updated = _mm_sub_epi16(
    _mm_add_epi16(probabilities, _mm_and_si128(below, up)),
    _mm_andnot_si128(below, down),
  );
  let counter = observations.wrapping_add(u16::from(observations < 32));
  let packed = _mm_insert_epi16::<3>(updated, i32::from(counter));
  *cdf = zerocopy::transmute!(_mm_cvtsi128_si64(packed).to_le_bytes());
  Ok(())
}

#[inline]
#[cold]
fn ctx_refill(s: &mut MsacContext) -> Result<(), console_core_never::Never> {
  let Ok(window_bits) = console_core_number_conversion::wrapped::<_, i32>(EC_WIN_SIZE);
  let refill_base = window_bits.wrapping_sub(24);
  let mut shift = refill_base.wrapping_sub(s.cnt);
  let mut difference = s.dif;

  let source = match s.data.as_deref() {
    Some(storage) => storage,
    None => &[],
  };
  let Ok(position) = index(s.buf.pos);
  let mut bytes = match source.get(position..) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => &[],
  };

  #[cfg(target_pointer_width = "64")]
  match refill_word(bytes, shift) {
    Ok(WordRefill::Chunk { word, remaining, next_shift }) => {
      difference |= word;
      bytes = remaining;
      shift = next_shift;
    }
    Ok(WordRefill::Bytewise) => {}
  }

  for _ in 0..EcWin::BITS.wrapping_div(8) {
    let shift_bits = match u32::try_from(shift) {
      Ok(value) => value,
      Err(_window_filled) => break,
    };

    match bytes.split_first() {
      Some((&byte, remaining)) => {
        let byte_bits = EcWin::from(!byte);
        difference |= byte_bits.wrapping_shl(shift_bits);
        bytes = remaining;
        shift = shift.wrapping_sub(8);
      }
      None => {
        let byte_mask = EcWin::from(u8::MAX);
        difference |= !(!byte_mask).wrapping_shl(shift_bits);
        break;
      }
    }
  }

  let Ok(storage_len) = fixed_index(source.len());
  let Ok(remaining_len) = fixed_index(bytes.len());
  s.buf.pos = storage_len.wrapping_sub(remaining_len);
  s.dif = difference;
  s.cnt = refill_base.wrapping_sub(shift);
  Ok(())
}

#[cfg(target_pointer_width = "64")]
enum WordRefill<'a> {
  Chunk { word: EcWin, remaining: &'a [u8], next_shift: i32 },
  Bytewise,
}

#[cfg(target_pointer_width = "64")]
#[inline]
fn refill_word(bytes: &[u8], shift: i32) -> Result<WordRefill<'_>, console_core_never::Never> {
  let shift_bits = match u32::try_from(shift) {
    Ok(bits) => bits,
    Err(_filled_window) => return Ok(WordRefill::Bytewise),
  };
  let word = match bytes.first_chunk::<8>() {
    Some(full_word) => full_word,
    None => return Ok(WordRefill::Bytewise),
  };
  let keep_bits = shift_bits.wrapping_div(8).wrapping_add(1).wrapping_mul(8);
  let mask = match keep_bits {
    64 => u64::MAX,
    _ => u64::MAX.wrapping_shl(64u32.wrapping_sub(keep_bits)),
  };
  let bits = !u64::from_be_bytes(*word) & mask;
  let placement = shift.wrapping_sub(56);
  let placed = match u32::try_from(placement) {
    Ok(left) => bits.wrapping_shl(left),
    Err(_right_shift) => bits.wrapping_shr(placement.unsigned_abs()),
  };
  let Ok(take) = index(keep_bits.wrapping_div(8));
  let remaining = match bytes.get(take..) {
    Some(rest) => rest,
    None => return Ok(WordRefill::Bytewise),
  };
  let Ok(consumed_bits) = console_core_number_conversion::wrapped::<_, i32>(keep_bits);
  Ok(WordRefill::Chunk { word: placed, remaining, next_shift: shift.wrapping_sub(consumed_bits) })
}

#[inline(always)]
#[must_use]
fn in_window(probability: u32) -> Result<EcWin, console_core_never::Never> {
  Ok(EcWin::from(probability).wrapping_shl(EC_WIN_SHIFT))
}

#[inline(always)]
#[must_use]
fn window_prefix(window: EcWin) -> Result<u32, console_core_never::Never> {
  console_core_number_conversion::wrapped::<_, u32>(window.wrapping_shr(EC_WIN_SHIFT))
}

#[inline(always)]
fn ctx_norm(
  s: &mut MsacContext,
  difference: EcWin,
  range: c_uint,
) -> Result<(), console_core_never::Never> {
  let shift = 16 ^ range.leading_zeros();
  let Ok(consumed_bits) = console_core_number_conversion::wrapped::<_, i32>(shift);
  let available = s.cnt;
  s.dif = difference.wrapping_shl(shift);
  s.rng = range.wrapping_shl(shift);
  s.cnt = available.wrapping_sub(consumed_bits);

  match (available.cast_unsigned() < shift).then_some(()) {
    Some(()) => ctx_refill(s),
    None => Ok(()),
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntropyBit {
  Zero,
  One,
}

#[inline(always)]
fn rav1d_msac_decode_bool_equi_rust(s: &mut MsacContext) -> Result<EntropyBit, console_core_never::Never> {
  let lower = s.rng.wrapping_shr(8).wrapping_shl(7).wrapping_add(EC_MIN_PROB);
  decode_binary(s, lower)
}

#[inline(always)]
fn rav1d_msac_decode_bool_rust(s: &mut MsacContext, probability: c_uint) -> Result<EntropyBit, console_core_never::Never> {
  let lower = s.rng.wrapping_shr(8)
    .wrapping_mul(probability.wrapping_shr(EC_PROB_SHIFT))
    .wrapping_shr(7u32.wrapping_sub(EC_PROB_SHIFT))
    .wrapping_add(EC_MIN_PROB);
  decode_binary(s, lower)
}

#[inline(always)]
fn decode_binary(s: &mut MsacContext, lower: u32) -> Result<EntropyBit, console_core_never::Never> {
  let Ok(boundary) = in_window(lower);
  let (difference, range, bit) = match s.dif.cmp(&boundary) {
    core::cmp::Ordering::Less => (s.dif, lower, EntropyBit::One),
    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
      (s.dif.wrapping_sub(boundary), s.rng.wrapping_sub(lower), EntropyBit::Zero)
    }
  };
  let Ok(()) = ctx_norm(s, difference, range);
  Ok(bit)
}

pub(crate) struct SubexpParameters {
  pub reference: u32,
  pub order: u8,
}

pub(crate) fn rav1d_msac_decode_subexp(
  s: &mut MsacContext,
  SubexpParameters { reference, order }: SubexpParameters,
) -> Result<c_int, console_core_never::Never> {
  let upper_bound = 8_u32.wrapping_shl(u32::from(order));
  let (bits, base) = match rav1d_msac_decode_bool_equi(s) {
    Ok(EntropyBit::Zero) => (order, 0),
    Ok(EntropyBit::One) => {
      let bits = match rav1d_msac_decode_bool_equi(s) {
        Ok(EntropyBit::Zero) => order,
        Ok(EntropyBit::One) => match rav1d_msac_decode_bool_equi(s) {
          Ok(EntropyBit::Zero) => order.wrapping_add(1),
          Ok(EntropyBit::One) => order.wrapping_add(2),
        },
      };
      (bits, 1_u32.wrapping_shl(u32::from(bits)))
    }
  };
  let Ok(tail) = rav1d_msac_decode_bools(s, bits);
  let symbol = base.wrapping_add(tail);
  let decoded = match reference.wrapping_mul(2).cmp(&upper_bound) {
    core::cmp::Ordering::Less | core::cmp::Ordering::Equal => inv_recenter(reference, symbol),
    core::cmp::Ordering::Greater => {
      let end = upper_bound.wrapping_sub(1);
      let inverted = inv_recenter(end.wrapping_sub(reference), symbol);
      end.wrapping_sub(inverted)
    }
  };
  let Ok(value) = console_core_number_conversion::wrapped::<_, i32>(decoded);
  Ok(value)
}

struct SymbolInterval {
  symbol: u8,
  bottom: u32,
  top: u32,
}

fn adaptive_threshold(
  range: u32,
  probability: u16,
  remaining: u8,
) -> Result<u32, console_core_never::Never> {
  let reduced_probability = u32::from(probability.wrapping_shr(EC_PROB_SHIFT));
  Ok(range.wrapping_mul(reduced_probability)
    .wrapping_shr(7u32.wrapping_sub(EC_PROB_SHIFT))
    .wrapping_add(EC_MIN_PROB.wrapping_mul(u32::from(remaining))))
}

fn select_interval(
  s: &MsacContext,
  cdf: &[u16],
  threshold_count: u8,
) -> Rav1dResult<SymbolInterval> {
  match threshold_count {
    0..=15 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let Ok(counter_position) = index(threshold_count);
  let table = cdf.get(..=counter_position).ok_or(Rav1dError::EINVAL)?;
  let (&observations, _thresholds) = table.split_last().ok_or(Rav1dError::EINVAL)?;

  match observations {
    0..=32 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let Ok(coded) = window_prefix(s.dif);
  let probability_range = s.rng.wrapping_shr(8);
  let mut top = s.rng;

  for (&probability, symbol) in table.iter().zip(0u8..) {
    let Ok(bottom) = adaptive_threshold(probability_range, probability, threshold_count.wrapping_sub(symbol));

    match coded.cmp(&bottom) {
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
        return Ok(SymbolInterval { symbol, bottom, top });
      }
      core::cmp::Ordering::Less => top = bottom,
    }
  }

  Err(Rav1dError::EINVAL)
}

#[inline(always)]
fn normalize_adapted_symbol(
  s: &mut MsacContext,
  cdf: &mut [u16],
  n_symbols: u8,
  selection: Rav1dResult<SymbolInterval>,
) -> Result<u8, console_core_never::Never> {
  let SymbolInterval { symbol, bottom, top } = match selection {
    Ok(interval) => interval,
    Err(error) => {
      s.failure = Some(error);
      return Ok(0);
    }
  };
  let Ok(base) = in_window(bottom);
  let Ok(()) = ctx_norm(s, s.dif.wrapping_sub(base), top.wrapping_sub(bottom));
  let Ok(()) = s.adapt_cdf(cdf, DecodedSymbol { threshold_count: n_symbols, value: symbol });
  Ok(symbol)
}

#[inline(always)]
fn rav1d_msac_decode_bool_adapt_rust(s: &mut MsacContext, cdf: &mut [u16; 2]) -> Result<EntropyBit, console_core_never::Never> {
  let [probability, _] = *cdf;
  let Ok(bit) = rav1d_msac_decode_bool(s, u32::from(probability));
  let symbol = DecodedSymbol {
    threshold_count: 1,
    value: u8::from(bit == EntropyBit::One),
  };
  let Ok(()) = s.adapt_cdf(cdf, symbol);
  Ok(bit)
}

#[inline(always)]
fn rav1d_msac_decode_hi_tok_rust(
  s: &mut MsacContext,
  cdf: &mut [u16; 4],
) -> core::result::Result<u8, console_core_never::Never> {
  for start in [3u8, 6, 9] {
    let Ok(symbol) = rav1d_msac_decode_symbol_adapt4(s, cdf, 3);

    match symbol {
      3 => {}
      _ => return Ok(start.wrapping_add(symbol)),
    }
  }

  let Ok(last) = rav1d_msac_decode_symbol_adapt4(s, cdf, 3);
  Ok(12u8.wrapping_add(last))
}

#[inline(always)]
fn rav1d_msac_decode_symbol_adapt4_branchless(
  s: &mut MsacContext,
  cdf: &mut [u16],
  n_symbols: u8,
) -> core::result::Result<u8, console_core_never::Never> {
  let Ok(c) = window_prefix(s.dif);
  let r = s.rng.wrapping_shr(8);

  let (v0, v1, v2) = match (n_symbols, &*cdf) {
    (1, [first, _, ..]) => {
      let Ok(v0) = adaptive_threshold(r, *first, 1);
      (v0, 0, 0)
    }
    (2, [first, second, _, ..]) => {
      let Ok(v0) = adaptive_threshold(r, *first, 2);
      let Ok(v1) = adaptive_threshold(r, *second, 1);
      (v0, v1, 0)
    }
    (3, [first, second, third, _, ..]) => {
      let Ok(v0) = adaptive_threshold(r, *first, 3);
      let Ok(v1) = adaptive_threshold(r, *second, 2);
      let Ok(v2) = adaptive_threshold(r, *third, 1);
      (v0, v1, v2)
    }
    _ => {
      s.failure = Some(Rav1dError::EINVAL);
      return Ok(0);
    }
  };

  let symbol = u8::from(c < v0).wrapping_add(u8::from(c < v1)).wrapping_add(u8::from(c < v2));
  let (top, bottom) = match symbol {
    0 => (s.rng, v0),
    1 => (v0, v1),
    2 => (v1, v2),
    3 => (v2, 0),
    _ => {
      s.failure = Some(Rav1dError::EINVAL);
      return Ok(0);
    }
  };

  normalize_adapted_symbol(s, cdf, n_symbols, Ok(SymbolInterval { symbol, bottom, top }))
}

fn select_eight_interval(
  s: &MsacContext,
  cdf: &[u16],
  threshold_count: u8,
) -> Rav1dResult<SymbolInterval> {
  match threshold_count {
    1..=7 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let Ok(counter_position) = index(threshold_count);
  let table = cdf.get(..=counter_position).ok_or(Rav1dError::EINVAL)?;
  let (&observations, probabilities) = table.split_last().ok_or(Rav1dError::EINVAL)?;

  match observations {
    0..=32 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let Ok(coded) = window_prefix(s.dif);
  let probability_range = s.rng.wrapping_shr(8);
  let mut thresholds = [0u32; 8];

  for ((threshold, &probability), position) in thresholds.iter_mut().zip(probabilities).zip(0u8..) {
    let Ok(value) = adaptive_threshold(probability_range, probability, threshold_count.wrapping_sub(position));
    *threshold = value;
  }

  let symbol = thresholds.iter().fold(0u8, |sum, &threshold| sum.wrapping_add(u8::from(coded < threshold)));
  let Ok(position) = index(symbol);
  let bottom = ::core::result::Result::<&_, _>::copied(thresholds.get(position).ok_or(Rav1dError::EINVAL))?;
  let top = match position.checked_sub(1) {
    Some(previous) => ::core::result::Result::<&_, _>::copied(thresholds.get(previous).ok_or(Rav1dError::EINVAL))?,
    None => s.rng,
  };
  Ok(SymbolInterval { symbol, bottom, top })
}

impl MsacContext {
  pub(crate) fn new(
    data: CArc<[u8]>,
    updates: CdfUpdates,
    dsp: &Rav1dMsacDSPContext,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let buf = MsacAsmContextBuf { pos: 0 };
      let asm = MsacAsmContext {
        buf,
        dif: 0,
        rng: 0x8000,
        cnt: -15,
        updates,
      };
      let mut s = Self {
        asm,
        data: Some(data),
        failure: None,
      };
      let _ = dsp;
      let Ok(()) = ctx_refill(&mut s);
      s
    })
  }
}

#[inline(always)]
pub fn rav1d_msac_decode_symbol_adapt4(
  s: &mut MsacContext,
  cdf: &mut [u16],
  n_symbols: u8,
) -> core::result::Result<u8, console_core_never::Never> {
  rav1d_msac_decode_symbol_adapt4_branchless(s, cdf, n_symbols)
}

#[inline(always)]
pub fn rav1d_msac_decode_symbol_adapt8(
  s: &mut MsacContext,
  cdf: &mut [u16],
  n_symbols: u8,
) -> Result<u8, console_core_never::Never> {
  let interval = select_eight_interval(s, cdf, n_symbols);
  normalize_adapted_symbol(s, cdf, n_symbols, interval)
}

#[inline(always)]
pub fn rav1d_msac_decode_symbol_adapt16(
  s: &mut MsacContext,
  cdf: &mut [u16],
  n_symbols: u8,
) -> Result<u8, console_core_never::Never> {
  let interval = select_interval(s, cdf, n_symbols);
  normalize_adapted_symbol(s, cdf, n_symbols, interval)
}

#[inline(always)]
pub fn rav1d_msac_decode_bool_adapt(s: &mut MsacContext, cdf: &mut [u16; 2]) -> Result<EntropyBit, console_core_never::Never> {
  rav1d_msac_decode_bool_adapt_rust(s, cdf)
}

#[inline(always)]
pub fn rav1d_msac_decode_bool_equi(s: &mut MsacContext) -> Result<EntropyBit, console_core_never::Never> {
  rav1d_msac_decode_bool_equi_rust(s)
}

#[inline(always)]
pub fn rav1d_msac_decode_bool(s: &mut MsacContext, f: c_uint) -> Result<EntropyBit, console_core_never::Never> {
  rav1d_msac_decode_bool_rust(s, f)
}

#[inline(always)]
pub fn rav1d_msac_decode_hi_tok(
  s: &mut MsacContext,
  cdf: &mut [u16; 4],
) -> core::result::Result<u8, console_core_never::Never> {
  rav1d_msac_decode_hi_tok_rust(s, cdf)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::src::c_box::CBox;

  #[cfg_attr(
    dylint_lib = "explicit077_no_ambient_randomness",
    allow(
      explicit077_no_ambient_randomness,
      reason = "the generator is written out here and takes its seed from the test, so a failing run is replayed rather than met again by chance"
    )
  )]
  fn next_sample(seed: &mut u64) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      *seed ^= u64::wrapping_shl(*seed, 13_u32);
      *seed ^= u64::wrapping_shr(*seed, 7_u32);
      *seed ^= u64::wrapping_shl(*seed, 17_u32);
      {
        let Ok(numeric_value) =
          console_core_number_conversion::wrapped::<_, u32>(u64::wrapping_shr(*seed, 16_u32));
        numeric_value
      }
    })
  }

  fn state(
    s: &MsacContext,
  ) -> core::result::Result<(EcWin, c_uint, c_int, u64), console_core_never::Never> {
    Ok((s.dif, s.rng, s.cnt, s.buf.pos))
  }

  #[test]
  fn eight_symbol_reader_and_general_reader_preserve_the_same_entropy_state() -> Rav1dResult {
    let mut bytes = [0u8; 257];
    let mut seed = 0x9852_1d2c_5f61_7803;

    for byte in &mut bytes {
      let Ok(value) = next_sample(&mut seed);
      let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(value);
      *byte = sample;
    }

    let payload: Box<[u8]> = Box::new(bytes);
    let data = CArc::wrap(CBox::Rust(payload))?;
    let Ok(dsp) = Rav1dMsacDSPContext::default();

    for updates in [CdfUpdates::Frozen, CdfUpdates::Allowed] {
      for threshold_count in 1u8..=7 {
        let Ok(mut actual) = MsacContext::new(data.clone(), updates, &dsp);
        let Ok(mut general) = MsacContext::new(data.clone(), updates, &dsp);
        let mut table = [0xcede; 8];
        let Ok(observation_position) = index(threshold_count);
        let used = table.get_mut(..=observation_position).ok_or(Rav1dError::EINVAL)?;
        let (observations, probabilities) = used.split_last_mut().ok_or(Rav1dError::EINVAL)?;
        *observations = 0;

        for (entry, probability) in probabilities.iter_mut().zip([22_528, 18_432, 14_336, 10_240, 6144, 4096, 2048]) {
          *entry = probability;
        }

        let mut reference = table;

        for _sample in 0..2048 {
          let decoded = rav1d_msac_decode_symbol_adapt8(&mut actual, &mut table, threshold_count);
          let expected = rav1d_msac_decode_symbol_adapt16(&mut general, &mut reference, threshold_count);
          assert_eq!(decoded, expected);
          assert_eq!(table, reference);
          assert_eq!(state(&actual), state(&general));
        }
      }
    }

    let Ok(mut decoder) = MsacContext::new(data, CdfUpdates::Allowed, &dsp);
    let mut short_table = [4096];
    let beginning = state(&decoder);
    let decoded = rav1d_msac_decode_symbol_adapt8(&mut decoder, &mut short_table, 1);
    assert_eq!(decoded, Ok(0));
    assert_eq!(state(&decoder), beginning);
    assert_eq!(short_table, [4096]);
    assert!(matches!(decoder.check(), Err(Rav1dError::EINVAL)));
    Ok(())
  }

  #[test]
  fn an_invalid_adaptive_table_latches_a_decode_error_without_mutating_storage() {
    let mut decoder = MsacContext::default();
    let mut table = [8192, 4096];
    let symbol = DecodedSymbol { threshold_count: 3, value: 1 };

    let Ok(()) = decoder.adapt_cdf(&mut table, symbol);
    assert_eq!(decoder.check(), Ok(()));
    assert_eq!(table, [8192, 4096]);

    decoder.updates = CdfUpdates::Allowed;
    let Ok(()) = decoder.adapt_cdf(&mut table, symbol);
    assert_eq!(decoder.check(), Err(Rav1dError::EINVAL));
    assert_eq!(table, [8192, 4096]);

    let mut valid_table = [24576, 16384, 8192, 0];
    let Ok(()) = decoder.adapt_cdf(&mut valid_table, symbol);
    assert_eq!(decoder.check(), Err(Rav1dError::EINVAL));
    assert_eq!(valid_table, [24576, 16384, 8192, 0]);
  }

  struct BytewiseWindow {
    consumed: u64,
    shift: i32,
    difference: EcWin,
  }

  fn refill_bytewise_reference(
    source: &[u8],
    mut window: BytewiseWindow,
  ) -> Result<BytewiseWindow, console_core_never::Never> {
    let mut bytes = source.iter();

    for bit in (0..=window.shift).rev().step_by(8) {
      let Ok(place) = console_core_number_conversion::wrapped::<_, u32>(bit);
      let mask = EcWin::from(u8::MAX);

      match bytes.next() {
        Some(&byte) => {
          let bits = EcWin::from(!byte);
          window.difference |= bits.wrapping_shl(place);
          window.shift = window.shift.wrapping_sub(8);
          window.consumed = window.consumed.wrapping_add(1);
        }
        None => {
          window.difference |= !(!mask).wrapping_shl(place);
          break;
        }
      }
    }

    Ok(window)
  }

  #[test]
  fn refill_chunk_matches_bytewise_window_and_position() -> Rav1dResult {
    let Ok(window_bits) = console_core_number_conversion::wrapped::<_, i32>(EC_WIN_SIZE);
    let refill_base = window_bits.wrapping_sub(24);
    let seed_difference = EcWin::MAX.wrapping_shl(EcWin::BITS.wrapping_sub(1));

    for length in 0..=16u64 {
      let bytes: Vec<u8> = (0..length).map(|index| {
        let Ok(value) = console_core_number_conversion::wrapped::<_, u8>(index.wrapping_mul(53));
        value
      }).collect();
      let data = CArc::wrap(CBox::Rust(bytes.into_boxed_slice()))?;

      for start in 0..=length.min(3) {
        for shift in -1..window_bits {
          let mut decoder = MsacContext {
            asm: MsacAsmContext {
              buf: MsacAsmContextBuf { pos: start },
              cnt: refill_base.wrapping_sub(shift),
              dif: seed_difference,
              ..Default::default()
            },
            data: Some(data.clone()),
            failure: None,
          };
          let Ok(()) = ctx_refill(&mut decoder);

          let Ok(offset) = index(start);
          let source = data.get(offset..).ok_or(Rav1dError::EINVAL)?;
          let initial = BytewiseWindow { consumed: start, shift, difference: seed_difference };
          let Ok(expected) = refill_bytewise_reference(source, initial);
          assert_eq!(decoder.dif, expected.difference);
          assert_eq!(decoder.cnt, refill_base.wrapping_sub(expected.shift));
          assert_eq!(decoder.buf.pos, expected.consumed);
        }
      }
    }

    Ok(())
  }

  fn assert_cdf_update(initial: [u16; 5], update: CdfUpdate) -> Rav1dResult {
    let mut actual = initial;
    let mut expected = initial;
    let CdfUpdate { symbol, rate, observations } = update;
    let Ok(end) = index(symbol.threshold_count);
    let used = expected.get_mut(..=end).ok_or(Rav1dError::EINVAL)?;
    let (counter, probabilities) = used.split_last_mut().ok_or(Rav1dError::EINVAL)?;
    let observed = u32::from(rate);

    for (lane, probability) in (0_u8..).zip(probabilities) {
      let current = u32::from(*probability);
      let reference = match lane.cmp(&symbol.value) {
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
          current.wrapping_sub(current.wrapping_shr(observed))
        }
        core::cmp::Ordering::Less => {
          let distance = 98_304u32.wrapping_sub(current) & 65_535;
          current.wrapping_add(distance.wrapping_shr(observed))
        }
      };
      let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(reference);
      *probability = value;
    }

    *counter = observations.wrapping_add(u16::from(observations < 32));
    let result = update_cdf(&mut actual, update);
    assert_eq!(result, Ok(()));
    assert_eq!(actual, expected, "initial={initial:?} count={observations} rate={rate}");
    Ok(())
  }

  #[test]
  fn cdf_update_matches_directional_arithmetic_for_every_u16() -> Rav1dResult {
    for probability in 0..=u16::MAX {
      for rate in 4..=7 {
        for threshold_count in [1u8, 3] {
          for value in 0..=threshold_count {
            for observations in [0, 15, 16, 31, 32] {
              let mut initial = [0xfade; 5];
              let Ok(last) = index(threshold_count);
              let used = initial.get_mut(..=last).ok_or(Rav1dError::EINVAL)?;
              let (count, probabilities) = used.split_last_mut().ok_or(Rav1dError::EINVAL)?;
              probabilities.fill(probability);
              *count = observations;
              assert_cdf_update(initial, CdfUpdate {
                symbol: DecodedSymbol { threshold_count, value }, rate, observations,
              })?;
            }
          }
        }
      }
    }

    Ok(())
  }

  #[cfg(target_arch = "x86_64")]
  #[test]
  fn cdf_update_preserves_arithmetic_with_baseline_token_disabled() {
    use archmage::SimdToken as _;
    use archmage::testing::{CompileTimePolicy, for_each_token_permutation};
    use core::cell::Cell;

    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let executed = Cell::new(0u8);
    let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |_| {
      let path = match archmage::X64V1Token::summon() {
        Some(_) => 1,
        None => 2,
      };
      executed.set(executed.get() | path);

      for value in [0, 1, 2, 3, 4, u8::MAX] {
        for rate in 0..16 {
          for observations in [0, 15, 16, 31, 32, u16::MAX] {
            let initial = [0, 32768, 65535, observations, 0xfade];
            let update = CdfUpdate {
              symbol: DecodedSymbol { threshold_count: 3, value }, rate, observations,
            };
            assert_eq!(assert_cdf_update(initial, update), Ok(()));
          }
        }
      }
    });

    assert!(report.permutations_run > 1);
    assert_eq!(executed.get(), 3, "both dispatch paths must execute");
  }

  struct RefillFixture {
    length: u16,
    thresholds: u8,
    updates: CdfUpdates,
    observations: u16,
  }

  fn check_refill_fixture(
    RefillFixture { length, thresholds, updates, observations }: RefillFixture,
    seed: &mut u64,
    symbols: &mut [u32; 4],
  ) -> Rav1dResult {
    let mut bytes = Vec::new();

    for _ in 0..length {
      let Ok(sample) = next_sample(seed);
      let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(sample);
      bytes.push(byte);
    }

    let data = CArc::wrap(CBox::Rust(bytes.into_boxed_slice()))?;
    let Ok(dsp) = Rav1dMsacDSPContext::default();
    let Ok(mut actual) = MsacContext::new(data.clone(), updates, &dsp);
    let Ok(mut reference) = MsacContext::new(data, updates, &dsp);
    let mut actual_cdf = [0xfade; 8];
    let Ok(last) = index(thresholds);
    let used = actual_cdf.get_mut(..=last).ok_or(Rav1dError::EINVAL)?;
    let (counter, probabilities) = used.split_last_mut().ok_or(Rav1dError::EINVAL)?;

    for probability in probabilities.iter_mut() {
      let Ok(sample) = next_sample(seed);
      let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(sample & 32_767);
      *probability = value;
    }

    probabilities.sort_unstable_by(|left, right| right.cmp(left));
    *counter = observations;
    let mut reference_cdf = actual_cdf;

    for _ in 0..256 {
      let Ok(value) = rav1d_msac_decode_symbol_adapt4(&mut actual, &mut actual_cdf, thresholds);
      let Ok(expected) = rav1d_msac_decode_symbol_adapt16(&mut reference, &mut reference_cdf, thresholds);
      let Ok(lane) = index(value);
      let count = symbols.get_mut(lane).ok_or(Rav1dError::EINVAL)?;
      *count = count.wrapping_add(1);
      assert_eq!(value, expected, "len={length} n={thresholds} updates={updates:?}");
      assert_eq!(actual_cdf, reference_cdf, "all CDF entries, including untouched suffix");
      assert_eq!(state(&actual), state(&reference));
    }

    Ok(())
  }

  #[test]
  fn adapt4_matches_reference_through_refills_and_preserves_unused_cdf() -> Rav1dResult {
    let mut seed = 0x61d4_f283_190b_7365;
    let mut symbols = [0u32; 4];

    for length in [0, 1, 2, 7, 8, 9, 15, 16, 17, 64, 511] {
      for thresholds in 1..=3 {
        for updates in [CdfUpdates::Frozen, CdfUpdates::Allowed] {
          for observations in [0, 15, 16, 31, 32] {
            let fixture = RefillFixture { length, thresholds, updates, observations };
            check_refill_fixture(fixture, &mut seed, &mut symbols)?;
          }
        }
      }
    }

    assert!(symbols.into_iter().all(|count| count > 100), "exercise every symbol");
    Ok(())
  }

  struct BoundaryFixture {
    thresholds: u8,
    range: u32,
    probability: u16,
    code: u32,
  }

  fn check_boundary_slice(
    initial: [u16; 8],
    BoundaryFixture { thresholds, range, probability, code }: BoundaryFixture,
  ) -> Rav1dResult<u32> {
    let mut checks = 0u32;

    for slice_length in [thresholds.wrapping_add(1), 4, 8] {
      for updates in [CdfUpdates::Frozen, CdfUpdates::Allowed] {
        let Ok(window_code) = in_window(code);
        let context = || MsacContext {
          asm: MsacAsmContext { dif: window_code, rng: range, cnt: 64, updates, ..Default::default() },
          ..Default::default()
        };
        let (mut actual_cdf, mut reference_cdf) = (initial, initial);
        let (mut actual, mut reference) = (context(), context());
        let Ok(length) = index(slice_length);
        let actual_slice = actual_cdf.get_mut(..length).ok_or(Rav1dError::EINVAL)?;
        let reference_slice = reference_cdf.get_mut(..length).ok_or(Rav1dError::EINVAL)?;
        let actual_value = rav1d_msac_decode_symbol_adapt4(&mut actual, actual_slice, thresholds);
        let expected = rav1d_msac_decode_symbol_adapt16(&mut reference, reference_slice, thresholds);
        assert_eq!(actual_value, expected, "n={thresholds} rng={range} p={probability} c={code}");
        assert_eq!(actual_cdf, reference_cdf);
        assert_eq!(state(&actual), state(&reference));
        checks = checks.wrapping_add(1);
      }
    }

    Ok(checks)
  }

  fn compare_threshold_boundaries(thresholds: u8, range: u32, probability: u16) -> Rav1dResult<u32> {
    let rate = range.wrapping_shr(8).wrapping_mul(u32::from(probability.wrapping_shr(6))).wrapping_shr(1);
    let highest = rate.wrapping_add(4u32.wrapping_mul(u32::from(thresholds)));

    match {
        ::core::primitive::bool::then_some(highest > range, ())
    } {
        ::core::option::Option::Some(()) => {
          return Ok(0);
        },
        ::core::option::Option::None => {},
    }

    let mut checks = 0u32;

    for observations in [0, 15, 16, 31, 32] {
      let mut initial = [0xfade; 8];
      let Ok(last) = index(thresholds);
      let used = initial.get_mut(..=last).ok_or(Rav1dError::EINVAL)?;
      let (counter, probabilities) = used.split_last_mut().ok_or(Rav1dError::EINVAL)?;
      probabilities.fill(probability);
      *counter = observations;

      for entry in 0..=thresholds {
        let remaining = u32::from(thresholds.wrapping_sub(entry));
        let boundary = match remaining {
          0 => 0,
          _ => rate.wrapping_add(4u32.wrapping_mul(remaining)),
        };

        'sample: for code in [boundary.saturating_sub(1), boundary, boundary.wrapping_add(1), range.wrapping_sub(1)] {
          match code.cmp(&range) {
            core::cmp::Ordering::Less => {}
            core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => continue 'sample,
          }

          let fixture = BoundaryFixture { thresholds, range, probability, code };
          let count = check_boundary_slice(initial, fixture)?;
          checks = checks.wrapping_add(count);
        }
      }
    }

    Ok(checks)
  }

  #[test]
  fn adapt4_matches_reference_at_interval_boundaries_and_short_slices() -> Rav1dResult {
    let mut checks = 0u32;

    for thresholds in 1..=3 {
      for range in [32768, 32769, 32831, 32896, 33024, 49151, 65534, 65535] {
        for probability in [0, 1, 31, 32, 63, 64, 65, 127, 128, 129, 16384, 32767, 32768] {
          let count = compare_threshold_boundaries(thresholds, range, probability)?;
          checks = checks.wrapping_add(count);
        }
      }
    }

    assert!(checks > 50_000);
    Ok(())
  }
}
