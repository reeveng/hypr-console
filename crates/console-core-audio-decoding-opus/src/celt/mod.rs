//! CELT layer decoder (RFC 6716 section 4.3).
//!
//! The mode's tables -- the pulse cache, the band caps, the window and the four transforms --
//! are computed when a decoder is made and belong to it, rather than to the process. They are
//! a few kilobytes and every stream of a file builds the same ones, which is a small price for
//! a decoder that is handed everything it decides from.
//!
//! The output rate is always 48 kHz, so nothing is decimated. The synthesis memory keeps the
//! last 2048 samples of each channel and the window's overlap; a frame slides it left by its
//! own length and writes its samples at the end, and the comb filter and the concealment read
//! back into it from there, so every position they read is at least the shortest pitch before
//! the one they write. A position past either end of a buffer reads as silence and a write
//! past it is dropped.

pub(crate) mod bands;
pub(crate) mod energy;
pub(crate) mod mdct;
pub(crate) mod rate;
pub(crate) mod tables;

use std::cmp::Ordering;
use std::num::FpCategory;

use std::cell::Cell;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32};

use bands::{
  AllBands, Collapse, Denormal, Inversion, Spectra, Spread, anti_collapse, denormalise_bands,
  lcg_rand, quant_all_bands, renormalise_vector,
};
use energy::{Fine, unquant_coarse_energy, unquant_energy_finalise, unquant_fine_energy};
use mdct::{ImdctScratch, Lapping, Spectrum};
use rate::{Mode, Request, compute_allocation, edge, of, set, width};
use tables::{
  BANDS, COMB_GAINS, MAX_LM, OVERLAP, PREEMPH, Place, SHORT_MDCT, SPREAD, TAPSET,
  TF_SELECT_TABLE, TRIM, entry,
};

use crate::Error;
use crate::packet::Channels;
use crate::quantities::{BandGain, LossFade};
use crate::range::{BITRES, Bit, RangeDecoder};

const DECODE_BUFFER_SIZE: u32 = 2048;
const MEMORY: u32 = DECODE_BUFFER_SIZE + OVERLAP;
const COMBFILTER_MINPERIOD: i32 = 15;
const PLC_PITCH_LAG_MAX: u32 = 720;
const PLC_PITCH_LAG_MIN: u32 = 100;
const PLC_HISTORY: u32 = 1024;
const LONGEST_FRAME: u32 = 1275;
const SILENT_ENERGY: f32 = -28.0;

#[derive(Clone, Copy)]
pub(crate) struct Layout {
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) channels: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mixing {
  Replace,
  Add,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "gain is the comb-filter tap amplitude ratio"
  )
)]
struct Tap {
  period: i32,
  gain: f32,
  tapset: u32,
}

const QUIET: Tap = Tap {
  period: 0,
  gain: 0.0,
  tapset: 0,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Concealment {
  Pitch,
  Noise,
}

pub(crate) struct CeltDecoder {
  mode: Mode,
  channels: u32,
  pub(crate) stream_channels: u32,
  pub(crate) start: u32,
  pub(crate) end: u32,
  inversion: Inversion,
  rng: u32,
  loss_count: u32,
  skip_plc: Bit,
  postfilter: Tap,
  postfilter_old: Tap,
  preemph_mem: [f32; 2],
  decode_mem: Vec<f32>,
  old_band_e: [f32; 42],
  old_log_e: [f32; 42],
  old_log_e2: [f32; 42],
  background_log_e: [f32; 42],
  x: Vec<f32>,
  freq: Vec<f32>,
  imdct: ImdctScratch,
}

struct Header {
  silence: Bit,
  postfilter: Tap,
  transient: Bit,
  intra: Bit,
}

struct Coded {
  lm: u32,
  size: u32,
  length: u32,
}

fn float(value: u32) -> Result<f32, Never> {
  narrow_f32(f64::from(value))
}

fn sample(cells: &[Cell<f32>], at: u32) -> Result<f32, Never> {
  let Ok(place) = index(at);

  Ok(match cells.get(place) {
    Some(cell) => cell.get(),
    None => 0.0,
  })
}

fn slide(memory: &mut [f32], by: u32) -> Result<(), Never> {
  let Ok(from) = index(by);
  let Ok(end) = index(DECODE_BUFFER_SIZE.wrapping_add(OVERLAP.wrapping_shr(1)));

  match memory.get(from..end) {
    Some(_) => memory.copy_within(from..end, 0),
    None => {}
  }

  Ok(())
}

impl CeltDecoder {
  pub(crate) fn new(layout: Channels) -> Result<CeltDecoder, Never> {
    let (channels, inversion) = match layout {
      Channels::Mono => (1u32, Inversion::Disabled),
      Channels::Stereo => (2u32, Inversion::Allowed),
    };
    let Ok(memory) = index(channels.wrapping_mul(MEMORY));
    let Ok(mode) = Mode::new();
    let mut made = CeltDecoder {
      mode,
      channels,
      stream_channels: channels,
      start: 0,
      end: BANDS,
      inversion,
      rng: 0,
      loss_count: 0,
      skip_plc: Bit::One,
      postfilter: QUIET,
      postfilter_old: QUIET,
      preemph_mem: [0.0; 2],
      decode_mem: vec![0.0; memory.min(const { 2 * (2048 + 120) })],
      old_band_e: [0.0; 42],
      old_log_e: [0.0; 42],
      old_log_e2: [0.0; 42],
      background_log_e: [0.0; 42],
      x: Vec::new(),
      freq: Vec::new(),
      imdct: ImdctScratch::default(),
    };
    let Ok(()) = made.reset();

    Ok(made)
  }

  pub(crate) fn window(&self) -> Result<&[f32; 120], Never> {
    Ok(&self.mode.window)
  }

  pub(crate) fn reset(&mut self) -> Result<(), Never> {
    self.rng = 0;
    self.loss_count = 0;
    self.skip_plc = Bit::One;
    self.postfilter = QUIET;
    self.postfilter_old = QUIET;
    self.preemph_mem = [0.0; 2];
    self.decode_mem.fill(0.0);
    self.old_band_e = [0.0; 42];
    self.old_log_e = [SILENT_ENERGY; 42];
    self.old_log_e2 = [SILENT_ENERGY; 42];
    self.background_log_e = [0.0; 42];

    Ok(())
  }

  pub(crate) fn decode(
    &mut self,
    data: Option<&[u8]>,
    frame_size: u32,
    pcm: &mut [f32],
    mixing: Mixing,
    dec: Option<&mut RangeDecoder<'_>>,
  ) -> Result<u32, Error> {
    let lm = match frame_size {
      120 => 0,
      240 => 1,
      480 => 2,
      960 => 3,
      _other => return Err(Error::CeltFrameSize),
    };

    match data.filter(|data_2| data_2.len() > 1) {
      None => {
        let Ok(()) = self.decode_lost(
          crate::quantities::FrameSize(frame_size),
          crate::quantities::BlockOrder(lm),
        );
      }
      Some(data_3) => {
        let Ok(length) = fitted::<_, u32>(data_3.len());

        match length.cmp(&LONGEST_FRAME) {
          Ordering::Greater => return Err(Error::CeltFrameTooLong),
          Ordering::Less | Ordering::Equal => {}
        }

        let coded = Coded {
          lm,
          size: frame_size,
          length,
        };
        let Ok(()) = match dec {
          Some(dec_2) => self.decode_frame(dec_2, &coded),
          None => {
            let Ok(mut own) = RangeDecoder::new(data_3);

            self.decode_frame(&mut own, &coded)
          }
        };
      }
    }

    let Ok(()) = self.deemphasis(frame_size, pcm, mixing);

    Ok(frame_size)
  }

  fn header(&mut self, dec: &mut RangeDecoder<'_>, coded: &Coded) -> Result<Header, Never> {
    let total = coded.length.wrapping_mul(8).cast_signed();
    let Ok(mut tell) = dec.tell();

    let silence = match (tell.cmp(&total), tell) {
      (Ordering::Equal | Ordering::Greater, _) => Bit::One,
      (Ordering::Less, 1) => {
        let Ok(bit) = dec.bit_logp(15);

        bit
      }
      (Ordering::Less, _) => Bit::Zero,
    };

    match silence {
      Bit::One => {
        let Ok(()) = dec.skip_to_end(total);

        tell = total;
      }
      Bit::Zero => {}
    }

    let mut postfilter = QUIET;

    match (self.start, tell.wrapping_add(16).cmp(&total)) {
      (0, Ordering::Less | Ordering::Equal) => {
        let Ok(()) = postfilter_header(dec, total, &mut postfilter);
        let Ok(told) = dec.tell();

        tell = told;
      }
      _skip => {}
    }

    let transient = match (coded.lm, tell.wrapping_add(3).cmp(&total)) {
      (1.., Ordering::Less | Ordering::Equal) => {
        let Ok(bit) = dec.bit_logp(3);
        let Ok(told) = dec.tell();

        tell = told;

        bit
      }
      _unset => Bit::Zero,
    };
    let intra = match tell.wrapping_add(3).cmp(&total) {
      Ordering::Less | Ordering::Equal => {
        let Ok(bit) = dec.bit_logp(3);

        bit
      }
      Ordering::Greater => Bit::Zero,
    };

    Ok(Header {
      silence,
      postfilter,
      transient,
      intra,
    })
  }

  fn layout(&self) -> Result<Layout, Never> {
    Ok(Layout {
      start: self.start,
      end: self.end,
      channels: self.stream_channels,
    })
  }

  fn dynalloc(
    &self,
    dec: &mut RangeDecoder<'_>,
    coded: &Coded,
    cap: &[i32; 21],
    total_bits: &mut i32,
  ) -> Result<[i32; 21], Never> {
    let mut offsets = [0i32; 21];
    let mut dynalloc_logp = 6i32;
    let Ok(told) = dec.tell_frac();
    let mut tellf = told.cast_signed();
    let channels = self.stream_channels.cast_signed();

    for band in self.start..self.end {
      let Ok(span) = width(band);
      let wide = channels.wrapping_mul(span).wrapping_shl(coded.lm);
      let quanta = wide
        .wrapping_shl(BITRES)
        .min(6i32.wrapping_shl(BITRES).max(wide));
      let mut loop_logp = dynalloc_logp;
      let mut boost = 0i32;
      let Ok(limit) = of(cap, band);

      'band_quanta: for _quantum in 0..limit.max(0) {
        match (
          tellf
            .wrapping_add(loop_logp.wrapping_shl(BITRES))
            .cmp(total_bits),
          boost.cmp(&limit),
        ) {
          (Ordering::Less, Ordering::Less) => {}
          _budget_reached => break 'band_quanta,
        }

        let Ok(flag) = dec.bit_logp(loop_logp.cast_unsigned());
        let Ok(told_2) = dec.tell_frac();

        tellf = told_2.cast_signed();

        match flag {
          Bit::Zero => break 'band_quanta,
          Bit::One => {}
        }

        boost = boost.wrapping_add(quanta);
        *total_bits = total_bits.wrapping_sub(quanta);
        loop_logp = 1;
      }

      let Ok(()) = set(&mut offsets, band, boost);

      match boost.cmp(&0) {
        Ordering::Greater => dynalloc_logp = dynalloc_logp.wrapping_sub(1).max(2),
        Ordering::Less | Ordering::Equal => {}
      }
    }

    Ok(offsets)
  }

  fn decode_frame(&mut self, dec: &mut RangeDecoder<'_>, coded: &Coded) -> Result<(), Never> {
    match self.loss_count {
      0 => self.skip_plc = Bit::Zero,
      _lost => {}
    }

    match self.stream_channels {
      1 => {
        let (first, second) = self.old_band_e.split_at_mut(21);

        for (mine, other) in first.iter_mut().zip(second.iter()) {
          *mine = mine.max(*other);
        }
      }
      _stereo => {}
    }

    let Ok(header) = self.header(dec, coded);
    let Ok(layout) = self.layout();
    let Ok(()) = unquant_coarse_energy(layout, &mut self.old_band_e, header.intra, dec, coded.lm);
    let mut tf_res = [0i32; 21];
    let Ok(()) = tf_decode(layout, header.transient, &mut tf_res, coded.lm, dec);
    let Ok(spread) = spread(dec, coded);
    let Ok(cap) = self.mode.init_caps(
      crate::quantities::BlockOrder(coded.lm),
      crate::quantities::ChannelCount(self.stream_channels),
    );
    let mut total_bits = coded
      .length
      .wrapping_mul(8)
      .cast_signed()
      .wrapping_shl(BITRES);
    let Ok(offsets) = self.dynalloc(dec, coded, &cap, &mut total_bits);
    let Ok(told) = dec.tell_frac();
    let alloc_trim = match told
      .cast_signed()
      .wrapping_add(6i32.wrapping_shl(BITRES))
      .cmp(&total_bits)
    {
      Ordering::Less | Ordering::Equal => {
        let Ok(trim) = dec.symbol(&TRIM, 7);

        trim.cast_signed()
      }
      Ordering::Greater => 5,
    };
    let Ok(told_2) = dec.tell_frac();
    let bits = coded
      .length
      .wrapping_mul(8)
      .cast_signed()
      .wrapping_shl(BITRES)
      .wrapping_sub(told_2.cast_signed())
      .wrapping_sub(1);
    let anti_collapse_rsv = match (
      header.transient,
      coded.lm,
      bits.cmp(&coded.lm.wrapping_add(2).cast_signed().wrapping_shl(BITRES)),
    ) {
      (Bit::One, 2.., Ordering::Equal | Ordering::Greater) => 1i32.wrapping_shl(BITRES),
      _unreserved => 0,
    };
    let request = Request {
      start: self.start,
      end: self.end,
      offsets: &offsets,
      cap: &cap,
      alloc_trim,
      total: bits.wrapping_sub(anti_collapse_rsv),
      channels: self.stream_channels,
      lm: coded.lm,
    };
    let Ok(alloc) = compute_allocation(&self.mode, &request, dec);
    let Ok(()) = unquant_fine_energy(layout, &mut self.old_band_e, &alloc.fine_quant, dec);
    let Ok(()) = self.slide_all(coded.size);
    let Ok(()) = self.shapes(
      dec,
      coded,
      &header,
      &alloc,
      (spread, tf_res, anti_collapse_rsv),
    );
    let Ok(()) = self.postfilter(coded, header.postfilter);
    let Ok(()) = self.remember(coded, header.transient);

    let Ok(range) = dec.range();

    self.rng = range;
    self.loss_count = 0;

    Ok(())
  }

  fn slide_all(&mut self, by: u32) -> Result<(), Never> {
    let Ok(each) = index(MEMORY);

    for memory in self.decode_mem.chunks_exact_mut(each.max(1)) {
      let Ok(()) = slide(memory, by);
    }

    Ok(())
  }

  fn shapes(
    &mut self,
    dec: &mut RangeDecoder<'_>,
    coded: &Coded,
    header: &Header,
    alloc: &rate::Allocation,
    coding: (Spread, [i32; 21], i32),
  ) -> Result<(), Never> {
    let (spread, tf_res, anti_collapse_rsv) = coding;
    let Ok(layout) = self.layout();
    let mut collapse_masks = [0u32; 42];
    let mut x = std::mem::take(&mut self.x);
    let Ok(spectrum) = index(self.stream_channels.wrapping_mul(coded.size));
    let Ok(size) = index(coded.size);
    let packet_bits = coded.length.wrapping_mul(8).cast_signed();

    x.clear();
    x.resize(spectrum.min(1920), 0.0);

    let frame = AllBands {
      layout,
      pulses: &alloc.pulses,
      short_blocks: header.transient,
      spread,
      dual_stereo: alloc.dual_stereo,
      intensity: alloc.intensity,
      tf_res: &tf_res,
      total_bits: packet_bits.wrapping_shl(BITRES).wrapping_sub(anti_collapse_rsv),
      balance: alloc.balance,
      lm: coded.lm,
      coded_bands: alloc.coded_bands,
      inversion: self.inversion,
    };
    let (first, second) = x.split_at_mut(size.min(spectrum).min(1920));
    let y = bool::then_some(self.stream_channels == 2, second);
    let Ok(()) = quant_all_bands(
      &self.mode,
      Spectra { x: first, y },
      &mut collapse_masks,
      &frame,
      dec,
      &mut self.rng,
    );

    let anti_collapse_on = match anti_collapse_rsv.cmp(&0) {
      Ordering::Greater => {
        let Ok(bit) = dec.bits(1);

        bit
      }
      Ordering::Less | Ordering::Equal => 0,
    };
    let Ok(told) = dec.tell();
    let bits_left = packet_bits.wrapping_sub(told);
    let Ok(()) = unquant_energy_finalise(
      layout,
      &mut self.old_band_e,
      Fine {
        quant: &alloc.fine_quant,
        priority: &alloc.fine_priority,
      },
      bits_left,
      dec,
    );

    match anti_collapse_on {
      0 => {}
      _on => {
        let collapse = Collapse {
          masks: &collapse_masks,
          log_e: &self.old_band_e,
          prev1: &self.old_log_e,
          prev2: &self.old_log_e2,
          pulses: &alloc.pulses,
          lm: coded.lm,
          size: coded.size,
        };
        let Ok(()) = anti_collapse(&mut x, &collapse, layout, self.rng);
      }
    }

    match header.silence {
      Bit::One => {
        let Ok(count) = index(self.stream_channels.wrapping_mul(BANDS));

        for energy in self.old_band_e.iter_mut().take(count) {
          *energy = SILENT_ENERGY;
        }
      }
      Bit::Zero => {}
    }

    let synthesis = Synthesis {
      size: coded.size,
      start: self.start,
      end: self.end.min(BANDS),
      coded: self.stream_channels,
      transient: header.transient,
      lm: coded.lm,
      silence: header.silence,
    };
    let Ok(()) = self.synthesis(&x, &synthesis);

    self.x = x;

    Ok(())
  }

  fn postfilter(&mut self, coded: &Coded, wanted: Tap) -> Result<(), Never> {
    let Ok(each) = index(MEMORY);
    let offset = DECODE_BUFFER_SIZE.wrapping_sub(coded.size);
    let window = self.mode.window;

    for memory in self.decode_mem.chunks_exact_mut(each.max(1)) {
      self.postfilter.period = self.postfilter.period.max(COMBFILTER_MINPERIOD);
      self.postfilter_old.period = self.postfilter_old.period.max(COMBFILTER_MINPERIOD);

      let Ok(()) = comb_filter(
        memory,
        &Comb {
          offset,
          length: SHORT_MDCT,
          from: self.postfilter_old,
          to: self.postfilter,
        },
        &window,
      );

      match coded.lm {
        0 => {}
        _longer => {
          let Ok(()) = comb_filter(
            memory,
            &Comb {
              offset: offset.wrapping_add(SHORT_MDCT),
              length: coded.size.wrapping_sub(SHORT_MDCT),
              from: self.postfilter,
              to: wanted,
            },
            &window,
          );
        }
      }
    }

    self.postfilter_old = self.postfilter;
    self.postfilter = wanted;

    match coded.lm {
      0 => {}
      _longer => self.postfilter_old = self.postfilter,
    }

    Ok(())
  }

  fn remember(&mut self, coded: &Coded, transient: Bit) -> Result<(), Never> {
    match self.stream_channels {
      1 => {
        let (first, second) = self.old_band_e.split_at_mut(21);

        for (other, mine) in second.iter_mut().zip(first.iter()) {
          *other = *mine;
        }
      }
      _stereo => {}
    }

    match transient {
      Bit::Zero => {
        self.old_log_e2 = self.old_log_e;
        self.old_log_e = self.old_band_e;

        let Ok(blocks) = float(1u32.wrapping_shl(coded.lm));

        for (background, energy) in self.background_log_e.iter_mut().zip(self.old_band_e.iter()) {
          *background = (*background + blocks * 0.001).min(*energy);
        }
      }
      Bit::One => {
        for (log, energy) in self.old_log_e.iter_mut().zip(self.old_band_e.iter()) {
          *log = log.min(*energy);
        }
      }
    }

    for channel in 0..2u32 {
      for band in (0..self.start).chain(self.end..BANDS) {
        let Ok(at) = index(channel.wrapping_mul(BANDS).wrapping_add(band));

        for (energies, value) in [
          (&mut self.old_band_e, 0.0),
          (&mut self.old_log_e, SILENT_ENERGY),
          (&mut self.old_log_e2, SILENT_ENERGY),
        ] {
          match energies.get_mut(at) {
            Some(slot) => *slot = value,
            None => {}
          }
        }
      }
    }

    Ok(())
  }

  fn synthesis(&mut self, x: &[f32], asked: &Synthesis) -> Result<(), Never> {
    let Ok(size) = index(asked.size.min(960));
    let mut freq = std::mem::take(&mut self.freq);

    freq.clear();
    freq.resize(size.saturating_mul(2).min(1920), 0.0);

    let (f0, f1) = freq.split_at_mut(size);
    let first = Denormal {
      band_log_e: &self.old_band_e,
      start: asked.start,
      end: asked.end,
      lm: asked.lm,
      silence: asked.silence,
    };
    let second_energies = match self.old_band_e.get(21..) {
      Some(energies) => energies,
      None => &[],
    };
    let second = Denormal {
      band_log_e: second_energies,
      ..first
    };
    let Ok(each) = index(MEMORY);

    match (self.channels, asked.coded) {
      (2, 1) => {
        let Ok(()) = denormalise_bands(x, f0, &first);

        for (copy, value) in f1.iter_mut().zip(f0.iter()) {
          *copy = *value;
        }

        for (memory, spectrum) in self.decode_mem.chunks_exact_mut(each.max(1)).zip([&*f1, &*f0]) {
          let Ok(()) = transform(&self.mode, memory, spectrum, asked, &mut self.imdct);
        }
      }
      (1, 2) => {
        let Ok(()) = denormalise_bands(x, f0, &first);
        let Ok(()) = denormalise_bands(x.get(size..).map_or(&[], |rest| rest), f1, &second);

        for (mixed, other) in f0.iter_mut().zip(f1.iter()) {
          *mixed = 0.5 * (*mixed + *other);
        }

        match self.decode_mem.get_mut(..each) {
          Some(memory) => {
            let Ok(()) = transform(&self.mode, memory, f0, asked, &mut self.imdct);
          }
          None => {}
        }
      }
      (_, _) => {
        for ((memory, channel), energies) in self
          .decode_mem
          .chunks_exact_mut(each.max(1))
          .zip(x.chunks(size.max(1)))
          .zip([&first, &second])
        {
          let Ok(()) = denormalise_bands(channel, f0, energies);
          let Ok(()) = transform(&self.mode, memory, f0, asked, &mut self.imdct);
        }
      }
    }

    self.freq = freq;

    Ok(())
  }

  fn deemphasis(&mut self, size: u32, pcm: &mut [f32], mixing: Mixing) -> Result<(), Never> {
    let Ok(each) = index(MEMORY);
    let Ok(from) = index(DECODE_BUFFER_SIZE.wrapping_sub(size));
    let Ok(length) = index(size);
    let Ok(across) = index(self.channels);

    for ((channel, memory), state) in (0usize..)
      .zip(self.decode_mem.chunks_exact(each.max(1)))
      .zip(self.preemph_mem.iter_mut())
    {
      let mut held = *state;
      let mut outputs = pcm.iter_mut().skip(channel).step_by(across.max(1));

      for value in memory.iter().skip(from).take(length) {
        let emphasised = *value + 1e-30 + held;

        held = PREEMPH * emphasised;

        let heard = emphasised * (1.0 / 32768.0);

        match (outputs.next(), mixing) {
          (Some(slot), Mixing::Add) => *slot += heard,
          (Some(slot), Mixing::Replace) => *slot = heard,
          (None, _) => {}
        }
      }

      *state = held;
    }

    Ok(())
  }

  fn decode_lost(
    &mut self,
    crate::quantities::FrameSize(size): crate::quantities::FrameSize,
    crate::quantities::BlockOrder(lm): crate::quantities::BlockOrder,
  ) -> Result<(), Never> {
    let concealment = match (self.loss_count.cmp(&5), self.start, self.skip_plc) {
      (Ordering::Less, 0, Bit::Zero) => Concealment::Pitch,
      (_, _, _) => Concealment::Noise,
    };
    let Ok(()) = match concealment {
      Concealment::Noise => self.conceal_noise(
        crate::quantities::FrameSize(size),
        crate::quantities::BlockOrder(lm),
      ),
      Concealment::Pitch => self.pitch(size),
    };

    self.loss_count = self.loss_count.wrapping_add(1);

    Ok(())
  }

  fn conceal_noise(
    &mut self,
    crate::quantities::FrameSize(size): crate::quantities::FrameSize,
    crate::quantities::BlockOrder(lm): crate::quantities::BlockOrder,
  ) -> Result<(), Never> {
    let decay = match self.loss_count {
      0 => 1.5,
      _later => 0.5,
    };

    for channel in 0..self.channels {
      for band in self.start..self.end {
        let Ok(at) = index(channel.wrapping_mul(BANDS).wrapping_add(band));

        match (self.old_band_e.get_mut(at), self.background_log_e.get(at)) {
          (Some(energy), Some(background)) => *energy = background.max(*energy - decay),
          (Some(_) | None, _) => {}
        }
      }
    }

    let mut seed = self.rng;
    let mut x = std::mem::take(&mut self.x);
    let Ok(length) = index(self.channels.wrapping_mul(size));
    let end = self.end.min(BANDS);

    x.clear();
    x.resize(length.min(1920), 0.0);

    for channel in 0..self.channels {
      for band in self.start..end {
        let Ok(low) = edge(band);
        let Ok(span) = width(band);
        let Ok(from) = index(
          size
            .wrapping_mul(channel)
            .wrapping_add(low.cast_unsigned().wrapping_shl(lm)),
        );
        let Ok(count) = index(span.cast_unsigned().wrapping_shl(lm));
        let block = match x.get_mut(from..from.saturating_add(count)) {
          Some(block) => block,
          None => &mut [],
        };

        for slot in block.iter_mut() {
          let Ok(next) = lcg_rand(seed);
          let Ok(value) = narrow_f32(f64::from(next.cast_signed().wrapping_shr(20)));

          seed = next;
          *slot = value;
        }

        let Ok(()) = renormalise_vector(block, BandGain(1.0));
      }
    }

    self.rng = seed;

    let Ok(()) = self.slide_all(size);
    let synthesis = Synthesis {
      size,
      start: self.start,
      end,
      coded: self.channels,
      transient: Bit::Zero,
      lm,
      silence: Bit::Zero,
    };
    let Ok(()) = self.synthesis(&x, &synthesis);

    self.x = x;

    Ok(())
  }

  fn pitch(&mut self, size: u32) -> Result<(), Never> {
    let Ok(each) = index(MEMORY);
    let Ok(history) = index(PLC_HISTORY);
    let Ok(base) = index(DECODE_BUFFER_SIZE.wrapping_sub(PLC_HISTORY));
    let mut mono = vec![0f32; history.min(1024)];

    for memory in self.decode_mem.chunks_exact(each.max(1)) {
      for (sum, value) in mono.iter_mut().zip(memory.iter().skip(base)) {
        *sum += *value;
      }
    }

    let period = match self.loss_count {
      0 => {
        let Ok(found) = find_pitch(&mono);

        found
      }
      _later => self
        .postfilter_old
        .period
        .max(PLC_PITCH_LAG_MIN.cast_signed())
        .cast_unsigned(),
    };
    let period_2 = period.clamp(PLC_PITCH_LAG_MIN, PLC_PITCH_LAG_MAX);

    self.postfilter_old.period = period_2.cast_signed();

    let Ok(lost) = fitted::<u32, i32>(self.loss_count.wrapping_add(1));
    let fade = 0.8f32.powi(lost);
    let Ok(extended) = index(size.wrapping_add(OVERLAP));
    let mut ext = vec![0f32; extended.min(1080)];
    let window = self.mode.window;

    for memory in self.decode_mem.chunks_exact_mut(each.max(1)) {
      let Ok(()) = extend(memory, &mut ext, period_2, LossFade(fade));
      let Ok(()) = slide(memory, size);
      let Ok(()) = lap(memory, &ext, size, &window);
    }

    Ok(())
  }
}

struct Synthesis {
  size: u32,
  start: u32,
  end: u32,
  coded: u32,
  transient: Bit,
  lm: u32,
  silence: Bit,
}

fn transform(
  mode: &Mode,
  memory: &mut [f32],
  spectrum: &[f32],
  asked: &Synthesis,
  scratch: &mut ImdctScratch,
) -> Result<(), Never> {
  let (blocks, each, shift) = match asked.transient {
    Bit::One => (1u32.wrapping_shl(asked.lm), SHORT_MDCT, MAX_LM),
    Bit::Zero => (
      1,
      SHORT_MDCT.wrapping_shl(asked.lm),
      MAX_LM.wrapping_sub(asked.lm),
    ),
  };
  let Ok(which) = index(shift);
  let imdct = match mode.mdct.get(which) {
    Some(imdct) => imdct,
    None => return Ok(()),
  };
  let out_from = DECODE_BUFFER_SIZE.wrapping_sub(asked.size);

  for block in 0..blocks {
    let Ok(from) = index(block);
    let Ok(to) = index(out_from.wrapping_add(each.wrapping_mul(block)));
    let bins = match spectrum.get(from..) {
      Some(bins) => bins,
      None => &[],
    };

    match memory.get_mut(to..) {
      Some(out) => {
        let Ok(()) = imdct.backward(
          Spectrum {
            bins,
            stride: blocks,
          },
          out,
          Lapping {
            window: &mode.window,
            overlap: OVERLAP,
          },
          scratch,
        );
      }
      None => {}
    }
  }

  Ok(())
}

fn energy_of(values: &[f32]) -> Result<f32, Never> {
  Ok(values.iter().map(|value| value * value).sum())
}

fn extend(memory: &[f32], ext: &mut [f32], period: u32, LossFade(fade): LossFade) -> Result<(), Never> {
  let Ok(end) = index(DECODE_BUFFER_SIZE);
  let Ok(one) = index(DECODE_BUFFER_SIZE.wrapping_sub(period));
  let Ok(two) = index(DECODE_BUFFER_SIZE.wrapping_sub(period.wrapping_mul(2)));
  let recent = match memory.get(one..end) {
    Some(recent) => recent,
    None => &[],
  };
  let earlier = match memory.get(two..one) {
    Some(earlier) => earlier,
    None => &[],
  };
  let Ok(e1) = energy_of(recent);
  let Ok(e0) = energy_of(earlier);
  let decay = match e0.partial_cmp(&0.0) {
    Some(Ordering::Greater) => (e1 / e0).sqrt().min(1.0),
    Some(Ordering::Less | Ordering::Equal) | None => 0.0,
  };
  let mut gain = 1.0f32;
  let Ok(cycle) = index(period);

  for (round, chunk) in (0u32..).zip(ext.chunks_mut(cycle.max(1))) {
    match round {
      0 => {}
      _later => gain *= decay,
    }

    for (slot, source) in chunk.iter_mut().zip(recent.iter()) {
      *slot = *source * gain * fade;
    }
  }

  Ok(())
}

fn lap(memory: &mut [f32], ext: &[f32], size: u32, window: &[f32; 120]) -> Result<(), Never> {
  let Ok(from) = index(DECODE_BUFFER_SIZE.wrapping_sub(size));
  let Ok(length) = index(size);
  let Ok(overlap) = index(OVERLAP);
  let Ok(half) = index(OVERLAP.wrapping_shr(1));
  let Ok(buffer) = index(DECODE_BUFFER_SIZE);

  for (slot, value) in memory.iter_mut().skip(from).take(length).zip(ext.iter()) {
    *slot = *value;
  }

  let tail = match ext.get(length..length.saturating_add(overlap)) {
    Some(tail) => tail,
    None => &[],
  };
  let pairs = window
    .iter()
    .zip(window.iter().rev())
    .zip(tail.iter().rev().zip(tail.iter()));

  for (slot, ((w1, w2), (t1, t2))) in memory.iter_mut().skip(buffer).take(half).zip(pairs) {
    *slot = *w1 * *t1 + *w2 * *t2;
  }

  Ok(())
}

fn find_pitch(x: &[f32]) -> Result<u32, Never> {
  let Ok(length) = fitted::<_, u32>(x.len());
  let window = 512u32.min(length.wrapping_sub(PLC_PITCH_LAG_MAX));
  let Ok(target_from) = index(length.wrapping_sub(window));
  let target = match x.get(target_from..) {
    Some(target) => target,
    None => &[],
  };
  let mut best = PLC_PITCH_LAG_MIN;
  let mut best_score = f32::MIN;
  let Ok(target_energy) = energy_of(target);
  let target_energy_2 = target_energy + 1e-9;

  for lag in PLC_PITCH_LAG_MIN..=PLC_PITCH_LAG_MAX {
    let Ok(from) = index(length.wrapping_sub(window).wrapping_sub(lag));
    let Ok(to) = index(length.wrapping_sub(lag));
    let source = match x.get(from..to) {
      Some(source) => source,
      None => &[],
    };
    let mut cross = 0f32;
    let mut energy = 1e-9f32;

    for (a, b) in target.iter().zip(source.iter()) {
      cross += a * b;
      energy += b * b;
    }

    let score = cross / (target_energy_2 * energy).sqrt();

    match score.partial_cmp(&best_score) {
      Some(Ordering::Greater) => {
        best_score = score;
        best = lag;
      }
      Some(Ordering::Less | Ordering::Equal) | None => {}
    }
  }

  Ok(best)
}

fn postfilter_header(
  dec: &mut RangeDecoder<'_>,
  total: i32,
  postfilter: &mut Tap,
) -> Result<(), Never> {
  let Ok(on) = dec.bit_logp(1);

  match on {
    Bit::Zero => Ok(()),
    Bit::One => {
      let Ok(octave) = dec.uint(6);
      let Ok(fine) = dec.bits(4u32.wrapping_add(octave));
      let Ok(gain) = dec.bits(3);

      postfilter.period = 16u32
        .wrapping_shl(octave)
        .wrapping_add(fine)
        .cast_signed()
        .wrapping_sub(1);

      let Ok(told) = dec.tell();

      match told.wrapping_add(2).cmp(&total) {
        Ordering::Less | Ordering::Equal => {
          let Ok(tapset) = dec.symbol(&TAPSET, 2);

          postfilter.tapset = tapset;
        }
        Ordering::Greater => {}
      }

      let Ok(steps) = float(gain.wrapping_add(1));

      postfilter.gain = 0.09375 * steps;

      Ok(())
    }
  }
}

fn spread(dec: &mut RangeDecoder<'_>, coded: &Coded) -> Result<Spread, Never> {
  let total = coded.length.wrapping_mul(8).cast_signed();
  let Ok(told) = dec.tell();

  match told.wrapping_add(4).cmp(&total) {
    Ordering::Less | Ordering::Equal => {
      let Ok(symbol) = dec.symbol(&SPREAD, 5);

      Spread::from_symbol(symbol)
    }
    Ordering::Greater => Ok(Spread::Normal),
  }
}

fn tf_select_entry(
  crate::quantities::BlockOrder(lm): crate::quantities::BlockOrder,
  crate::quantities::TimeFrequencyColumn(column): crate::quantities::TimeFrequencyColumn,
) -> Result<i32, Never> {
  entry(&TF_SELECT_TABLE, Place { row: lm, column })
}

fn tf_decode(
  layout: Layout,
  transient: Bit,
  tf_res: &mut [i32; 21],
  lm: u32,
  dec: &mut RangeDecoder<'_>,
) -> Result<(), Never> {
  let Ok(storage) = dec.storage();
  let mut budget = storage.wrapping_mul(8).cast_signed();
  let Ok(mut tell) = dec.tell();
  let (first_logp, later_logp, column) = match transient {
    Bit::One => (2u32, 4u32, 4u32),
    Bit::Zero => (4, 5, 0),
  };
  let mut logp = first_logp;

  enum SelectionBit {
    Reserved,
    Unavailable,
  }

  let selection = match (lm, tell.wrapping_add(logp.cast_signed()).cmp(&budget)) {
    (1.., Ordering::Less) => {
      budget = budget.wrapping_sub(1);

      SelectionBit::Reserved
    }
    _no_room => SelectionBit::Unavailable,
  };

  let mut changed = 0u32;
  let mut current = 0u32;
  let Ok(from) = index(layout.start);
  let Ok(count) = index(layout.end.saturating_sub(layout.start));

  for slot in tf_res.iter_mut().skip(from).take(count) {
    match tell.wrapping_add(logp.cast_signed()).cmp(&budget) {
      Ordering::Less | Ordering::Equal => {
        let Ok(bit) = dec.bit_logp(logp);
        let Ok(told) = dec.tell();

        current ^= match bit {
          Bit::One => 1,
          Bit::Zero => 0,
        };
        tell = told;
        changed |= current;
      }
      Ordering::Greater => {}
    }

    *slot = current.cast_signed();
    logp = later_logp;
  }

  let Ok(plain) = tf_select_entry(
    crate::quantities::BlockOrder(lm),
    crate::quantities::TimeFrequencyColumn(column.wrapping_add(changed)),
  );
  let Ok(selected) = tf_select_entry(
    crate::quantities::BlockOrder(lm),
    crate::quantities::TimeFrequencyColumn(column.wrapping_add(2).wrapping_add(changed)),
  );
  let select = match (selection, plain.cmp(&selected)) {
    (SelectionBit::Reserved, Ordering::Less | Ordering::Greater) => {
      let Ok(bit) = dec.bit_logp(1);

      match bit {
        Bit::One => 1u32,
        Bit::Zero => 0,
      }
    }
    (SelectionBit::Reserved, Ordering::Equal) | (SelectionBit::Unavailable, _) => 0,
  };

  for slot in tf_res.iter_mut().skip(from).take(count) {
    let Ok(entry) = tf_select_entry(
      crate::quantities::BlockOrder(lm),
      crate::quantities::TimeFrequencyColumn(
        column
          .wrapping_add(select.wrapping_mul(2))
          .wrapping_add(slot.cast_unsigned()),
      ),
    );

    *slot = entry;
  }

  Ok(())
}

struct Comb {
  offset: u32,
  length: u32,
  from: Tap,
  to: Tap,
}

fn gains(tap: Tap) -> Result<[f32; 3], Never> {
  let Ok(row) = index(tap.tapset);
  let weights = match COMB_GAINS.get(row) {
    Some(weights) => *weights,
    None => [0.0; 3],
  };

  Ok(weights.map(|weight| tap.gain * weight))
}

fn comb_filter(buffer: &mut [f32], comb: &Comb, window: &[f32; 120]) -> Result<(), Never> {
  match (comb.from.gain.classify(), comb.to.gain.classify()) {
    (FpCategory::Zero, FpCategory::Zero) => return Ok(()),
    _has_gain => {}
  }

  let t0 = comb.from.period.max(COMBFILTER_MINPERIOD).cast_unsigned();
  let t1 = comb.to.period.max(COMBFILTER_MINPERIOD).cast_unsigned();
  let Ok([g00, g01, g02]) = gains(comb.from);
  let Ok([g10, g11, g12]) = gains(comb.to);
  let cells = Cell::from_mut(buffer).as_slice_of_cells();
  let start = comb.offset;
  let Ok(mut x1) = sample(cells, start.wrapping_add(1).wrapping_sub(t1));
  let Ok(mut x2) = sample(cells, start.wrapping_sub(t1));
  let Ok(mut x3) = sample(cells, start.wrapping_sub(t1).wrapping_sub(1));
  let Ok(mut x4) = sample(cells, start.wrapping_sub(t1).wrapping_sub(2));
  let overlap = match (
    comb.from.gain.partial_cmp(&comb.to.gain),
    t0.cmp(&t1),
    comb.from.tapset.cmp(&comb.to.tapset),
  ) {
    (Some(Ordering::Equal), Ordering::Equal, Ordering::Equal) => 0,
    _changed => OVERLAP.min(comb.length),
  };

  for (place, weight) in (0..overlap).zip(window.iter()) {
    let at = start.wrapping_add(place);
    let Ok(x0) = sample(cells, at.wrapping_add(2).wrapping_sub(t1));
    let fading = weight * weight;
    let kept = 1.0 - fading;
    let Ok(here) = sample(cells, at);
    let Ok(centre) = sample(cells, at.wrapping_sub(t0));
    let Ok(after) = sample(cells, at.wrapping_add(1).wrapping_sub(t0));
    let Ok(before) = sample(cells, at.wrapping_sub(t0).wrapping_sub(1));
    let Ok(far_after) = sample(cells, at.wrapping_add(2).wrapping_sub(t0));
    let Ok(far_before) = sample(cells, at.wrapping_sub(t0).wrapping_sub(2));
    let near = after + before;
    let far = far_after + far_before;
    let value = here
      + kept * g00 * centre
      + kept * g01 * near
      + kept * g02 * far
      + fading * g10 * x2
      + fading * g11 * (x1 + x3)
      + fading * g12 * (x0 + x4);
    let Ok(slot) = index(at);

    match cells.get(slot) {
      Some(cell) => cell.set(value),
      None => {}
    }

    x4 = x3;
    x3 = x2;
    x2 = x1;
    x1 = x0;
  }

  match comb.to.gain.classify() {
    FpCategory::Zero => Ok(()),
    FpCategory::Normal | FpCategory::Subnormal | FpCategory::Infinite | FpCategory::Nan => {
      steady(cells, comb, overlap, (t1, [g10, g11, g12]))
    }
  }
}

fn steady(
  cells: &[Cell<f32>],
  comb: &Comb,
  overlap: u32,
  taps: (u32, [f32; 3]),
) -> Result<(), Never> {
  let (period, [g10, g11, g12]) = taps;

  for place in overlap..comb.length {
    let at = comb.offset.wrapping_add(place);
    let Ok(here) = sample(cells, at);
    let Ok(centre) = sample(cells, at.wrapping_sub(period));
    let Ok(after) = sample(cells, at.wrapping_add(1).wrapping_sub(period));
    let Ok(before) = sample(cells, at.wrapping_sub(period).wrapping_sub(1));
    let Ok(far_after) = sample(cells, at.wrapping_add(2).wrapping_sub(period));
    let Ok(far_before) = sample(cells, at.wrapping_sub(period).wrapping_sub(2));
    let value = here + g10 * centre + g11 * (after + before) + g12 * (far_after + far_before);
    let Ok(slot) = index(at);

    match cells.get(slot) {
      Some(cell) => cell.set(value),
      None => {}
    }
  }

  Ok(())
}
