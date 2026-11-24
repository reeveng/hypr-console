#[cfg(all(test, target_arch = "x86_64", feature = "bitdepth_8"))]
mod mask_parity_tests {
  use super::*;
  use crate::src::loopfilter::{ScalarFilterFixture, loop_filter_scalar_for_test};
  use archmage::SimdToken;
  use console_core_geometry::Point;
  use console_core_never::Never;
  use console_core_number_conversion::{index, indices, wrapped};

  const KERNELS: [(i32, HV, u32); 16] = [
    (6, HV::V, 4), (8, HV::V, 4), (8, HV::V, 8), (16, HV::V, 8),
    (16, HV::V, 16), (16, HV::V, 4), (4, HV::H, 4), (6, HV::H, 4),
    (8, HV::H, 4), (8, HV::H, 8), (16, HV::H, 4), (4, HV::V, 4),
    (4, HV::V, 8), (6, HV::V, 8), (6, HV::H, 8), (16, HV::H, 8),
  ];

  #[derive(Clone, Copy)]
  enum Kernel {
    Avx2 { token: Desktop64, kind: u8 },
    Avx512(Server64),
  }

  fn limits([edge, interior, variance]: [u8; 3]) -> Result<FilterLevels, Never> {
    Ok(FilterLevels { edge, interior, variance })
  }

  fn run_simd(kernel: Kernel, pixels: &mut [u8], window: PackedWindow, levels: [u8; 3]) -> Rav1dResult {
    let Ok(strength) = limits(levels);

    match kernel {
      Kernel::Avx512(token) => loop_filter_4_8bpc_wd16_simd_v_x16(token, pixels, window, strength),
      Kernel::Avx2 { token, kind } => match kind {
        0 => loop_filter_4_8bpc_wd6_simd_v(token, pixels, window, strength),
        1 => loop_filter_4_8bpc_wd8_simd_v(token, pixels, window, strength),
        2 => loop_filter_4_8bpc_wd8_simd_v_x8(token, pixels, window, strength),
        3 => loop_filter_4_8bpc_wd16_simd_v_x8(token, pixels, window, strength),
        5 => loop_filter_4_8bpc_wd16_simd_v(token, pixels, window, strength),
        6 => loop_filter_4_8bpc_narrow_simd(token, pixels, window, strength, HV::H),
        7 => loop_filter_4_8bpc_wd6_simd_h(token, pixels, window, strength),
        8 => loop_filter_4_8bpc_wd8_simd_h(token, pixels, window, strength),
        9 => loop_filter_4_8bpc_wd8_simd_h_x8(token, pixels, window, strength),
        10 => loop_filter_4_8bpc_wd16_simd_h(token, pixels, window, strength),
        11 => loop_filter_4_8bpc_narrow_simd(token, pixels, window, strength, HV::V),
        12 => loop_filter_4_8bpc_narrow_simd_v_x8(token, pixels, window, strength),
        13 => packed6::apply::<false>(token, pixels, window, levels),
        14 => packed6::apply::<true>(token, pixels, window, levels),
        15 => packed16::apply_h(token, pixels, window, levels),
        _unknown => Err(Rav1dError::EINVAL),
      },
    }
  }

  fn next(state: &mut u64) -> Result<u8, Never> {
    *state ^= state.wrapping_shr(12);
    *state ^= state.wrapping_shl(25);
    *state ^= state.wrapping_shr(27);
    wrapped(state.wrapping_mul(0x2545_f491_4f6c_dd1d))
  }

  #[derive(Clone, Copy, Debug)]
  enum Pattern {
    Alternating,
    Plateau,
    OuterStep,
    InnerStep,
    PlateauFirst,
    PlateauOdd,
    Gradient,
    NearBlack,
    NearWhite,
    Noise,
    PositiveRounding,
    NegativeRounding,
  }

  impl Pattern {
    const ALL: [Self; 12] = [
      Self::Alternating, Self::Plateau, Self::OuterStep, Self::InnerStep,
      Self::PlateauFirst, Self::PlateauOdd, Self::Gradient, Self::NearBlack,
      Self::NearWhite, Self::Noise, Self::PositiveRounding, Self::NegativeRounding,
    ];

    fn sample(self, position: Point<i64>, rng: &mut u64) -> Result<u8, Never> {
      let Point { x: tap, y: lane } = position;
      let side = match tap {
        ..=-1 => 64_u8,
        0.. => 68,
      };
      let plateau = match self {
        Self::PlateauFirst => lane == 0,
        Self::PlateauOdd => lane & 1 == 1,
        _other @ (Self::Alternating | Self::Plateau | Self::OuterStep | Self::InnerStep | Self::Gradient | Self::NearBlack | Self::NearWhite | Self::Noise | Self::PositiveRounding | Self::NegativeRounding) => false,
      };
      let pattern = match (self, plateau) {
        (Self::PlateauFirst | Self::PlateauOdd, true) => Self::Plateau,
        (Self::PlateauFirst | Self::PlateauOdd, false) => Self::Alternating,
        (value, _) => value,
      };

      match pattern {
        Self::Alternating | Self::PlateauFirst | Self::PlateauOdd => {
          Ok(match tap & 1 { 0 => 0, _odd => 255 })
        }
        Self::Plateau => Ok(side),
        Self::OuterStep => Ok(match tap {
          -4..=3 => side,
          _outer => side.wrapping_add(12),
        }),
        Self::InnerStep => Ok(match tap {
          -3 | 2 => side.wrapping_add(4),
          _other => side,
        }),
        Self::Gradient => wrapped(tap.wrapping_mul(37).wrapping_add(lane.wrapping_mul(53))),
        Self::NearBlack => Ok(match tap { ..=-1 => 0, 0.. => 4 }),
        Self::NearWhite => Ok(match tap { ..=-1 => 251, 0.. => 255 }),
        Self::Noise => next(rng),
        Self::PositiveRounding => Ok(match tap { ..=0 => 0, 1.. => 8 }),
        Self::NegativeRounding => Ok(match tap { ..=-2 => 247, -1.. => 255 }),
      }
    }
  }

  #[derive(Clone, Copy)]
  struct Input {
    direction: HV,
    lanes: u32,
    stride: i64,
    offset: u64,
    pattern: Pattern,
  }

  struct Pixels {
    samples: Vec<u8>,
    window: FilterWindow,
  }

  fn input(case: Input) -> Rav1dResult<Pixels> {
    let stride = case.stride;
    let pitch = stride.unsigned_abs();
    let lanes = u64::from(case.lanes);
    let anchor_rows = match (case.direction, stride) {
      (HV::H, ..=-1) => lanes.wrapping_add(8),
      _forward => 16,
    };
    let base = pitch.wrapping_mul(anchor_rows).wrapping_add(8).wrapping_add(case.offset);
    let (along, across) = match case.direction {
      HV::H => (stride, 1),
      HV::V => (1, stride),
    };
    let window = FilterWindow { base, along, tap_stride: across };
    let mut rng = 0x0dec_afba_df00_d123;
    let count = pitch.wrapping_mul(lanes.wrapping_add(32)).wrapping_add(64);
    let mut samples = Vec::new();

    for _ in 0..count {
      let Ok(value) = next(&mut rng);
      samples.push(value);
    }

    for y in 0..i64::from(case.lanes) {
      for x in -7_i64..=6 {
        let point = Point { x, y };
        let displacement = y.wrapping_mul(along).wrapping_add(x.wrapping_mul(across));
        let address = base.checked_add_signed(displacement).ok_or(Rav1dError::EINVAL)?;
        let Ok(position) = index(address);
        let sample = samples.get_mut(position).ok_or(Rav1dError::EINVAL)?;
        let Ok(value) = case.pattern.sample(point, &mut rng);
        *sample = value;
      }
    }

    Ok(Pixels { samples, window })
  }

  #[derive(Clone, Copy)]
  struct MaskFixture {
    kernel: Kernel,
    ordinal: u8,
    width: i32,
    input: Input,
  }

  fn compare_masks(case: MaskFixture, levels: [u8; 3]) -> Rav1dResult {
    let Pixels { samples, window } = input(case.input)?;
    let mut actual = samples.clone();
    let mut expected = samples.clone();
    let view = PackedWindow { anchor: window.base, stride: case.input.stride };
    let Ok(strength) = limits(levels);
    run_simd(case.kernel, &mut actual, view, levels)?;
    loop_filter_scalar_for_test(&mut expected, ScalarFilterFixture {
      window, lanes: case.input.lanes, limits: strength, width: case.width,
    })?;
    assert_eq!(
      actual.iter().zip(&expected).position(|(a, b)| a != b), None,
      "kernel={} width={} lanes={} stride={} offset={} pattern={:?} levels={levels:?}",
      case.ordinal, case.width, case.input.lanes, case.input.stride, case.input.offset, case.input.pattern,
    );

    match (case.input.pattern, levels) {
      (Pattern::Alternating, _) => assert_eq!(actual, samples),
      (Pattern::Plateau | Pattern::PlateauFirst | Pattern::PlateauOdd
        | Pattern::PositiveRounding | Pattern::NegativeRounding, [16, 8, 1]) => assert_ne!(actual, samples),
      _unchanged_expectation => {}
    }

    Ok(())
  }

  #[test]
  fn test_loopfilter_masks_match_scalar() -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let avx2 = Desktop64::summon();
    let avx512 = Server64::summon();
    let mut cells = 0_u32;

    'kernels: for (ordinal, (width, direction, lanes)) in (0_u8..).zip(KERNELS) {
      let kernel = match (ordinal, avx2, avx512) {
        (4, _, Some(token)) => Kernel::Avx512(token),
        (4, _, None) => continue 'kernels,
        (kind, Some(token), _) => Kernel::Avx2 { token, kind },
        (_kind, None, _) => continue 'kernels,
      };

      for stride in [32, 67, -32, -67] {
        for offset in [0, 3] {
          for pattern in Pattern::ALL {
            let geometry = Input { direction, lanes, stride, offset, pattern };
            let scenario = MaskFixture { kernel, ordinal, width, input: geometry };

            for levels in [[0, 0, 0], [8, 4, 0], [16, 8, 1], [63, 63, 3], [255, 255, 255]] {
              compare_masks(scenario, levels)?;
              cells = cells.wrapping_add(1);
            }
          }
        }
      }
    }

    let supported = u32::from(avx2.is_some()).wrapping_mul(15).wrapping_add(u32::from(avx512.is_some()));
    assert_eq!(cells, supported.wrapping_mul(480));
    eprintln!("loopfilter mask sweep: {cells} live SIMD cells match the scalar decoder");
    Ok(())
  }

  #[derive(Clone, Copy)]
  enum PackedFixture {
    Chroma,
    Luma,
  }

  const LEVEL_BASE: u64 = 64;
  const LEVEL_PITCH: u64 = 37;
  const LEVEL_SAMPLES: u16 = 4996;

  #[derive(Clone, Copy)]
  struct Grouping {
    fixture: PackedFixture,
    direction: HV,
    pitch: i64,
    component: u64,
    case: u8,
  }

  fn group_mask(fixture: Grouping) -> Rav1dResult<([u32; 3], u64)> {
    let (fused, mask) = match fixture.case {
      0 => (1, 0b11),
      1 => (0, 0b11),
      2 => (0, 0b01),
      3 => (0, 0b101),
      4 => (1, 0b11),
      5 => (0, 0b11),
      6 => (1, 3_u32.wrapping_shl(30)),
      7 => (0, 1_u32.wrapping_shl(31)),
      _invalid => return Err(Rav1dError::EINVAL),
    };
    let weak = match fixture.case { 2 => 0b10, _otherwise => 0 };

    Ok((match fixture.fixture {
      PackedFixture::Chroma => [weak, mask, 0],
      PackedFixture::Luma => [0, weak, mask],
    }, fused))
  }

  fn level_sample(LevelSampleArguments { block, component }: LevelSampleArguments) -> Result<u64, Never> {
    Ok(block.wrapping_mul(4).wrapping_add(component))
  }

  struct LevelSampleArguments {
    block: u64,
    component: u64,
  }

  fn set_level(levels: &mut [u8], address: u64, value: u8) -> Rav1dResult {
    let Ok(position) = index(address);
    let level = levels.get_mut(position).ok_or(Rav1dError::EINVAL)?;
    *level = value;
    Ok(())
  }

  struct CachedLevels {
    raw: [u8; match console_core_number_conversion::const_u16_index(LEVEL_SAMPLES) { Ok(length) => length }],
    step: u64,
    lookback: u64,
  }

  fn group_levels(case: Grouping) -> Rav1dResult<CachedLevels> {
    let (step, lookback) = match case.direction {
      HV::H => (LEVEL_PITCH, 1),
      HV::V => (1, LEVEL_PITCH),
    };
    let mut raw = [32_u8; _];
    let Ok(first) = level_sample(self::LevelSampleArguments { block: LEVEL_BASE, component: case.component });
    let Ok(second) = level_sample(self::LevelSampleArguments { block: LEVEL_BASE.wrapping_add(step), component: case.component });
    let Ok(previous) = level_sample(self::LevelSampleArguments { block: LEVEL_BASE.wrapping_sub(lookback), component: case.component });

    match case.case {
      1 => set_level(&mut raw, second, 33)?,
      4 => set_level(&mut raw, second, 0)?,
      5 => {
        set_level(&mut raw, first, 0)?;
        set_level(&mut raw, previous, 0)?;
      }
      _unchanged => {}
    }

    Ok(CachedLevels { raw, step, lookback })
  }

  fn cached_level(cache: &CachedLevels, ordinal: u32, component: u64) -> Rav1dResult<u8> {
    let block = LEVEL_BASE.wrapping_add(u64::from(ordinal).wrapping_mul(cache.step));
    let Ok(address) = level_sample(self::LevelSampleArguments { block, component });
    let Ok(position) = index(address);
    let &current = cache.raw.get(position).ok_or(Rav1dError::EINVAL)?;

    match current {
      0 => {
        let Ok(previous) = level_sample(self::LevelSampleArguments { block: block.wrapping_sub(cache.lookback), component });
        let Ok(position_2) = index(previous);
        cache.raw.get(position_2).copied().ok_or(Rav1dError::EINVAL)
      }
      found => Ok(found),
    }
  }

  fn filter_reference(
    expected: &mut [u8],
    window: FilterWindow,
    cache: &CachedLevels,
    case: Grouping,
    table: &Av1FilterLUT,
    mask: [u32; 3],
  ) -> Rav1dResult {
    let [weak, middle, wide] = mask;
    let chosen = weak | middle | wide;

    'blocks: for bit in 0_u32..32 {
      let selected = 1_u32.wrapping_shl(bit);

      match chosen & selected {
        0 => continue 'blocks,
        _present => {}
      }

      let level = cached_level(cache, bit, case.component)?;

      match level {
        0 => continue 'blocks,
        _nonzero => {}
      }

      let width = match (case.fixture, middle & selected, wide & selected) {
        (PackedFixture::Chroma, 0, _) => 4,
        (PackedFixture::Chroma, _, _) => 6,
        (PackedFixture::Luma, _, 0) => 8,
        (PackedFixture::Luma, _, _) => 16,
      };
      let Ok(position) = index(level);
      let &edge = table.e.get(position).ok_or(Rav1dError::EINVAL)?;
      let &interior = table.i.get(position).ok_or(Rav1dError::EINVAL)?;
      let strength = FilterLevels { edge, interior, variance: level.wrapping_shr(4) };
      let displacement = i64::from(bit).wrapping_mul(4).wrapping_mul(window.along);
      let base = window.base.checked_add_signed(displacement).ok_or(Rav1dError::EINVAL)?;
      let cell = FilterWindow { base, ..window };
      loop_filter_scalar_for_test(expected, ScalarFilterFixture {
        window: cell, lanes: 4, limits: strength, width,
      })?;
    }

    Ok(())
  }

  fn group_calls(case: Grouping) -> Result<u64, Never> {
    match case.fixture {
      PackedFixture::Luma => packed16::calls(),
      PackedFixture::Chroma => {
        let Ok([vertical, horizontal]) = packed6::calls();
        Ok(match case.direction { HV::H => horizontal, HV::V => vertical })
      }
    }
  }

  fn compare_grouping(token: Desktop64, case: Grouping, lut: &Av1FilterLUT) -> Rav1dResult {
    let (mask, expected_calls) = group_mask(case)?;
    let cache = group_levels(case)?;
    let levels: Vec<_> = cache.raw.iter().copied().map(AtomicU8::new).collect();
    let shape = Input { direction: case.direction, lanes: 128, stride: case.pitch, offset: 3, pattern: Pattern::PlateauOdd };
    let Pixels { samples, window } = input(shape)?;
    let mut actual = samples.clone();
    let mut expected = samples;
    let plane = match case.fixture { PackedFixture::Chroma => YUV::UV, PackedFixture::Luma => YUV::Y };
    let tile = FilterTile {
      pixel_base: window.base, pixel_stride: case.pitch, levels: &levels,
      level_base: LEVEL_BASE, level_stride: LEVEL_PITCH.cast_signed(), level_component: case.component,
      lut, masks: &mask, plane, direction: case.direction,
    };
    let Ok(before) = group_calls(case);
    let Ok(()) = filter_tile_8bpc(token, &mut actual, tile, 255);
    let Ok(after) = group_calls(case);
    assert_eq!(after.wrapping_sub(before), expected_calls, "fusion {:?} case {}", plane, case.case);
    filter_reference(&mut expected, window, &cache, case, lut, mask)?;
    assert_eq!(
      actual.iter().zip(&expected).position(|(a, b)| a != b), None,
      "grouping {:?} pitch={} component={} case={}", case.direction, case.pitch, case.component, case.case,
    );

    Ok(())
  }

  fn sweep_grouping(token: Desktop64, fixture: PackedFixture) -> Rav1dResult<u32> {
    let mut lut: Align16<Av1FilterLUT> = crate::src::align::ArrayDefault::default();
    lut.e.fill(16);
    lut.i.fill(8);
    let over = lut.e.get_mut(33).ok_or(Rav1dError::EINVAL)?;
    *over = 32;
    let directions: &[HV] = match fixture {
      PackedFixture::Chroma => &[HV::V, HV::H],
      PackedFixture::Luma => &[HV::H],
    };
    let components = match fixture { PackedFixture::Chroma => [2, 3], PackedFixture::Luma => [0, 1] };
    let mut cells = 0_u32;

    for &direction in directions {
      for pitch in [160, -160] {
        for component in components {
          for case in 0..8 {
            compare_grouping(token, Grouping { fixture, direction, pitch, component, case }, &lut)?;
            cells = cells.wrapping_add(1);
          }
        }
      }
    }

    Ok(cells)
  }

  #[test]
  fn test_loopfilter_packed6_production_grouping() -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    match avx2 {
      Some(token) => {
        let cells = sweep_grouping(token, PackedFixture::Chroma)?;
        assert_eq!(cells, 64);
        eprintln!("packed six-tap grouping: {cells} production-path cases match scalar");
      }
      None => {}
    }

    Ok(())
  }

  #[test]
  fn test_loopfilter_packed16_production_grouping() -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    match avx2 {
      Some(token) => {
        let cells = sweep_grouping(token, PackedFixture::Luma)?;
        assert_eq!(cells, 32);
        eprintln!("packed wide H grouping: {cells} production-path cases match scalar");
      }
      None => {}
    }

    Ok(())
  }

  fn compare_exact_span(token: Desktop64, shape: Input) -> Rav1dResult {
    let Pixels { samples, window } = input(shape)?;
    let other = window.base.checked_add_signed(7_i64.wrapping_mul(shape.stride)).ok_or(Rav1dError::EINVAL)?;
    let start = window.base.min(other).checked_sub(7).ok_or(Rav1dError::EINVAL)?;
    let end = window.base.max(other).wrapping_add(7);
    let Ok(full) = indices(start..end);
    let Ok(truncated) = indices(start..end.wrapping_sub(1));
    let levels = [16, 8, 1];
    let Ok(strength) = limits(levels);
    let mut actual = samples.clone();
    let mut expected = samples.clone();
    let mut short = samples.clone();
    loop_filter_scalar_for_test(&mut expected, ScalarFilterFixture {
      window, lanes: 8, limits: strength, width: 16,
    })?;
    let anchor = window.base.wrapping_sub(start);
    let view = PackedWindow { anchor, stride: shape.stride };
    let all = actual.get_mut(full).ok_or(Rav1dError::EINVAL)?;
    packed16::apply_h(token, all, view, levels)?;
    assert_eq!(actual, expected, "exact span: pitch={} offset={} pattern={:?}", shape.stride, shape.offset, shape.pattern);
    assert_ne!(actual, samples);
    let missing = short.get_mut(truncated).ok_or(Rav1dError::EINVAL)?;
    let rejected = packed16::apply_h(token, missing, view, levels);
    assert_eq!(rejected, Err(Rav1dError::EINVAL));
    assert_eq!(short, samples);
    Ok(())
  }

  #[test]
  fn test_loopfilter_packed16_exact_span() -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let Ok(avx2) = crate::src::cpu::summon_avx2();
    let token = match avx2 {
      Some(matched) => matched,
      None => return Ok(()),
    };
    let mut cells = 0_u32;

    for stride in [14, -14, 19, -19] {
      for offset in [0, 3] {
        for pattern in [Pattern::Plateau, Pattern::InnerStep, Pattern::PositiveRounding, Pattern::NegativeRounding] {
          let shape = Input { direction: HV::H, lanes: 8, stride, offset, pattern };
          compare_exact_span(token, shape)?;
          cells = cells.wrapping_add(1);
        }
      }
    }

    assert_eq!(cells, 32);
    eprintln!("packed wide H footprint: {cells} exact spans and {cells} short-view rejections");
    Ok(())
  }
}
