//! Support for rendering variable composite glyphs from the VARC table.

use alloc::vec;
use alloc::vec::Vec;
use read_fonts::{
  FontRef, ReadError, TableProvider,
  tables::{
    layout::Condition,
    varc::{
      DecomposedTransform, MultiItemVariationStore, SparseVariationRegionList, Varc,
      VarcComponent, VarcFlags,
    },
    variations::NO_VARIATION_INDEX,
  },
  types::{F2Dot14, GlyphId, Matrix},
};

use crate::{
  GLYF_COMPOSITE_RECURSION_LIMIT,
  instance::Size,
  outline::{DrawError, OutlinePen, cff, glyf, metrics::GlyphHMetrics, pen::PathStyle},
  provider::MetadataProvider,
};

use super::OutlineKind;

type GlyphStack = Vec<GlyphId>;
type CoordVec = Vec<F2Dot14>;
type AxisIndexVec = Vec<u16>;
type AxisValueVec = Vec<f32>;
type DeltaVec = Vec<f32>;
type ScalarCacheVec = Vec<f32>;
type Affine = Matrix<f32>;

struct Scratchpad {
  deltas: DeltaVec,
  axis_indices: AxisIndexVec,
  axis_values: AxisValueVec,
}

impl Scratchpad {
  fn new() -> Self {
    Self {
      deltas: DeltaVec::new(),
      axis_indices: AxisIndexVec::new(),
      axis_values: AxisValueVec::new(),
    }
  }
}

struct VarcSharedContext<'a, 'b> {
  font_coords: &'b [F2Dot14],
  size: Size,
  path_style: PathStyle,
  coverage: &'b read_fonts::tables::layout::CoverageTable<'a>,
  var_store: Option<&'b MultiItemVariationStore<'a>>,
  store_regions: Option<(
    &'b MultiItemVariationStore<'a>,
    &'b SparseVariationRegionList<'a>,
  )>,
}

#[derive(Clone)]
enum BaseOutlines<'a> {
  Glyf(glyf::Outlines<'a>),
  Cff(cff::Outlines<'a>),
}

impl<'a> BaseOutlines<'a> {
  fn glyph_count(&self) -> u32 {
    match self {
      Self::Glyf(glyf) => {
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u32>(glyf.glyph_count());
          migrated_value
        })
      }
      Self::Cff(cff) => {
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u32>(cff.glyph_count());
          migrated_value
        })
      }
    }
  }

  fn font(&self) -> &FontRef<'a> {
    match self {
      Self::Glyf(glyf) => &glyf.font,
      Self::Cff(cff) => &cff.font,
    }
  }

  fn base_outline_kind(&self, glyph_id: GlyphId) -> Option<OutlineKind<'a>> {
    match self {
      Self::Glyf(glyf) => Some(OutlineKind::Glyf(
        glyf.clone(),
        glyf.outline(glyph_id).ok()?,
      )),
      Self::Cff(cff) => Some(OutlineKind::Cff(
        cff.clone(),
        glyph_id,
        cff.subfont_index(glyph_id),
      )),
    }
  }

  fn base_outline_memory(&self, glyph_id: GlyphId) -> usize {
    match self {
      Self::Glyf(glyf) => glyf
        .outline(glyph_id)
        .ok()
        .map(|outline| outline.required_buffer_size(super::Hinting::None))
        .unwrap_or(0),
      Self::Cff(..) => 0,
    }
  }
}

#[derive(Clone)]
pub(crate) struct Outlines<'a> {
  varc: Varc<'a>,
  coverage: read_fonts::tables::layout::CoverageTable<'a>,
  var_store: Option<MultiItemVariationStore<'a>>,
  regions: Option<SparseVariationRegionList<'a>>,
  base: BaseOutlines<'a>,
  glyph_metrics: GlyphHMetrics<'a>,
  units_per_em: u16,
  axis_count: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct Outline {
  pub(crate) glyph_id: GlyphId,
  pub(crate) coverage_index: u16,
  max_component_memory: usize,
}

impl Outline {
  pub fn required_buffer_size(&self) -> usize {
    self.max_component_memory
  }
}

impl<'a> Outlines<'a> {
  pub fn new(font: &FontRef<'a>) -> Option<Self> {
    let varc = font.varc().ok()?;
    match glyf::Outlines::new(font) {
      Some(glyf) => {
        return Self::from_base(font, varc, BaseOutlines::Glyf(glyf));
      }
      None => {}
    }
    match cff::Outlines::new(font) {
      Some(cff) => {
        return Self::from_base(font, varc, BaseOutlines::Cff(cff));
      }
      None => {}
    }
    None
  }

  fn from_base(font: &FontRef<'a>, varc: Varc<'a>, base: BaseOutlines<'a>) -> Option<Self> {
    let glyph_metrics = GlyphHMetrics::new(font)?;
    let units_per_em = ({
      let Ok(value) = font.head().ok()?.units_per_em();
      value
    });
    let axis_count = ({
      let value = font.axes();
      value
    })
    .len();
    let coverage = varc.coverage().ok()?;
    let var_store = varc.multi_var_store().ok()?;
    let regions = var_store
      .as_ref()
      .map(|s| s.region_list())
      .transpose()
      .ok()?;
    Some(Self {
      varc,
      coverage,
      var_store,
      regions,
      base,
      glyph_metrics,
      units_per_em: (units_per_em),
      axis_count,
    })
  }

  pub fn units_per_em(&self) -> u16 {
    self.units_per_em
  }

  pub fn glyph_count(&self) -> u32 {
    self.base.glyph_count()
  }

  pub fn prefer_interpreter(&self) -> bool {
    false
  }

  pub fn fractional_size_hinting(&self) -> bool {
    false
  }

  pub fn font(&self) -> &FontRef<'a> {
    self.base.font()
  }

  pub(crate) fn fallback_outline_kind(&self, glyph_id: GlyphId) -> Option<OutlineKind<'a>> {
    self.base.base_outline_kind(glyph_id)
  }

  pub fn outline(&self, glyph_id: GlyphId) -> Result<Option<Outline>, ReadError> {
    let coverage_index = match (self.coverage.get(glyph_id)) {
      Some(coverage_index) => coverage_index,
      None => {
        return Ok(None);
      }
    };
    let max_component_memory = self.compute_max_component_memory(glyph_id, coverage_index)?;
    Ok(Some(Outline {
      glyph_id,
      coverage_index,
      max_component_memory,
    }))
  }

  /// Lightweight coverage lookup without computing max_component_memory.
  fn coverage_index(&self, glyph_id: GlyphId) -> Result<Option<u16>, ReadError> {
    Ok((self.coverage.get(glyph_id)))
  }

  fn compute_max_component_memory(
    &self,
    glyph_id: GlyphId,
    coverage_index: u16,
  ) -> Result<usize, ReadError> {
    let mut stack = GlyphStack::new();
    self.max_component_memory_for_glyph(glyph_id, coverage_index, &mut stack)
  }

  fn max_component_memory_for_glyph(
    &self,
    glyph_id: GlyphId,
    coverage_index: u16,
    stack: &mut GlyphStack,
  ) -> Result<usize, ReadError> {
    match stack.contains(&glyph_id) {
      true => {
        return Ok(0);
      }
      false => {}
    }
    match stack.len() >= GLYF_COMPOSITE_RECURSION_LIMIT {
      true => {
        return Ok(0);
      }
      false => {}
    }
    stack.push(glyph_id);
    let mut max_memory = 0usize;
    let glyph = self.varc.glyph(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index(coverage_index);
            migrated_value
          }),
        );
        value
      }),
    )?;
    for component in glyph.components() {
      let component = component?;
      let component_gid = component.gid();
      let component_memory = match component_gid == glyph_id {
        true => self.base.base_outline_memory(component_gid),
        false => match self.coverage_index(component_gid)? {
          Some(coverage_index) => {
            self.max_component_memory_for_glyph(component_gid, coverage_index, stack)?
          }
          None => self.base.base_outline_memory(component_gid),
        },
      };
      max_memory = max_memory.max(component_memory);
    }
    stack.pop();
    Ok(max_memory)
  }

  pub fn draw(
    &self,
    outline: &Outline,
    buf: &mut [u8],
    size: Size,
    coords: &[F2Dot14],
    path_style: PathStyle,
    pen: &mut impl OutlinePen,
  ) -> Result<(), DrawError> {
    let mut font_coords = CoordVec::new();
    expand_coords(&mut font_coords, self.axis_count, coords);
    let mut stack = GlyphStack::new();
    let pen: &mut dyn OutlinePen = pen;
    let mut scalar_cache = self
      .scalar_cache_from_store(self.var_store.as_ref())?
      .unwrap();
    let mut scratch = Scratchpad::new();
    let ctx = VarcSharedContext {
      font_coords: &font_coords,
      size,
      path_style,
      coverage: &self.coverage,
      var_store: self.var_store.as_ref(),
      store_regions: self.var_store.as_ref().zip(self.regions.as_ref()),
    };
    self.draw_glyph(
      outline.glyph_id,
      outline.coverage_index,
      &font_coords,
      Affine::IDENTITY,
      &ctx,
      buf,
      pen,
      &mut stack,
      &mut scalar_cache,
      &mut scratch,
    )
  }

  pub fn draw_unscaled(
    &self,
    outline: &Outline,
    buf: &mut [u8],
    coords: &[F2Dot14],
    pen: &mut impl OutlinePen,
  ) -> Result<i32, DrawError> {
    let size = Size::unscaled();
    self.draw(outline, buf, size, coords, PathStyle::default(), pen)?;
    Ok(self.glyph_metrics.advance_width(outline.glyph_id, coords))
  }

  #[allow(clippy::too_many_arguments)]
  fn draw_glyph(
    &self,
    glyph_id: GlyphId,
    coverage_index: u16,
    current_coords: &[F2Dot14],
    parent_matrix: Affine,
    ctx: &VarcSharedContext<'a, '_>,
    buf: &mut [u8],
    pen: &mut dyn OutlinePen,
    stack: &mut GlyphStack,
    scalar_cache: &mut ScalarCache,
    scratch: &mut Scratchpad,
  ) -> Result<(), DrawError> {
    match stack.len() >= GLYF_COMPOSITE_RECURSION_LIMIT {
      true => {
        return Err(DrawError::RecursionLimitExceeded(glyph_id));
      }
      false => {}
    }
    let glyph = self.varc.glyph(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index(coverage_index);
            migrated_value
          }),
        );
        value
      }),
    )?;
    stack.push(glyph_id);
    let coverage = ctx.coverage;
    let store_regions = ctx.store_regions;
    let mut component_coords_buffer = CoordVec::new();
    let mut child_scalar_cache: Option<ScalarCache> = None;
    for component in glyph.components() {
      let component = component?;
      match !self.component_condition_met(
        &component,
        current_coords,
        scalar_cache,
        scratch,
        store_regions,
      )? {
        true => {
          continue;
        }
        false => {}
      }
      let component_gid = component.gid();
      let flags = component.flags();

      let coords_the_same = !({
        let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_AXES);
        (explicit_value == read_fonts::Presence::Present)
      }) && !({
        let Ok(explicit_value) = flags.contains(VarcFlags::RESET_UNSPECIFIED_AXES);
        (explicit_value == read_fonts::Presence::Present)
      });

      let component_coords = match coords_the_same {
        true => current_coords,
        false => {
          self.component_coords(
            &component,
            current_coords,
            &mut component_coords_buffer,
            scalar_cache,
            scratch,
            ctx.font_coords,
            store_regions,
          )?;
          component_coords_buffer.as_slice()
        }
      };

      let mut transform = *component.transform();
      self.apply_transform_variations(
        &component,
        current_coords,
        &mut transform,
        scalar_cache,
        scratch,
        store_regions,
      )?;
      let scale = ctx.size.linear_scale(self.units_per_em);
      let matrix = parent_matrix * scale_matrix(transform.matrix(), scale);
      match component_gid != glyph_id {
        true => match (coverage.get(component_gid)) {
          Some(coverage_index) => match !stack.contains(&component_gid) {
            true => {
              match coords_the_same {
                true => {
                  self.draw_glyph(
                    component_gid,
                    coverage_index,
                    current_coords,
                    matrix,
                    ctx,
                    buf,
                    pen,
                    stack,
                    scalar_cache,
                    scratch,
                  )?;
                }
                false => {
                  if let Some(ref mut cache) = child_scalar_cache {
                    cache.values.fill(ScalarCache::INVALID);
                  } else {
                    child_scalar_cache = self.scalar_cache_from_store(ctx.var_store)?;
                  }
                  self.draw_glyph(
                    component_gid,
                    coverage_index,
                    component_coords,
                    matrix,
                    ctx,
                    buf,
                    pen,
                    stack,
                    child_scalar_cache.as_mut().unwrap(),
                    scratch,
                  )?;
                }
              }
              continue;
            }
            false => {}
          },
          None => {}
        },
        false => {}
      }
      let mut transform_pen = TransformPen::new(pen, matrix);
      self.draw_base_glyph(
        component_gid,
        component_coords,
        ctx.size,
        ctx.path_style,
        buf,
        &mut transform_pen,
      )?;
    }
    stack.pop();
    Ok(())
  }

  fn draw_base_glyph(
    &self,
    glyph_id: GlyphId,
    coords: &[F2Dot14],
    size: Size,
    path_style: PathStyle,
    buf: &mut [u8],
    pen: &mut impl OutlinePen,
  ) -> Result<(), DrawError> {
    let kind = match self.base.base_outline_kind(glyph_id) {
      Some(kind) => kind,
      None => {
        return Err(DrawError::GlyphNotFound(glyph_id));
      }
    };
    let settings =
      crate::outline::DrawSettings::unhinted(size, crate::instance::LocationRef::new(coords))
        .with_path_style(path_style)
        .with_memory(Some(buf));
    crate::outline::OutlineGlyph { kind }.draw(settings, pen)?;
    Ok(())
  }

  #[allow(clippy::too_many_arguments)]
  fn component_coords(
    &self,
    component: &VarcComponent<'_>,
    current_coords: &[F2Dot14],
    coords: &mut CoordVec,
    scalar_cache: &mut ScalarCache,
    scratch: &mut Scratchpad,
    font_coords: &[F2Dot14],
    store_regions: Option<(&MultiItemVariationStore<'a>, &SparseVariationRegionList<'a>)>,
  ) -> Result<(), DrawError> {
    let flags = component.flags();
    let Ok(contains) = flags.contains(VarcFlags::RESET_UNSPECIFIED_AXES);

    match (contains == read_fonts::Presence::Present) {
      true => {
        expand_coords(coords, font_coords.len(), font_coords);
      }
      false => {
        expand_coords(coords, current_coords.len(), current_coords);
      }
    }

    let Ok(flags_contains) = flags.contains(VarcFlags::HAVE_AXES);

    match !(flags_contains == read_fonts::Presence::Present) {
      true => {
        return Ok(());
      }
      false => {}
    }

    let axis_indices_index = component
      .axis_indices_index()
      .ok_or(ReadError::MalformedData("Missing axisIndicesIndex"))?;
    let num_axes = self.axis_indices(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(axis_indices_index);
        migrated_value
      }),
      &mut scratch.axis_indices,
    )?;

    self.axis_values(component, num_axes, &mut scratch.axis_values)?;
    match component.axis_values_var_index() {
      Some(var_idx) => {
        let (store, regions) = store_regions.ok_or(ReadError::NullOffset)?;
        compute_tuple_deltas(
          store,
          regions,
          var_idx,
          current_coords,
          scratch.axis_indices.len(),
          scalar_cache,
          &mut scratch.deltas,
        )?;
        for (value, delta) in scratch.axis_values.iter_mut().zip(scratch.deltas.iter()) {
          *value += *delta;
        }
      }
      None => {}
    }

    for (axis_index, value) in scratch
      .axis_indices
      .iter()
      .zip(scratch.axis_values.iter().copied())
    {
      let slot = match coords.get_mut(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(*axis_index);
          migrated_value
        }),
      ) {
        Some(slot) => slot,
        None => {
          return Err(DrawError::Read(ReadError::OutOfBounds));
        }
      };
      #[cfg(feature = "libm")]
      let rounded = libm::roundf(value);
      #[cfg(not(feature = "libm"))]
      let rounded = value.round();
      let raw = ({
        let Ok(migrated_value) = console_core_number_conversion::toward_zero_i16(f64::from(
          rounded.clamp(f32::from(i16::MIN), f32::from(i16::MAX)),
        ));
        migrated_value
      });
      *slot = ({
        let Ok(value) = F2Dot14::from_bits(raw);
        value
      });
    }
    Ok(())
  }

  fn axis_indices(&self, nth: usize, out: &mut AxisIndexVec) -> Result<usize, DrawError> {
    let packed = self.varc.axis_indices(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(nth);
        value
      }),
    )?;
    out.clear();
    for value in packed.iter() {
      out.push(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(value);
          migrated_value
        }),
      );
    }
    Ok(out.len())
  }

  fn axis_values(
    &self,
    component: &VarcComponent<'_>,
    count: usize,
    out: &mut AxisValueVec,
  ) -> Result<(), DrawError> {
    let packed = match component.axis_values() {
      Some(packed) => packed,
      None => {
        out.clear();
        return Ok(());
      }
    };
    out.clear();
    out.resize(count, 0.0);
    for (slot, value) in out.iter_mut().zip(packed.iter().by_ref().take(count)) {
      *slot = value as f32;
    }
    Ok(())
  }

  fn apply_transform_variations(
    &self,
    component: &VarcComponent<'_>,
    coords: &[F2Dot14],
    transform: &mut DecomposedTransform,
    scalar_cache: &mut ScalarCache,
    scratch: &mut Scratchpad,
    store_regions: Option<(&MultiItemVariationStore<'a>, &SparseVariationRegionList<'a>)>,
  ) -> Result<(), DrawError> {
    let var_idx = match component.transform_var_index() {
      Some(var_idx) => var_idx,
      None => {
        return Ok(());
      }
    };

    let flags = component.flags();

    let Ok(have_translate_x_bits) = VarcFlags::HAVE_TRANSLATE_X.bits();
    let Ok(have_translate_y_bits) = VarcFlags::HAVE_TRANSLATE_Y.bits();
    let Ok(have_rotation_bits) = VarcFlags::HAVE_ROTATION.bits();
    let Ok(have_scale_x_bits) = VarcFlags::HAVE_SCALE_X.bits();
    let Ok(have_scale_y_bits) = VarcFlags::HAVE_SCALE_Y.bits();
    let Ok(have_skew_x_bits) = VarcFlags::HAVE_SKEW_X.bits();
    let Ok(have_skew_y_bits) = VarcFlags::HAVE_SKEW_Y.bits();
    let Ok(have_tcenter_x_bits) = VarcFlags::HAVE_TCENTER_X.bits();
    let Ok(have_tcenter_y_bits) = VarcFlags::HAVE_TCENTER_Y.bits();
    let Ok(TRANSFORM_MASK) = VarcFlags::from_bits_truncate(
      have_translate_x_bits
        | have_translate_y_bits
        | have_rotation_bits
        | have_scale_x_bits
        | have_scale_y_bits
        | have_skew_x_bits
        | have_skew_y_bits
        | have_tcenter_x_bits
        | have_tcenter_y_bits,
    );
    let field_count = ({
      let Ok(flags_bits) = flags.bits();
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        (flags_bits
          & ({
            let Ok(explicit_value) = TRANSFORM_MASK.bits();
            explicit_value
          }))
        .count_ones(),
      );
      migrated_value
    });
    match field_count == 0 {
      true => {
        return Ok(());
      }
      false => {}
    }

    let (store, regions) = store_regions.ok_or(ReadError::NullOffset)?;
    compute_tuple_deltas(
      store,
      regions,
      var_idx,
      coords,
      field_count,
      scalar_cache,
      &mut scratch.deltas,
    )?;

    let mut delta_iter = scratch.deltas.iter().copied();

    let Ok(contains) = flags.contains(VarcFlags::HAVE_TRANSLATE_X);

    match (contains == read_fonts::Presence::Present) {
      true => {
        let delta = delta_iter.next().unwrap_or(0.0);
        transform.set_translate_x(transform.translate_x() + delta);
      }
      false => {}
    }
    let Ok(flags_contains) = flags.contains(VarcFlags::HAVE_TRANSLATE_Y);

    match (flags_contains == read_fonts::Presence::Present) {
      true => {
        let delta = delta_iter.next().unwrap_or(0.0);
        transform.set_translate_y(transform.translate_y() + delta);
      }
      false => {}
    }
    match (({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_ROTATION);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        let delta = delta_iter.next().unwrap_or(0.0);
        transform.set_rotation(transform.rotation() + delta / 4096.0);
      }
      false => {}
    }
    match (({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SCALE_X);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        let delta = delta_iter.next().unwrap_or(0.0);
        transform.set_scale_x(transform.scale_x() + delta / 1024.0);
      }
      false => {}
    }
    match (({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SCALE_Y);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        let delta = delta_iter.next().unwrap_or(0.0);
        transform.set_scale_y(transform.scale_y() + delta / 1024.0);
      }
      false => {}
    }
    const SKEW_OR_CENTER: VarcFlags = ({
      let Ok(explicit_value) = VarcFlags::from_bits_truncate(
        ({
          let Ok(explicit_value) = VarcFlags::HAVE_SKEW_X.bits();
          explicit_value
        }) | ({
          let Ok(explicit_value) = VarcFlags::HAVE_SKEW_Y.bits();
          explicit_value
        }) | ({
          let Ok(explicit_value) = VarcFlags::HAVE_TCENTER_X.bits();
          explicit_value
        }) | ({
          let Ok(explicit_value) = VarcFlags::HAVE_TCENTER_Y.bits();
          explicit_value
        }),
      );
      explicit_value
    });
    let Ok(intersects) = flags.intersects(SKEW_OR_CENTER);

    match (intersects == read_fonts::Overlap::Intersecting) {
      true => {
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SKEW_X);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            let delta = delta_iter.next().unwrap_or(0.0);
            transform.set_skew_x(transform.skew_x() + delta / 4096.0);
          }
          false => {}
        }
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SKEW_Y);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            let delta = delta_iter.next().unwrap_or(0.0);
            transform.set_skew_y(transform.skew_y() + delta / 4096.0);
          }
          false => {}
        }
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_TCENTER_X);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            let delta = delta_iter.next().unwrap_or(0.0);
            transform.set_center_x(transform.center_x() + delta);
          }
          false => {}
        }
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_TCENTER_Y);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            let delta = delta_iter.next().unwrap_or(0.0);
            transform.set_center_y(transform.center_y() + delta);
          }
          false => {}
        }
      }
      false => {}
    }

    match !(({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SCALE_Y);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        transform.set_scale_y(transform.scale_x());
      }
      false => {}
    }
    Ok(())
  }

  fn component_condition_met(
    &self,
    component: &VarcComponent<'_>,
    coords: &[F2Dot14],
    scalar_cache: &mut ScalarCache,
    scratch: &mut Scratchpad,
    store_regions: Option<(&MultiItemVariationStore<'a>, &SparseVariationRegionList<'a>)>,
  ) -> Result<bool, DrawError> {
    let condition_index = match component.condition_index() {
      Some(condition_index) => condition_index,
      None => {
        return Ok(true);
      }
    };
    let condition_list = match self.varc.condition_list()? {
      Some(condition_list) => condition_list,
      None => {
        return Err(DrawError::Read(ReadError::NullOffset));
      }
    };

    let Ok(conditions) = condition_list.conditions();
    let condition = (conditions.get(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index(condition_index);
            migrated_value
          }),
        );
        value
      }),
    ))?;
    let (store, regions) = store_regions.ok_or(ReadError::NullOffset)?;
    Self::eval_condition(&condition, coords, store, regions, scalar_cache, scratch)
  }

  fn eval_condition(
    condition: &Condition<'a>,
    coords: &[F2Dot14],
    var_store: &MultiItemVariationStore<'a>,
    regions: &SparseVariationRegionList<'a>,
    scalar_cache: &mut ScalarCache,
    scratch: &mut Scratchpad,
  ) -> Result<bool, DrawError> {
    match condition {
      Condition::Format1AxisRange(condition) => {
        let axis_index = ({
          let Ok(condition_axis_index) = condition.axis_index();
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped_index(condition_axis_index);
          migrated_value
        });
        let coord = (coords.get(axis_index)).copied().unwrap_or(F2Dot14::ZERO);
        let Ok(filter_range_min_value) = condition.filter_range_min_value();

        Ok(
          coord >= filter_range_min_value
            && coord
              <= ({
                let Ok(explicit_value) = condition.filter_range_max_value();
                explicit_value
              }),
        )
      }
      Condition::Format2VariableValue(condition) => {
        let Ok(condition_default_value) = condition.default_value();
        let default_value = f32::from(condition_default_value);
        let Ok(var_idx) = condition.var_index();
        compute_tuple_deltas(
          var_store,
          regions,
          var_idx,
          coords,
          1,
          scalar_cache,
          &mut scratch.deltas,
        )?;
        let delta = scratch.deltas.first().copied().unwrap_or(0.0);
        Ok(default_value + delta > 0.0)
      }
      Condition::Format3And(condition) => {
        let conditions = ({
          let Ok(array) = condition.conditions();
          let Ok(items) = array.iter();
          items
        });
        for nested in conditions {
          let nested = nested?;
          match !Self::eval_condition(&nested, coords, var_store, regions, scalar_cache, scratch)?
          {
            true => {
              return Ok(false);
            }
            false => {}
          }
        }
        Ok(true)
      }
      Condition::Format4Or(condition) => {
        let conditions = ({
          let Ok(array) = condition.conditions();
          let Ok(items) = array.iter();
          items
        });
        for nested in conditions {
          let nested = nested?;
          match Self::eval_condition(&nested, coords, var_store, regions, scalar_cache, scratch)?
          {
            true => {
              return Ok(true);
            }
            false => {}
          }
        }
        Ok(false)
      }
      Condition::Format5Negate(condition) => {
        let nested = condition.condition()?;
        Ok(!Self::eval_condition(
          &nested,
          coords,
          var_store,
          regions,
          scalar_cache,
          scratch,
        )?)
      }
    }
  }

  fn scalar_cache_from_store(
    &self,
    store: Option<&MultiItemVariationStore<'a>>,
  ) -> Result<Option<ScalarCache>, DrawError> {
    let store = match store {
      Some(store) => store,
      None => {
        return Ok(None);
      }
    };
    let region_count = ({
      let Ok(store_region_count) = store.region_list()?.region_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(store_region_count);
      migrated_value
    });
    Ok(Some(ScalarCache::new(region_count)))
  }
}

struct ScalarCache {
  values: ScalarCacheVec,
}

impl ScalarCache {
  const INVALID: f32 = 2.0;

  fn new(count: usize) -> Self {
    Self {
      values: alloc::vec![Self::INVALID; count],
    }
  }

  fn get(&self, index: usize) -> f32 {
    (self.values.get(index)).copied().unwrap_or(Self::INVALID)
  }

  fn set(&mut self, index: usize, value: f32) {
    match self.values.get_mut(index) {
      Some(slot) => {
        *slot = value;
      }
      None => {}
    }
  }
}

fn expand_coords(out: &mut CoordVec, axis_count: usize, coords: &[F2Dot14]) {
  out.clear();
  out.resize(axis_count, F2Dot14::ZERO);
  for (slot, value) in out.iter_mut().zip(coords.iter().copied()) {
    *slot = value;
  }
}

fn compute_tuple_deltas(
  store: &MultiItemVariationStore,
  regions: &SparseVariationRegionList,
  var_idx: u32,
  coords: &[F2Dot14],
  tuple_len: usize,
  cache: &mut ScalarCache,
  out: &mut DeltaVec,
) -> Result<(), ReadError> {
  out.clear();
  out.resize(tuple_len, 0.0);
  match tuple_len == 0 || var_idx == NO_VARIATION_INDEX {
    true => {
      return Ok(());
    }
    false => {}
  }
  let outer = ({
    let Ok(migrated_value) = console_core_number_conversion::wrapped_index((var_idx >> 16));
    migrated_value
  });
  let inner = ({
    let Ok(migrated_value) = console_core_number_conversion::wrapped_index((var_idx & 0xFFFF));
    migrated_value
  });
  let Ok(variation_data) = store.variation_data();
  let data = (variation_data.get(
    ({
      let Ok(value) = console_core_number_conversion::fitted::<_, u64>(outer);
      value
    }),
  ))
  .map_err(|_| {
    ReadError::InvalidCollectionIndex(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(outer);
        migrated_value
      }),
    )
  })?;
  let Ok(region_indices) = data.region_indices();
  let mut deltas = data
    .delta_set(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(inner);
        value
      }),
    )?
    .fetcher();
  let Ok(regions) = regions.regions();
  let out_slice = out.as_mut_slice();

  let mut skip = 0;
  for region_index in region_indices.iter() {
    let region_idx = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        ({
          let Ok(value) = region_index.get();
          value
        }),
      );
      migrated_value
    });
    let mut scalar = (cache.get(region_idx));
    match scalar >= 2.0 {
      true => {
        scalar = (regions.get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(region_idx);
            value
          }),
        ))?
        .compute_scalar_f32(coords);
        cache.set(region_idx, scalar);
      }
      false => {}
    }

    match scalar == 0.0 {
      true => {
        skip += out_slice.len();
      }
      false => {
        match skip != 0 {
          true => {
            deltas.skip(
              ({
                let Ok(value) = console_core_number_conversion::fitted::<_, u64>(skip);
                value
              }),
            )?;
            skip = 0;
          }
          false => {}
        }
        deltas.add_to_f32_scaled(out_slice, scalar)?;
      }
    }
  }

  Ok(())
}

#[inline(always)]
fn scale_matrix(mut m: Affine, s: f32) -> Affine {
  m.dx *= s;
  m.dy *= s;
  m
}

struct TransformPen<'a, P: OutlinePen + ?Sized> {
  pen: &'a mut P,
  matrix: Affine,
}

impl<'a, P: OutlinePen + ?Sized> TransformPen<'a, P> {
  fn new(pen: &'a mut P, matrix: Affine) -> Self {
    Self { pen, matrix }
  }

  #[inline(always)]
  fn transform(&self, x: f32, y: f32) -> (f32, f32) {
    ({
      let Ok(value) = self.matrix.transform(x, y);
      value
    })
  }
}

impl<P: OutlinePen + ?Sized> OutlinePen for TransformPen<'_, P> {
  fn move_to(&mut self, x: f32, y: f32) {
    let (x, y) = self.transform(x, y);
    self.pen.move_to(x, y);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let (x, y) = self.transform(x, y);
    self.pen.line_to(x, y);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let (cx0, cy0) = self.transform(cx0, cy0);
    let (x, y) = self.transform(x, y);
    self.pen.quad_to(cx0, cy0, x, y);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let (cx0, cy0) = self.transform(cx0, cy0);
    let (cx1, cy1) = self.transform(cx1, cy1);
    let (x, y) = self.transform(x, y);
    self.pen.curve_to(cx0, cy0, cx1, cy1, x, y);
  }

  fn close(&mut self) {
    self.pen.close();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::outline::pen::PathElement;
  use read_fonts::{FontRef, TableProvider};

  fn coord(value: f32) -> F2Dot14 {
    ({
      let Ok(value) = F2Dot14::from_f32(value);
      value
    })
  }

  fn assert_close(actual: f32, expected: f32) {
    let diff = (actual - expected).abs();
    assert!(
      diff <= 1e-6,
      "expected {expected}, got {actual}, diff {diff}"
    );
  }

  fn path_head_signature(path: &[PathElement], n: usize) -> Vec<String> {
    path
      .iter()
      .take(n)
      .map(|el| match *el {
        PathElement::MoveTo { x, y } => format!("M{:.2},{:.2}", x, y),
        PathElement::LineTo { x, y } => format!("L{:.2},{:.2}", x, y),
        PathElement::QuadTo { cx0, cy0, x, y } => {
          format!("Q{:.2},{:.2} {:.2},{:.2}", cx0, cy0, x, y)
        }
        PathElement::CurveTo {
          cx0,
          cy0,
          cx1,
          cy1,
          x,
          y,
        } => format!(
          "C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}",
          cx0, cy0, cx1, cy1, x, y
        ),
        PathElement::Close => "Z".to_string(),
      })
      .collect()
  }

  #[test]
  fn expand_coords_pads_and_truncates() {
    let mut out = CoordVec::new();
    expand_coords(&mut out, 4, &[coord(0.25), coord(-0.5)]);
    assert_eq!(
      out.as_slice(),
      &[coord(0.25), coord(-0.5), F2Dot14::ZERO, F2Dot14::ZERO]
    );

    expand_coords(&mut out, 1, &[coord(0.25), coord(-0.5)]);
    assert_eq!(out.as_slice(), &[coord(0.25)]);
  }

  #[test]
  fn scale_matrix_only_scales_translation() {
    let matrix = ({
      let Ok(value) = Matrix::from_elements([1.0, 2.0, 3.0, 4.0, 5.0, -6.0]);
      value
    });
    assert_eq!(
      ({
        let Ok(value) = scale_matrix((matrix), 10.0).elements();
        value
      }),
      [1.0, 2.0, 3.0, 4.0, 50.0, -60.0]
    );
  }

  #[test]
  fn compute_tuple_deltas_no_variation_index_is_noop_after_resize() {
    let font = FontRef::new(font_test_data::varc::CJK_6868).unwrap();
    let varc = font.varc().unwrap();
    let store = varc.multi_var_store().unwrap().unwrap();
    let regions = store.region_list().unwrap();
    let mut cache = ScalarCache::new(
      ({
        let Ok(region_count) = regions.region_count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(region_count);
        migrated_value
      }),
    );
    let mut out = DeltaVec::new();
    out.push(42.0);

    compute_tuple_deltas(
      &store,
      &regions,
      NO_VARIATION_INDEX,
      &[coord(0.0)],
      3,
      &mut cache,
      &mut out,
    )
    .unwrap();
    assert_eq!(out.as_slice(), &[0.0, 0.0, 0.0]);

    compute_tuple_deltas(
      &store,
      &regions,
      NO_VARIATION_INDEX,
      &[coord(0.0)],
      0,
      &mut cache,
      &mut out,
    )
    .unwrap();
    assert!(out.is_empty());
  }

  #[test]
  fn compute_tuple_deltas_invalid_outer_index_errors() {
    let font = FontRef::new(font_test_data::varc::CJK_6868).unwrap();
    let varc = font.varc().unwrap();
    let store = varc.multi_var_store().unwrap().unwrap();
    let regions = store.region_list().unwrap();
    let mut cache = ScalarCache::new(
      ({
        let Ok(region_count) = regions.region_count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(region_count);
        migrated_value
      }),
    );
    let mut out = DeltaVec::new();

    let err = compute_tuple_deltas(&store, &regions, 0xFFFF_0000, &[], 1, &mut cache, &mut out)
      .unwrap_err();
    assert!(matches!(err, ReadError::InvalidCollectionIndex(_)));
  }

  #[test]
  fn compute_tuple_deltas_matches_manual_decode() {
    let font = FontRef::new(font_test_data::varc::CJK_6868).unwrap();
    let varc = font.varc().unwrap();
    let store = varc.multi_var_store().unwrap().unwrap();
    let regions = store.region_list().unwrap();
    let Ok(region_list) = regions.regions();
    let coords = [coord(0.5); 8];

    let mut tried = 0usize;
    for (outer, data) in ({
      let value = ({
        let Ok(array) = store.variation_data();
        let Ok(items) = array.iter();
        items
      });
      value
    })
    .enumerate()
    {
      let data = data.unwrap();
      let Ok(region_indices) = data.region_indices();
      let region_count = region_indices.len();
      match region_count == 0 {
        true => {
          continue;
        }
        false => {}
      }
      let delta_set_count = ({
        let Ok(count) = data.delta_sets().unwrap().count();
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(count);
        migrated_value
      });
      for inner in 0..delta_set_count.min(3) {
        let decoded = data
          .delta_set(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(inner);
              value
            }),
          )
          .unwrap()
          .iter()
          .collect::<Vec<_>>();
        match decoded.is_empty() || decoded.len() % region_count != 0 {
          true => {
            continue;
          }
          false => {}
        }
        let tuple_len = decoded.len() / region_count;
        let var_idx = (({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(outer);
          migrated_value
        }) << 16)
          | ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(inner);
            migrated_value
          });

        let mut cache = ScalarCache::new(
          ({
            let Ok(regions_region_count) = regions.region_count();
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index(regions_region_count);
            migrated_value
          }),
        );
        let mut actual = DeltaVec::new();
        compute_tuple_deltas(
          &store,
          &regions,
          var_idx,
          &coords,
          tuple_len,
          &mut cache,
          &mut actual,
        )
        .unwrap();
        let first = actual.as_slice().to_vec();

        compute_tuple_deltas(
          &store,
          &regions,
          var_idx,
          &coords,
          tuple_len,
          &mut cache,
          &mut actual,
        )
        .unwrap();
        assert_eq!(actual.as_slice(), first.as_slice());

        let mut expected = vec![0.0f32; tuple_len];
        let Ok(data_region_indices) = data.region_indices();

        for (region_order, region_idx) in data_region_indices.iter().enumerate() {
          let scalar = region_list
            .get(
              ({
                let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                  ({
                    let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
                      ({
                        let Ok(value) = region_idx.get();
                        value
                      }),
                    );
                    migrated_value
                  }),
                );
                value
              }),
            )
            .unwrap()
            .compute_scalar_f32(&coords);
          match scalar == 0.0 {
            true => {
              continue;
            }
            false => {}
          }
          let base = region_order * tuple_len;
          for (i, slot) in expected.iter_mut().enumerate() {
            *slot += decoded[base + i] as f32 * scalar;
          }
        }
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected.iter()) {
          assert_close(*a, *e);
        }
        tried += 1;
      }
    }
    assert!(tried > 0, "expected at least one tuple to be exercised");
  }

  fn apply_transform_variations_reference(
    component: &VarcComponent<'_>,
    coords: &[F2Dot14],
    transform: &mut DecomposedTransform,
    var_store: Option<&MultiItemVariationStore<'_>>,
    regions: Option<&SparseVariationRegionList<'_>>,
    scalar_cache: &mut ScalarCache,
    deltas: &mut DeltaVec,
  ) -> Result<(), DrawError> {
    let var_idx = match component.transform_var_index() {
      Some(var_idx) => var_idx,
      None => {
        return Ok(());
      }
    };
    let flags = component.flags();
    let Ok(have_translate_x_bits) = VarcFlags::HAVE_TRANSLATE_X.bits();
    let Ok(have_translate_y_bits) = VarcFlags::HAVE_TRANSLATE_Y.bits();
    let Ok(have_rotation_bits) = VarcFlags::HAVE_ROTATION.bits();
    let Ok(have_scale_x_bits) = VarcFlags::HAVE_SCALE_X.bits();
    let Ok(have_scale_y_bits) = VarcFlags::HAVE_SCALE_Y.bits();
    let Ok(have_skew_x_bits) = VarcFlags::HAVE_SKEW_X.bits();
    let Ok(have_skew_y_bits) = VarcFlags::HAVE_SKEW_Y.bits();
    let Ok(have_tcenter_x_bits) = VarcFlags::HAVE_TCENTER_X.bits();
    let Ok(have_tcenter_y_bits) = VarcFlags::HAVE_TCENTER_Y.bits();
    let Ok(TRANSFORM_MASK) = VarcFlags::from_bits_truncate(
      have_translate_x_bits
        | have_translate_y_bits
        | have_rotation_bits
        | have_scale_x_bits
        | have_scale_y_bits
        | have_skew_x_bits
        | have_skew_y_bits
        | have_tcenter_x_bits
        | have_tcenter_y_bits,
    );
    let field_count = ({
      let Ok(flags_bits) = flags.bits();
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        (flags_bits
          & ({
            let Ok(explicit_value) = TRANSFORM_MASK.bits();
            explicit_value
          }))
        .count_ones(),
      );
      migrated_value
    });
    match field_count == 0 {
      true => {
        return Ok(());
      }
      false => {}
    }

    let store = var_store.ok_or(ReadError::NullOffset)?;
    let regions = regions.ok_or(ReadError::NullOffset)?;
    compute_tuple_deltas(
      store,
      regions,
      var_idx,
      coords,
      field_count,
      scalar_cache,
      deltas,
    )?;

    let mut delta_iter = deltas.iter().copied();
    let Ok(contains) = flags.contains(VarcFlags::HAVE_TRANSLATE_X);

    match (contains == read_fonts::Presence::Present) {
      true => {
        transform.set_translate_x(transform.translate_x() + delta_iter.next().unwrap_or(0.0));
      }
      false => {}
    }
    let Ok(flags_contains) = flags.contains(VarcFlags::HAVE_TRANSLATE_Y);

    match (flags_contains == read_fonts::Presence::Present) {
      true => {
        transform.set_translate_y(transform.translate_y() + delta_iter.next().unwrap_or(0.0));
      }
      false => {}
    }
    match (({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_ROTATION);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        transform.set_rotation(transform.rotation() + delta_iter.next().unwrap_or(0.0) / 4096.0);
      }
      false => {}
    }
    match (({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SCALE_X);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        transform.set_scale_x(transform.scale_x() + delta_iter.next().unwrap_or(0.0) / 1024.0);
      }
      false => {}
    }
    match (({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SCALE_Y);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        transform.set_scale_y(transform.scale_y() + delta_iter.next().unwrap_or(0.0) / 1024.0);
      }
      false => {}
    }
    const SKEW_OR_CENTER: VarcFlags = ({
      let Ok(explicit_value) = VarcFlags::from_bits_truncate(
        ({
          let Ok(explicit_value) = VarcFlags::HAVE_SKEW_X.bits();
          explicit_value
        }) | ({
          let Ok(explicit_value) = VarcFlags::HAVE_SKEW_Y.bits();
          explicit_value
        }) | ({
          let Ok(explicit_value) = VarcFlags::HAVE_TCENTER_X.bits();
          explicit_value
        }) | ({
          let Ok(explicit_value) = VarcFlags::HAVE_TCENTER_Y.bits();
          explicit_value
        }),
      );
      explicit_value
    });
    let Ok(intersects) = flags.intersects(SKEW_OR_CENTER);

    match (intersects == read_fonts::Overlap::Intersecting) {
      true => {
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SKEW_X);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            transform.set_skew_x(transform.skew_x() + delta_iter.next().unwrap_or(0.0) / 4096.0);
          }
          false => {}
        }
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SKEW_Y);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            transform.set_skew_y(transform.skew_y() + delta_iter.next().unwrap_or(0.0) / 4096.0);
          }
          false => {}
        }
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_TCENTER_X);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            transform.set_center_x(transform.center_x() + delta_iter.next().unwrap_or(0.0));
          }
          false => {}
        }
        match (({
          let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_TCENTER_Y);
          explicit_value
        }) == read_fonts::Presence::Present)
        {
          true => {
            transform.set_center_y(transform.center_y() + delta_iter.next().unwrap_or(0.0));
          }
          false => {}
        }
      }
      false => {}
    }
    match !(({
      let Ok(explicit_value) = flags.contains(VarcFlags::HAVE_SCALE_Y);
      explicit_value
    }) == read_fonts::Presence::Present)
    {
      true => {
        transform.set_scale_y(transform.scale_x());
      }
      false => {}
    }
    Ok(())
  }

  #[test]
  fn apply_transform_variations_matches_reference_path() {
    let font = FontRef::new(font_test_data::varc::CJK_6868).unwrap();
    let outlines = Outlines::new(&font).unwrap();
    let coverage = outlines.varc.coverage().unwrap();
    let var_store = outlines.varc.multi_var_store().unwrap();
    let regions = var_store
      .as_ref()
      .map(|s| s.region_list())
      .transpose()
      .unwrap();
    let region_count = regions
      .as_ref()
      .map(|r| {
        ({
          let Ok(r_region_count) = r.region_count();
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(r_region_count);
          migrated_value
        })
      })
      .unwrap_or(0);

    let mut coords = CoordVec::new();
    coords.clear();
    coords.resize(outlines.axis_count, F2Dot14::ZERO);
    for (i, c) in coords.iter_mut().enumerate() {
      *c = match i % 4 {
        0 => coord(0.5),
        1 => coord(-0.5),
        2 => coord(0.25),
        _ => coord(-0.25),
      };
    }

    let mut tested = 0usize;
    for gid16 in coverage.iter() {
      let gid: GlyphId = gid16.into();
      let coverage_index = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(coverage.get(gid).unwrap());
        migrated_value
      });
      let glyph = outlines
        .varc
        .glyph(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(coverage_index);
            value
          }),
        )
        .unwrap();
      for component in glyph.components() {
        let component = component.unwrap();
        match component.transform_var_index().is_none() {
          true => {
            continue;
          }
          false => {}
        }

        let mut transform_new = *component.transform();
        let mut transform_ref = *component.transform();
        let mut cache_new = ScalarCache::new(region_count);
        let mut cache_ref = ScalarCache::new(region_count);
        let mut deltas_ref = DeltaVec::new();
        let mut scratch = Scratchpad::new();
        let store_regions = var_store.as_ref().zip(regions.as_ref());

        outlines
          .apply_transform_variations(
            &component,
            &coords,
            &mut transform_new,
            &mut cache_new,
            &mut scratch,
            store_regions,
          )
          .unwrap();
        apply_transform_variations_reference(
          &component,
          &coords,
          &mut transform_ref,
          var_store.as_ref(),
          regions.as_ref(),
          &mut cache_ref,
          &mut deltas_ref,
        )
        .unwrap();

        assert_close(transform_new.translate_x(), transform_ref.translate_x());
        assert_close(transform_new.translate_y(), transform_ref.translate_y());
        assert_close(transform_new.rotation(), transform_ref.rotation());
        assert_close(transform_new.scale_x(), transform_ref.scale_x());
        assert_close(transform_new.scale_y(), transform_ref.scale_y());
        assert_close(transform_new.skew_x(), transform_ref.skew_x());
        assert_close(transform_new.skew_y(), transform_ref.skew_y());
        assert_close(transform_new.center_x(), transform_ref.center_x());
        assert_close(transform_new.center_y(), transform_ref.center_y());
        tested += 1;
        match tested >= 32 {
          true => {
            break;
          }
          false => {}
        }
      }
      match tested >= 32 {
        true => {
          break;
        }
        false => {}
      }
    }
    assert!(tested > 0, "expected at least one transformed component");
  }

  #[test]
  fn draw_varc_6868_freetype_path_head_snapshot() {
    let font = FontRef::new(font_test_data::varc::CJK_6868).unwrap();
    let outlines = Outlines::new(&font).unwrap();
    let gid = font.cmap().unwrap().map_codepoint(0x6868_u32).unwrap();
    let outline = outlines.outline(gid).unwrap().unwrap();
    let mut memory = vec![0u8; outline.required_buffer_size()];
    let mut pen = Vec::<PathElement>::new();
    outlines
      .draw(
        &outline,
        &mut memory,
        Size::unscaled(),
        &[],
        PathStyle::FreeType,
        &mut pen,
      )
      .unwrap();

    let head = path_head_signature(&pen, 8);
    assert_eq!(
      head,
      vec![
        "M454.56,574.77".to_string(),
        "Q477.58,585.56 499.80,598.25".to_string(),
        "Q522.02,610.95 543.36,625.16".to_string(),
        "Q564.71,639.37 584.54,655.19".to_string(),
        "Q604.38,671.02 623.03,688.41".to_string(),
        "Q641.67,705.80 658.41,724.46".to_string(),
        "Q675.14,743.12 689.79,763.21".to_string(),
        "Q704.43,783.31 717.03,804.56".to_string(),
      ]
    );
  }
}
