//! Unified access to CFF/CFF2 fonts.

use super::CffVersion;
use crate::{
  FontData, FontRead, ReadError,
  model::pen::OutlinePen,
  ps::{
    Pitch,
    cff::{
      blend::BlendState, charset::Charset, dict, encoding::Encoding as RawEncoding,
      fd_select::FdSelect, index::Index,
    },
    cs::{self, CommandSink, NopFilterSink, TransformSink},
    encoding::PredefinedEncoding,
    error::Error,
    hinting::HintingParams,
    string::Sid,
    transform::{self, ScaledFontMatrix, Transform},
  },
  tables::{cff, cff2, variations::ItemVariationStore},
};
use core::ops::Range;
use types::{BoundingBox, F2Dot14, Fixed, GlyphId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphIndexing {
  NameKeyed,
  CidKeyed,
  GlyphId,
}

#[derive(Clone)]
pub struct CffFontRef<'a> {
  data: &'a [u8],
  version: CffVersion,
  upem: i32,
  global_subrs: Index<'a>,
  top_dict: TopDict<'a>,
  top_dict_index: u16,
}

impl<'a> CffFontRef<'a> {
  pub fn new(data: &'a [u8], top_dict_index: u32, upem: Option<i32>) -> Result<Self, Error> {
    let Ok(value_1) = FontData::new(data);

    let version: u8 = value_1.read_at(0)?;

    match version {
      1 => Self::new_cff(data, top_dict_index, upem),
      2 => Self::new_cff2(data, upem),
      _ => Err(Error::InvalidFontFormat),
    }
  }

  pub fn new_cff(data: &'a [u8], top_dict_index: u32, upem: Option<i32>) -> Result<Self, Error> {
    let cff = cff::Cff::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(data))?;
    let top_dict_data = cff.top_dicts().get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(top_dict_index);
      migrated_value
    })?;
    let top_dict_index_2: u16 = top_dict_index
      .try_into()
      .map_err(|_| ReadError::OutOfBounds)?;
    Self::new_impl(
      data,
      CffVersion::Cff1,
      upem,
      top_dict_data,
      top_dict_index_2,
      <crate::ps::cff::index::Index<'_> as ::core::convert::From<
        crate::ps::cff::v1::Index<'_>,
      >>::from(cff.strings()),
      <crate::ps::cff::index::Index<'_> as ::core::convert::From<
        crate::ps::cff::v1::Index<'_>,
      >>::from(cff.global_subrs()),
    )
  }

  pub fn new_cff2(data: &'a [u8], upem: Option<i32>) -> Result<Self, Error> {
    let cff = cff2::Cff2::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(data))?;
    Self::new_impl(
      data,
      CffVersion::Cff2,
      upem,
      cff.top_dict_data(),
      0,
      Index::Empty,
      <crate::ps::cff::index::Index<'_> as ::core::convert::From<
        crate::ps::cff::v2::Index<'_>,
      >>::from(cff.global_subrs()),
    )
  }

  fn new_impl(
    data: &'a [u8],
    version: CffVersion,
    upem: Option<i32>,
    top_dict_data: &'a [u8],
    top_dict_index: u16,
    strings: Index<'a>,
    global_subrs: Index<'a>,
  ) -> Result<Self, Error> {
    let top_dict = TopDict::new(data, top_dict_data, strings, version)?;
    let top_upem = match top_dict.matrix.map(|mat| mat.scale) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1000,
    };
    Ok(Self {
      data,
      version,
      upem: match upem {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => top_upem,
      },
      global_subrs,
      top_dict,
      top_dict_index,
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::data` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn data(&self) -> &'a [u8] {
    self.data
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::version` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn version(&self) -> u16 {
    match self.version {
      CffVersion::Cff2 => 2,
      CffVersion::Cff1 => 1,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::metadata` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn metadata(&self) -> Option<Metadata<'a>> {
    let Ok(metadata) = Metadata::new(self.data, self.top_dict_index);

    metadata
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::glyph_indexing` method returns `GlyphIndexing` directly in the existing parser interface"
    )
  )]
  pub fn glyph_indexing(&self) -> GlyphIndexing {
    match &self.top_dict.kind {
      CffFontKind::Sid { .. } => GlyphIndexing::NameKeyed,
      CffFontKind::Cid { .. } => GlyphIndexing::CidKeyed,
      CffFontKind::Cff2 { .. } => GlyphIndexing::GlyphId,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::global_subrs` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn global_subrs(&self) -> &Index<'a> {
    &self.global_subrs
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::num_glyphs` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn num_glyphs(&self) -> u32 {
    self.top_dict.charstrings.count()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::charstrings` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn charstrings(&self) -> &Index<'a> {
    &self.top_dict.charstrings
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::strings` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn strings(&self) -> Option<&Index<'a>> {
    match &self.top_dict.kind {
      CffFontKind::Sid { strings, .. } => Some(strings),
      CffFontKind::Cid { .. } | CffFontKind::Cff2 { .. } => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::string` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn string(&self, sid: Sid) -> Option<&'a [u8]> {
    match sid.resolve_standard() {
      Ok(s) => Some(s),
      Err(idx) => {
        let strings = self.strings()?;

        match strings.get(idx) {
          Ok(value) => Some(value),
          Err(_invalid_string_index) => None,
        }
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::charset` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn charset(&self) -> Option<Charset<'a>> {
    let Ok(value_30) = self.top_dict.charset_offset.get();

    match Charset::new(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        self.data,
      ),
      {
        let checked_value_2 = value_30?;
        checked_value_2
      },
      self.top_dict.charstrings.count(),
    ) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::encoding` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn encoding(&self) -> Option<Encoding<'a>> {
    let charset = self.charset()?;
    let Ok(value_31) = self.top_dict.encoding_offset.get();

    let encoding = match RawEncoding::new(self.data, {
      let checked_value_2 = value_31?;
      checked_value_2
    }) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }?;
    Some(Encoding { encoding, charset })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::matrix` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn matrix(&self) -> Option<&ScaledFontMatrix> {
    self.top_dict.matrix.as_ref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::upem` method returns `i32` directly in the existing parser interface"
    )
  )]
  pub fn upem(&self) -> i32 {
    self.upem
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::var_store` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn var_store(&self) -> Option<&ItemVariationStore<'a>> {
    self.top_dict.var_store.as_ref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::num_subfonts` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn num_subfonts(&self) -> u16 {
    match &self.top_dict.kind {
      CffFontKind::Sid { .. } => 1,
      CffFontKind::Cid { fd_array, .. } | CffFontKind::Cff2 { fd_array, .. } => {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(fd_array.count());
        migrated_value
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::subfont_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn subfont_index(&self, gid: GlyphId) -> Option<u16> {
    match &self.top_dict.kind {
      CffFontKind::Sid { .. } => Some(0),
      CffFontKind::Cid { fd_select, .. } | CffFontKind::Cff2 { fd_select, .. } => fd_select
        .as_ref()
        .map_or(Some(0), |fds| fds.font_index(gid)),
    }
  }

  pub fn subfont(&self, index: u16, coords: &[F2Dot14]) -> Result<Subfont, Error> {
    let Ok(blend) = self.blend_state(0, coords);

    match &self.top_dict.kind {
      CffFontKind::Sid { private_dict, .. } => Subfont::new(
        self.data,
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(private_dict.start);
          migrated_value
        })..({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(private_dict.end);
          migrated_value
        }),
        blend,
        None,
      ),
      CffFontKind::Cid { fd_array, .. } | CffFontKind::Cff2 { fd_array, .. } => {
        let checked_value = fd_array.get({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
          migrated_value
        })?;
        let font_dict = FontDict::new(checked_value)?;
        Subfont::new(
          self.data,
          font_dict.private_dict_range,
          blend,
          font_dict.matrix,
        )
      }
    }
  }

  pub fn subfont_hinted(
    &self,
    index: u16,
    coords: &[F2Dot14],
  ) -> Result<(Subfont, HintingParams), Error> {
    let Ok(blend) = self.blend_state(0, coords);

    match &self.top_dict.kind {
      CffFontKind::Sid { private_dict, .. } => Subfont::new_hinted(
        self.data,
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(private_dict.start);
          migrated_value
        })..({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(private_dict.end);
          migrated_value
        }),
        blend,
        None,
      ),
      CffFontKind::Cid { fd_array, .. } | CffFontKind::Cff2 { fd_array, .. } => {
        let checked_value = fd_array.get({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
          migrated_value
        })?;
        let font_dict = FontDict::new(checked_value)?;
        Subfont::new_hinted(
          self.data,
          font_dict.private_dict_range,
          blend,
          font_dict.matrix,
        )
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CffFontRef::transform` method returns `Transform` directly in the existing parser interface"
    )
  )]
  pub fn transform(&self, subfont: &Subfont, ppem: Option<f32>) -> Transform {
    let mut scale = ppem.map(|ppem_2| Transform::compute_scale(ppem_2, self.upem));

    let scaled_matrix = match self.top_dict.matrix {
      Some(top_matrix) => match subfont.matrix {
        Some(sub_matrix) => {
          let scaling = match ::core::primitive::bool::then_some(
            top_matrix.scale > 1 && sub_matrix.scale > 1,
            (),
          ) {
            ::core::option::Option::Some(()) => top_matrix.scale.min(sub_matrix.scale),
            ::core::option::Option::None => 1,
          };

          let matrix = transform::combine_scaled(&top_matrix.matrix, &sub_matrix.matrix, scaling);
          let Ok(value_2) = Fixed::from_bits(sub_matrix.scale);

          let Ok(value_14) = Fixed::from_bits(top_matrix.scale);
          let Ok(value_15) = Fixed::from_bits(scaling);

          let Ok(scaled_upem) = value_2.mul_div(value_14, value_15);

          let Ok(value_16) = scaled_upem.to_bits();

          Some(
            ScaledFontMatrix {
              matrix,
              scale: value_16,
            }
            .normalize(),
          )
        }
        None => Some(top_matrix),
      },
      None => subfont.matrix.map(|matrix| matrix.normalize()),
    };

    match scaled_matrix.as_ref() {
      Some(matrix) => match ::core::primitive::bool::then_some(matrix.scale != self.upem, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value_3) = Fixed::from_i32(64);

          let original_scale = match scale {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => value_3,
          };
          let Ok(value_17) = Fixed::from_bits(self.upem);
          let Ok(value_18) = Fixed::from_bits(matrix.scale);

          let Ok(value_4) = original_scale.mul_div(value_17, value_18);

          scale = Some(value_4);
        }
        ::core::option::Option::None => {}
      },
      None => {}
    }

    Transform {
      matrix: match scaled_matrix {
        Some(scaled) => scaled.matrix,
        None => Default::default(),
      },
      scale,
    }
  }

  pub fn evaluate_charstring(
    &self,
    subfont: &Subfont,
    gid: GlyphId,
    coords: &[F2Dot14],
    sink: &mut impl CommandSink,
  ) -> Result<Option<Fixed>, Error> {
    let charstrings = self.top_dict.charstrings.clone();
    let Ok(blend) = self.blend_state(subfont.vs_index, coords);
    let subrs = match ::core::primitive::bool::then_some(subfont.subrs_offset != 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_19) = console_core_number_conversion::indices_from(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u64>(subfont.subrs_offset);
            migrated_value
          })..,
        );

        let data = self.data.get(value_19).ok_or(ReadError::OutOfBounds)?;
        Index::new(data, self.version)?
      }
      ::core::option::Option::None => Index::Empty,
    };
    let Ok(value_5) = gid.to_u32();

    let charstring_data = charstrings.get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_5);
      migrated_value
    })?;
    let ctx = (self.data, &charstrings, &self.global_subrs, &subrs);

    let evaluate = cs::evaluate(&ctx, blend, charstring_data, sink)?;

    match evaluate {
      Some(width) => Ok(Some(width + subfont.nominal_width)),
      None => Ok(subfont.default_width),
    }
  }

  pub fn draw(
    &self,
    subfont: &Subfont,
    gid: GlyphId,
    coords: &[F2Dot14],
    ppem: Option<f32>,
    pen: &mut impl OutlinePen,
  ) -> Result<Option<f32>, Error> {
    let mut nop_filter = NopFilterSink::new(pen);
    let transform = self.transform(subfont, ppem);
    let mut transformer = TransformSink::new(&mut nop_filter, transform);
    let width = self.evaluate_charstring(subfont, gid, coords, &mut transformer)?;
    Ok(width.map(|w| {
      let Ok(value_6) = transform.transform_h_metric(w).to_f32();

      value_6.max(0.0)
    }))
  }

  #[must_use]
  fn blend_state(
    &self,
    vs_index: u16,
    coords: &'a [F2Dot14],
  ) -> core::result::Result<Option<BlendState<'a>>, console_core_never::Never> {
    Ok(self.top_dict.var_store.as_ref().and_then(|store| {
      match BlendState::new(store.clone(), coords, vs_index) {
        Ok(value) => Some(value),
        Err(_invalid_blend_state) => None,
      }
    }))
  }
}

#[derive(Clone)]
pub struct Encoding<'a> {
  encoding: RawEncoding<'a>,
  charset: Charset<'a>,
}

impl<'a> Encoding<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::predefined` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn predefined(&self) -> Option<PredefinedEncoding> {
    match &self.encoding {
      RawEncoding::Predefined(encoding) => Some(*encoding),
      RawEncoding::Custom(_) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::charset` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn charset(&self) -> &Charset<'a> {
    &self.charset
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Encoding::map` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn map(&self, code: u8) -> Option<GlyphId> {
    self.encoding.map(&self.charset, code)
  }
}

#[derive(Clone)]
enum CffFontKind<'a> {
  Sid {
    strings: Index<'a>,
    private_dict: Range<u32>,
  },
  Cid {
    fd_select: Option<FdSelect<'a>>,
    fd_array: Index<'a>,
  },
  Cff2 {
    fd_select: Option<FdSelect<'a>>,
    fd_array: Index<'a>,
  },
}

#[derive(Copy, Clone, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the published Subfont default represents empty local subroutines, widths, and matrix, with the default variation store index"
  )
)]
pub struct Subfont {
  subrs_offset: u32,
  default_width: Option<Fixed>,
  nominal_width: Fixed,
  matrix: Option<ScaledFontMatrix>,
  vs_index: u16,
}

impl Subfont {
  fn new(
    data: &[u8],
    range: Range<u64>,
    blend: Option<BlendState>,
    matrix: Option<ScaledFontMatrix>,
  ) -> Result<Self, Error> {
    let mut subfont = Self {
      matrix,
      ..Default::default()
    };
    let Ok(value_20) = console_core_number_conversion::indices(range.clone());

    let data_2 = data.get(value_20).ok_or(ReadError::OutOfBounds)?;

    for entry in dict::entries(data_2, blend).filter_map(|e| match e {
      Ok(value) => Some(value),
      Err(_unreadable_cff_dict_entry) => None,
    }) {
      match entry {
        dict::Entry::SubrsOffset(offset) => {
          subfont.subrs_offset = {
            let checked_value = range
              .start
              .checked_add(offset)
              .ok_or(ReadError::OutOfBounds)?;
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(checked_value);
            migrated_value
          };
        }
        dict::Entry::VariationStoreIndex(index) => subfont.vs_index = index,

        dict::Entry::DefaultWidthX(width) => {
          let Ok(value_7) = width.floor();

          subfont.default_width = Some(value_7)
        }
        dict::Entry::NominalWidthX(width) => {
          let Ok(value_8) = width.floor();

          subfont.nominal_width = value_8
        }
        dict::Entry::Version(_)
        | dict::Entry::Notice(_)
        | dict::Entry::FullName(_)
        | dict::Entry::FamilyName(_)
        | dict::Entry::Weight(_)
        | dict::Entry::FontBbox(_)
        | dict::Entry::CharstringsOffset(_)
        | dict::Entry::PrivateDictRange(_)
        | dict::Entry::VariationStoreOffset(_)
        | dict::Entry::Copyright(_)
        | dict::Entry::IsFixedPitch(_)
        | dict::Entry::ItalicAngle(_)
        | dict::Entry::UnderlinePosition(_)
        | dict::Entry::UnderlineThickness(_)
        | dict::Entry::PaintType(_)
        | dict::Entry::CharstringType(_)
        | dict::Entry::FontMatrix(_)
        | dict::Entry::StrokeWidth(_)
        | dict::Entry::FdArrayOffset(_)
        | dict::Entry::FdSelectOffset(_)
        | dict::Entry::BlueValues(_)
        | dict::Entry::OtherBlues(_)
        | dict::Entry::FamilyBlues(_)
        | dict::Entry::FamilyOtherBlues(_)
        | dict::Entry::BlueScale(_)
        | dict::Entry::BlueShift(_)
        | dict::Entry::BlueFuzz(_)
        | dict::Entry::LanguageGroup(_)
        | dict::Entry::ExpansionFactor(_)
        | dict::Entry::Encoding(_)
        | dict::Entry::Charset(_)
        | dict::Entry::UniqueId(_)
        | dict::Entry::Xuid
        | dict::Entry::SyntheticBase(_)
        | dict::Entry::PostScript(_)
        | dict::Entry::BaseFontName(_)
        | dict::Entry::BaseFontBlend
        | dict::Entry::Ros { .. }
        | dict::Entry::CidFontVersion(_)
        | dict::Entry::CidFontRevision(_)
        | dict::Entry::CidFontType(_)
        | dict::Entry::CidCount(_)
        | dict::Entry::UidBase(_)
        | dict::Entry::FontName(_)
        | dict::Entry::StdHw(_)
        | dict::Entry::StdVw(_)
        | dict::Entry::StemSnapH(_)
        | dict::Entry::StemSnapV(_)
        | dict::Entry::ForceBold(_)
        | dict::Entry::InitialRandomSeed(_) => {}
      }
    }

    Ok(subfont)
  }

  fn new_hinted(
    data: &[u8],
    range: Range<u64>,
    blend: Option<BlendState>,
    matrix: Option<ScaledFontMatrix>,
  ) -> Result<(Self, HintingParams), Error> {
    let mut subfont = Self {
      matrix,
      ..Default::default()
    };
    let mut params = HintingParams::default();
    let Ok(value_21) = console_core_number_conversion::indices(range.clone());

    let data_2 = data.get(value_21).ok_or(ReadError::OutOfBounds)?;

    for entry in dict::entries(data_2, blend).filter_map(|e| match e {
      Ok(value) => Some(value),
      Err(_unreadable_cff_dict_entry) => None,
    }) {
      match entry {
        dict::Entry::SubrsOffset(offset) => {
          subfont.subrs_offset = {
            let checked_value = range
              .start
              .checked_add(offset)
              .ok_or(ReadError::OutOfBounds)?;
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(checked_value);
            migrated_value
          };
        }
        dict::Entry::VariationStoreIndex(index) => subfont.vs_index = index,

        dict::Entry::DefaultWidthX(width) => {
          let Ok(value_9) = width.floor();

          subfont.default_width = Some(value_9)
        }
        dict::Entry::NominalWidthX(width) => {
          let Ok(value_10) = width.floor();

          subfont.nominal_width = value_10
        }
        dict::Entry::BlueValues(values) => params.blues = values,
        dict::Entry::FamilyBlues(values) => params.family_blues = values,
        dict::Entry::OtherBlues(values) => params.other_blues = values,
        dict::Entry::FamilyOtherBlues(values) => params.family_other_blues = values,
        dict::Entry::BlueScale(value) => params.blue_scale = value,
        dict::Entry::BlueShift(value) => params.blue_shift = value,
        dict::Entry::BlueFuzz(value) => params.blue_fuzz = value,
        dict::Entry::LanguageGroup(group) => params.language_group = group,
        dict::Entry::Version(_)
        | dict::Entry::Notice(_)
        | dict::Entry::FullName(_)
        | dict::Entry::FamilyName(_)
        | dict::Entry::Weight(_)
        | dict::Entry::FontBbox(_)
        | dict::Entry::CharstringsOffset(_)
        | dict::Entry::PrivateDictRange(_)
        | dict::Entry::VariationStoreOffset(_)
        | dict::Entry::Copyright(_)
        | dict::Entry::IsFixedPitch(_)
        | dict::Entry::ItalicAngle(_)
        | dict::Entry::UnderlinePosition(_)
        | dict::Entry::UnderlineThickness(_)
        | dict::Entry::PaintType(_)
        | dict::Entry::CharstringType(_)
        | dict::Entry::FontMatrix(_)
        | dict::Entry::StrokeWidth(_)
        | dict::Entry::FdArrayOffset(_)
        | dict::Entry::FdSelectOffset(_)
        | dict::Entry::ExpansionFactor(_)
        | dict::Entry::Encoding(_)
        | dict::Entry::Charset(_)
        | dict::Entry::UniqueId(_)
        | dict::Entry::Xuid
        | dict::Entry::SyntheticBase(_)
        | dict::Entry::PostScript(_)
        | dict::Entry::BaseFontName(_)
        | dict::Entry::BaseFontBlend
        | dict::Entry::Ros { .. }
        | dict::Entry::CidFontVersion(_)
        | dict::Entry::CidFontRevision(_)
        | dict::Entry::CidFontType(_)
        | dict::Entry::CidCount(_)
        | dict::Entry::UidBase(_)
        | dict::Entry::FontName(_)
        | dict::Entry::StdHw(_)
        | dict::Entry::StdVw(_)
        | dict::Entry::StemSnapH(_)
        | dict::Entry::StemSnapV(_)
        | dict::Entry::ForceBold(_)
        | dict::Entry::InitialRandomSeed(_) => {}
      }
    }

    Ok((subfont, params))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subfont::subrs_offset` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn subrs_offset(&self) -> u32 {
    self.subrs_offset
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subfont::default_width` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn default_width(&self) -> Option<Fixed> {
    self.default_width
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subfont::nominal_width` method returns `Fixed` directly in the existing parser interface"
    )
  )]
  pub fn nominal_width(&self) -> Fixed {
    self.nominal_width
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subfont::matrix` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn matrix(&self) -> Option<&ScaledFontMatrix> {
    self.matrix.as_ref()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Subfont::vs_index` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn vs_index(&self) -> u16 {
    self.vs_index
  }
}

#[derive(Copy, Clone, Default)]
struct MaybeOffset(Option<core::num::NonZeroU32>);

impl MaybeOffset {
  fn new(offset: u64) -> Result<Self, console_core_never::Never> {
    let Ok(offset32) = console_core_number_conversion::wrapped::<_, u32>(offset);
    Ok(Self(core::num::NonZeroU32::new(offset32.wrapping_add(1))))
  }

  fn get(self) -> Result<Option<u64>, console_core_never::Never> {
    Ok(
      self
        .0
        .map(|encoded| u64::from(encoded.get().wrapping_sub(1))),
    )
  }
}

#[derive(Clone)]
struct TopDict<'a> {
  charstrings: Index<'a>,
  charset_offset: MaybeOffset,
  encoding_offset: MaybeOffset,
  matrix: Option<ScaledFontMatrix>,
  var_store: Option<ItemVariationStore<'a>>,
  kind: CffFontKind<'a>,
}

impl<'a> TopDict<'a> {
  fn new(
    cff_data: &'a [u8],
    top_dict_data: &[u8],
    strings: Index<'a>,
    version: CffVersion,
  ) -> Result<Self, Error> {
    let mut ros = crate::Presence::Absent;
    let mut charstrings = None;
    let [mut charset_offset, mut encoding_offset] = match version {
      CffVersion::Cff2 => [MaybeOffset::default(); 2],
      CffVersion::Cff1 => {
        let Ok(offset) = MaybeOffset::new(0);
        [offset; 2]
      }
    };
    let mut fd_array = None;
    let mut fd_select = None;
    let mut private_dict_range = 0..0;
    let mut matrix = None;
    let mut var_store = None;

    for entry in dict::entries(top_dict_data, None).filter_map(|e| match e {
      Ok(value) => Some(value),
      Err(_unreadable_cff_dict_entry) => None,
    }) {
      match entry {
        dict::Entry::Ros { .. } => ros = crate::Presence::Present,
        dict::Entry::CharstringsOffset(offset) => {
          let Ok(value_25) = console_core_number_conversion::indices_from(offset..);

          charstrings = cff_data
            .get(value_25)
            .and_then(|data| match Index::new(data, version) {
              Ok(value) => Some(value),
              Err(_malformed_subroutine_index) => None,
            });
        }
        dict::Entry::Charset(offset) => {
          let Ok(decoded) = MaybeOffset::new(offset);
          charset_offset = decoded;
        }
        dict::Entry::Encoding(offset) => {
          let Ok(decoded) = MaybeOffset::new(offset);
          encoding_offset = decoded;
        }
        dict::Entry::FdArrayOffset(offset) => {
          let Ok(value_26) = console_core_number_conversion::indices_from(offset..);

          fd_array = cff_data
            .get(value_26)
            .and_then(|data| match Index::new(data, version) {
              Ok(value) => Some(value),
              Err(_malformed_subroutine_index) => None,
            });
        }
        dict::Entry::FdSelectOffset(offset) => {
          let Ok(value_27) = console_core_number_conversion::indices_from(offset..);

          fd_select = cff_data.get(value_27).and_then(|data| {
            match FdSelect::read(<crate::font_data::FontData<'_> as ::core::convert::From<
              &[::core::primitive::u8],
            >>::from(data))
            {
              ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
              ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
            }
          });
        }
        dict::Entry::PrivateDictRange(range) => {
          let Ok(value_28) = console_core_number_conversion::indices(range.clone());

          let _ = cff_data.get(value_28).ok_or(ReadError::OutOfBounds)?;
          private_dict_range = range;
        }
        dict::Entry::FontMatrix(font_matrix) => {
          matrix = Some(font_matrix.normalize());
        }
        dict::Entry::VariationStoreOffset(offset) => match version {
          CffVersion::Cff2 => {
            let offset_2 = offset.checked_add(2).ok_or(ReadError::OutOfBounds)?;
            let Ok(value_29) = console_core_number_conversion::indices_from(offset_2..);

            let read = ItemVariationStore::read(
              <crate::font_data::FontData<'_> as ::core::convert::From<
                &[::core::primitive::u8],
              >>::from(match cff_data.get(value_29) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => &[],
              }),
            )?;
            var_store = Some(read);
          }
          CffVersion::Cff1 => {}
        },
        dict::Entry::Version(_)
        | dict::Entry::Notice(_)
        | dict::Entry::FullName(_)
        | dict::Entry::FamilyName(_)
        | dict::Entry::Weight(_)
        | dict::Entry::FontBbox(_)
        | dict::Entry::Copyright(_)
        | dict::Entry::IsFixedPitch(_)
        | dict::Entry::ItalicAngle(_)
        | dict::Entry::UnderlinePosition(_)
        | dict::Entry::UnderlineThickness(_)
        | dict::Entry::PaintType(_)
        | dict::Entry::CharstringType(_)
        | dict::Entry::StrokeWidth(_)
        | dict::Entry::BlueValues(_)
        | dict::Entry::OtherBlues(_)
        | dict::Entry::FamilyBlues(_)
        | dict::Entry::FamilyOtherBlues(_)
        | dict::Entry::SubrsOffset(_)
        | dict::Entry::VariationStoreIndex(_)
        | dict::Entry::BlueScale(_)
        | dict::Entry::BlueShift(_)
        | dict::Entry::BlueFuzz(_)
        | dict::Entry::LanguageGroup(_)
        | dict::Entry::ExpansionFactor(_)
        | dict::Entry::UniqueId(_)
        | dict::Entry::Xuid
        | dict::Entry::SyntheticBase(_)
        | dict::Entry::PostScript(_)
        | dict::Entry::BaseFontName(_)
        | dict::Entry::BaseFontBlend
        | dict::Entry::CidFontVersion(_)
        | dict::Entry::CidFontRevision(_)
        | dict::Entry::CidFontType(_)
        | dict::Entry::CidCount(_)
        | dict::Entry::UidBase(_)
        | dict::Entry::FontName(_)
        | dict::Entry::StdHw(_)
        | dict::Entry::StdVw(_)
        | dict::Entry::DefaultWidthX(_)
        | dict::Entry::NominalWidthX(_)
        | dict::Entry::StemSnapH(_)
        | dict::Entry::StemSnapV(_)
        | dict::Entry::ForceBold(_)
        | dict::Entry::InitialRandomSeed(_) => {}
      }
    }

    let charstrings_2 = charstrings.ok_or(Error::MissingCharstrings)?;
    let kind = match fd_array {
      Some(fd_array_2) => match version {
        CffVersion::Cff2 => CffFontKind::Cff2 {
          fd_array: fd_array_2,
          fd_select,
        },
        CffVersion::Cff1 => CffFontKind::Cid {
          fd_array: fd_array_2,
          fd_select,
        },
      },
      None => {
        match (ros, version) {
          (crate::Presence::Present, _) | (_, CffVersion::Cff2) => {
            return Err(Error::MissingFdArray);
          }
          (crate::Presence::Absent, CffVersion::Cff1) => {}
        }

        CffFontKind::Sid {
          strings,
          private_dict: ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(private_dict_range.start);
            migrated_value
          })..({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(private_dict_range.end);
            migrated_value
          }),
        }
      }
    };
    Ok(Self {
      charset_offset,
      encoding_offset,
      charstrings: charstrings_2,
      matrix,
      kind,
      var_store,
    })
  }
}

struct FontDict {
  private_dict_range: Range<u64>,
  matrix: Option<ScaledFontMatrix>,
}

impl FontDict {
  fn new(dict_data: &[u8]) -> Result<Self, Error> {
    let mut range = None;
    let mut matrix = None;

    for entry in dict::entries(dict_data, None) {
      match {
        let checked_value_3 = entry?;
        checked_value_3
      } {
        dict::Entry::PrivateDictRange(dict_range) => {
          range = Some(dict_range);
        }
        dict::Entry::FontMatrix(font_matrix) => {
          matrix = Some(font_matrix);
        }
        dict::Entry::Version(_)
        | dict::Entry::Notice(_)
        | dict::Entry::FullName(_)
        | dict::Entry::FamilyName(_)
        | dict::Entry::Weight(_)
        | dict::Entry::FontBbox(_)
        | dict::Entry::CharstringsOffset(_)
        | dict::Entry::VariationStoreOffset(_)
        | dict::Entry::Copyright(_)
        | dict::Entry::IsFixedPitch(_)
        | dict::Entry::ItalicAngle(_)
        | dict::Entry::UnderlinePosition(_)
        | dict::Entry::UnderlineThickness(_)
        | dict::Entry::PaintType(_)
        | dict::Entry::CharstringType(_)
        | dict::Entry::StrokeWidth(_)
        | dict::Entry::FdArrayOffset(_)
        | dict::Entry::FdSelectOffset(_)
        | dict::Entry::BlueValues(_)
        | dict::Entry::OtherBlues(_)
        | dict::Entry::FamilyBlues(_)
        | dict::Entry::FamilyOtherBlues(_)
        | dict::Entry::SubrsOffset(_)
        | dict::Entry::VariationStoreIndex(_)
        | dict::Entry::BlueScale(_)
        | dict::Entry::BlueShift(_)
        | dict::Entry::BlueFuzz(_)
        | dict::Entry::LanguageGroup(_)
        | dict::Entry::ExpansionFactor(_)
        | dict::Entry::Encoding(_)
        | dict::Entry::Charset(_)
        | dict::Entry::UniqueId(_)
        | dict::Entry::Xuid
        | dict::Entry::SyntheticBase(_)
        | dict::Entry::PostScript(_)
        | dict::Entry::BaseFontName(_)
        | dict::Entry::BaseFontBlend
        | dict::Entry::Ros { .. }
        | dict::Entry::CidFontVersion(_)
        | dict::Entry::CidFontRevision(_)
        | dict::Entry::CidFontType(_)
        | dict::Entry::CidCount(_)
        | dict::Entry::UidBase(_)
        | dict::Entry::FontName(_)
        | dict::Entry::StdHw(_)
        | dict::Entry::StdVw(_)
        | dict::Entry::DefaultWidthX(_)
        | dict::Entry::NominalWidthX(_)
        | dict::Entry::StemSnapH(_)
        | dict::Entry::StemSnapV(_)
        | dict::Entry::ForceBold(_)
        | dict::Entry::InitialRandomSeed(_) => {}
      }
    }

    let private_dict_range = range.ok_or(Error::MissingPrivateDict)?;

    Ok(Self {
      private_dict_range,
      matrix,
    })
  }
}

#[derive(Clone, Debug)]
pub struct Metadata<'a> {
  name: Option<&'a str>,
  full_name: Option<&'a str>,
  family_name: Option<&'a str>,
  weight: Option<&'a str>,
  bbox: BoundingBox<Fixed>,
  italic_angle: Fixed,
  pitch: Pitch,
  underline_position: Fixed,
  underline_thickness: Fixed,
}

impl<'a> Metadata<'a> {
  #[must_use]
  fn new(
    data: &'a [u8],
    top_dict_index: u16,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(value_11) = FontData::new(data);

    let cff = match match cff::Cff::read(value_11) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let strings = cff.strings();
    let get_str = |sid: Sid| {
      let bytes = match sid.resolve_standard() {
        Ok(bytes) => bytes,
        Err(idx) => {
          (match strings.get(idx) {
            Ok(value) => Some(value),
            Err(_invalid_string_index) => None,
          })?
        }
      };

      match core::str::from_utf8(bytes) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
      }
    };
    let top_dict_data = match match cff.top_dicts().get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(top_dict_index);
      migrated_value
    }) {
      Ok(value) => Some(value),
      Err(_invalid_top_dictionary_index) => None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let name = cff
      .name({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(top_dict_index);
        migrated_value
      })
      .and_then(|bytes| match core::str::from_utf8(bytes) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
      });
    let mut meta = Metadata {
      name,
      ..Default::default()
    };

    for entry in dict::entries(top_dict_data, None).filter_map(|e| match e {
      Ok(value) => Some(value),
      Err(_unreadable_cff_dict_entry) => None,
    }) {
      match entry {
        dict::Entry::FullName(sid) => meta.full_name = get_str(sid),
        dict::Entry::FamilyName(sid) => meta.family_name = get_str(sid),
        dict::Entry::Weight(sid) => meta.weight = get_str(sid),
        dict::Entry::FontBbox([x_min, y_min, x_max, y_max]) => {
          meta.bbox = BoundingBox {
            x_min,
            x_max,
            y_min,
            y_max,
          }
        }
        dict::Entry::ItalicAngle(angle) => meta.italic_angle = angle,
        dict::Entry::IsFixedPitch(pitch) => meta.pitch = pitch,
        dict::Entry::UnderlinePosition(pos) => meta.underline_position = pos,
        dict::Entry::UnderlineThickness(size) => meta.underline_thickness = size,
        dict::Entry::Version(_)
        | dict::Entry::Notice(_)
        | dict::Entry::CharstringsOffset(_)
        | dict::Entry::PrivateDictRange(_)
        | dict::Entry::VariationStoreOffset(_)
        | dict::Entry::Copyright(_)
        | dict::Entry::PaintType(_)
        | dict::Entry::CharstringType(_)
        | dict::Entry::FontMatrix(_)
        | dict::Entry::StrokeWidth(_)
        | dict::Entry::FdArrayOffset(_)
        | dict::Entry::FdSelectOffset(_)
        | dict::Entry::BlueValues(_)
        | dict::Entry::OtherBlues(_)
        | dict::Entry::FamilyBlues(_)
        | dict::Entry::FamilyOtherBlues(_)
        | dict::Entry::SubrsOffset(_)
        | dict::Entry::VariationStoreIndex(_)
        | dict::Entry::BlueScale(_)
        | dict::Entry::BlueShift(_)
        | dict::Entry::BlueFuzz(_)
        | dict::Entry::LanguageGroup(_)
        | dict::Entry::ExpansionFactor(_)
        | dict::Entry::Encoding(_)
        | dict::Entry::Charset(_)
        | dict::Entry::UniqueId(_)
        | dict::Entry::Xuid
        | dict::Entry::SyntheticBase(_)
        | dict::Entry::PostScript(_)
        | dict::Entry::BaseFontName(_)
        | dict::Entry::BaseFontBlend
        | dict::Entry::Ros { .. }
        | dict::Entry::CidFontVersion(_)
        | dict::Entry::CidFontRevision(_)
        | dict::Entry::CidFontType(_)
        | dict::Entry::CidCount(_)
        | dict::Entry::UidBase(_)
        | dict::Entry::FontName(_)
        | dict::Entry::StdHw(_)
        | dict::Entry::StdVw(_)
        | dict::Entry::DefaultWidthX(_)
        | dict::Entry::NominalWidthX(_)
        | dict::Entry::StemSnapH(_)
        | dict::Entry::StemSnapV(_)
        | dict::Entry::ForceBold(_)
        | dict::Entry::InitialRandomSeed(_) => {}
      }
    }

    Ok(Some(meta))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn name(&self) -> Option<&'a str> {
    self.name
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::full_name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn full_name(&self) -> Option<&'a str> {
    self.full_name
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::family_name` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn family_name(&self) -> Option<&'a str> {
    self.family_name
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::weight` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn weight(&self) -> Option<&'a str> {
    self.weight
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::italic_angle` method returns `Fixed` directly in the existing parser interface"
    )
  )]
  pub fn italic_angle(&self) -> Fixed {
    self.italic_angle
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::pitch` method returns `Pitch` directly in the existing parser interface"
    )
  )]
  pub fn pitch(&self) -> Pitch {
    self.pitch
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::underline_position` method returns `Fixed` directly in the existing parser interface"
    )
  )]
  pub fn underline_position(&self) -> Fixed {
    self.underline_position
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::underline_thickness` method returns `Fixed` directly in the existing parser interface"
    )
  )]
  pub fn underline_thickness(&self) -> Fixed {
    self.underline_thickness
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Metadata::bbox` method returns `BoundingBox` directly in the existing parser interface"
    )
  )]
  pub fn bbox(&self) -> BoundingBox<Fixed> {
    self.bbox
  }
}

impl Default for Metadata<'_> {
  fn default() -> Self {
    let Ok(value_12) = Fixed::from_i32(-100);
    let Ok(value_13) = Fixed::from_i32(50);

    Self {
      name: None,
      full_name: None,
      family_name: None,
      weight: None,
      bbox: BoundingBox::default(),
      italic_angle: Fixed::ZERO,
      pitch: Pitch::Proportional,
      underline_position: value_12,
      underline_thickness: value_13,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    FontData, FontRef, TableProvider,
    ps::{hinting::Blues, transform::FontMatrix},
  };
  use cs::test_helpers::*;

  #[test]
  fn read_cff_static() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    assert_eq!(cff.version(), 1);
    assert!(cff.var_store().is_none());

    let private_dict = match &cff.top_dict.kind {
      CffFontKind::Sid {
        private_dict: matched_private_dict,
        ..
      } => matched_private_dict,
      crate::ps::cff::font::CffFontKind::Cid { .. }
      | crate::ps::cff::font::CffFontKind::Cff2 { .. } => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "this is an SID font"
        )));
      }
    };

    assert!(!private_dict.is_empty());
    assert_eq!((cff.num_glyphs()), 5);
    assert_eq!(cff.num_subfonts(), 1);
    assert_eq!(
      cff.subfont_index({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }),
      Some(0)
    );
    assert_eq!(cff.global_subrs.count(), 17);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_cff_metadata() -> Result<(), crate::test_support::Failure> {
    let cases = [
      (
        font_test_data::NOTO_SERIF_DISPLAY_TRIMMED,
        "NotoSerifDisplay-Regular",
        "Noto Serif Display Regular",
        Some("Noto Serif Display"),
        [-693, -470, 2797, 1048],
      ),
      (
        font_test_data::MATERIAL_ICONS_SUBSET,
        "GoogleMaterialIcons-Regular",
        "GoogleMaterialIcons-Regular",
        None,
        [-1, 0, 513, 512],
      ),
    ];
    let Ok(underline_position) = Fixed::from_i32(-100);
    let Ok(underline_thickness) = Fixed::from_i32(50);

    for (bytes, name, full_name, family_name, bounds) in cases {
      let font = FontRef::new(bytes)?;
      let table = font.cff()?;
      let table_data = table.offset_data();
      let Ok(cff_bytes) = table_data.as_bytes();
      let cff = CffFontRef::new(cff_bytes, 0, None)?;
      let metadata = crate::test_support::fixture_value(cff.metadata(), "CFF metadata")?;
      let [x_min, y_min, x_max, y_max] = bounds.map(|coordinate| {
        let Ok(value) = Fixed::from_i32(coordinate);
        value
      });
      assert_eq!(metadata.name(), Some(name));
      assert_eq!(metadata.full_name(), Some(full_name));
      assert_eq!(metadata.family_name(), family_name);
      assert_eq!(metadata.weight(), None);
      assert_eq!(
        metadata.bbox(),
        BoundingBox {
          x_min,
          y_min,
          x_max,
          y_max
        }
      );
      assert_eq!(metadata.italic_angle(), Fixed::ZERO);
      assert_eq!(metadata.pitch(), Pitch::Proportional);
      assert_eq!(metadata.underline_position(), underline_position);
      assert_eq!(metadata.underline_thickness(), underline_thickness);
    }

    Ok(())
  }

  #[test]
  fn read_cff2_static() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff2(
        {
          let Ok(explicit_value) = match font.cff2() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        None,
      ),
      "fixture value",
    )?;
    assert_eq!(cff.version(), 2);
    assert!(cff.var_store().is_some());

    let fd_array = match &cff.top_dict.kind {
      CffFontKind::Cff2 {
        fd_array: matched_fd_array,
        ..
      } => matched_fd_array,
      crate::ps::cff::font::CffFontKind::Sid { .. }
      | crate::ps::cff::font::CffFontKind::Cid { .. } => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "this is a CFF2 font"
        )));
      }
    };

    assert_eq!(fd_array.count(), 1);
    assert_eq!((cff.num_glyphs()), 6);
    assert_eq!(cff.num_subfonts(), 1);
    assert_eq!(
      cff.subfont_index({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }),
      Some(0)
    );
    assert_eq!(cff.global_subrs.count(), 0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn read_example_cff2_table() -> ::core::result::Result<(), crate::test_support::Failure> {
    let cff = CffFontRef::new_cff2(font_test_data::cff2::EXAMPLE, None)?;
    assert_eq!(cff.version(), 2);
    assert!(cff.var_store().is_some());

    let fd_array = match &cff.top_dict.kind {
      CffFontKind::Cff2 {
        fd_array: matched_fd_array,
        ..
      } => matched_fd_array,
      crate::ps::cff::font::CffFontKind::Sid { .. }
      | crate::ps::cff::font::CffFontKind::Cid { .. } => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "this is a CFF2 font"
        )));
      }
    };

    assert_eq!(fd_array.count(), 1);
    assert_eq!((cff.num_glyphs()), 2);
    assert_eq!(cff.num_subfonts(), 1);
    assert_eq!(
      cff.subfont_index({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }),
      Some(0)
    );
    assert_eq!(cff.global_subrs.count(), 0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn charset() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED),
      "fixture value",
    )?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let fixture_value = crate::test_support::fixture_value(font.cff(), "fixture value")?;
          let Ok(explicit_value) = fixture_value.offset_data().as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    let charset = crate::test_support::fixture_value(cff.charset(), "fixture value")?;
    let glyph_names = charset
      .iter()
      .map(|(glyph, sid)| -> Result<_, crate::test_support::Failure> {
        let Ok(id) = glyph.to_u32();
        let bytes =
          crate::test_support::fixture_value(sid.resolve_standard(), "standard CFF glyph name")?;
        let name = std::str::from_utf8(bytes)?;
        Ok((id, name))
      })
      .collect::<Result<Vec<_>, _>>()?;
    let expected = [(0, ".notdef"), (1, "i"), (2, "j"), (3, "k"), (4, "l")];

    assert_eq!(glyph_names, expected);
    Ok(())
  }

  #[test]
  fn top_dict_ivs_offset_overflow() {
    let top_dict = font_test_data::be_buffer! { (29u8), (-1i32), (24u8) }.to_vec();

    assert!(
      (match &(TopDict::new(&[], &top_dict, Index::Empty, CffVersion::Cff2)) {
        Ok(_validated_input) => false,
        Err(_invalid_fixture_input) => true,
      })
    );
  }

  #[test]
  fn subrs_offset_overflow() {
    let private_dict = font_test_data::be_buffer! { (0u32), (29u8), (-1i32), (19u8) }.to_vec();

    assert!(
      (match &(Subfont::new(
        &private_dict,
        4..({
          let Ok(explicit_index) =
            console_core_number_conversion::fitted::<_, u64>(private_dict.len());
          explicit_index
        }),
        None,
        None
      )) {
        Ok(_validated_input) => false,
        Err(_invalid_fixture_input) => true,
      })
    );
  }

  #[test]
  fn empty_private_dict() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;

    let private_dict = match &cff.top_dict.kind {
      CffFontKind::Sid {
        private_dict: matched_private_dict,
        ..
      } => matched_private_dict,
      crate::ps::cff::font::CffFontKind::Cid { .. }
      | crate::ps::cff::font::CffFontKind::Cff2 { .. } => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "this is an SID font"
        )));
      }
    };

    assert!(private_dict.is_empty());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn capture_family_other_blues() -> ::core::result::Result<(), crate::test_support::Failure> {
    let private_dict_data = crate::test_support::fixture_value(
      font_test_data::cff2::EXAMPLE.get(0x4f..=0xc0),
      "CFF2 fixture private DICT",
    )?;
    let variation_data = crate::test_support::fixture_value(
      font_test_data::cff2::EXAMPLE.get(18..),
      "CFF2 fixture variation store bytes",
    )?;
    let Ok(store_data) = FontData::new(variation_data);
    let store = ItemVariationStore::read(store_data)?;
    let coords = &[{
      let Ok(value) = F2Dot14::from_f32(0.0);
      value
    }];
    let blend_state =
      crate::test_support::fixture_value(BlendState::new(store, coords, 0), "fixture value")?;
    let (_subfont, hint_params) = crate::test_support::fixture_value(
      Subfont::new_hinted(
        private_dict_data,
        0..({
          let Ok(explicit_index) =
            console_core_number_conversion::fitted::<_, u64>(private_dict_data.len());
          explicit_index
        }),
        Some(blend_state),
        None,
      ),
      "fixture value",
    )?;

    assert_eq!(
      hint_params.family_other_blues,
      Blues::new(
        [-249.0, -239.0]
          .map(|argument| {
            let Ok(value_22) = Fixed::from_f64(argument);

            value_22
          })
          .into_iter()
      )
    );
    Ok(())
  }

  #[test]
  fn subfont_cff() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    let subfont = crate::test_support::fixture_value(cff.subfont(0, &[]), "fixture value")?;
    assert_eq!(subfont.default_width, None);
    assert_eq!(
      subfont.nominal_width,
      ({
        let Ok(explicit_value) = Fixed::from_i32(598);
        explicit_value
      })
    );
    assert_eq!(subfont.vs_index, 0);
    assert_eq!(subfont.matrix, None);
    ::core::result::Result::Ok(())
  }

  #[must_use]
  fn make_blues<const N: usize>(
    values: [i32; N],
  ) -> core::result::Result<Blues, console_core_never::Never> {
    Ok(Blues::new(
      values
        .map(|argument| {
          let Ok(value_23) = Fixed::from_i32(argument);

          value_23
        })
        .into_iter(),
    ))
  }

  #[test]
  fn hinted_subfont_cff() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    let (subfont, hinting) =
      crate::test_support::fixture_value(cff.subfont_hinted(0, &[]), "fixture value")?;
    assert_eq!(subfont.default_width, None);
    assert_eq!(
      subfont.nominal_width,
      ({
        let Ok(explicit_value) = Fixed::from_i32(598);
        explicit_value
      })
    );
    assert_eq!(subfont.vs_index, 0);
    assert_eq!(subfont.matrix, None);
    let Ok(blues) = make_blues([-15, 0, 536, 547, 571, 582, 714, 726, 760, 772]);
    let expected_hinting = HintingParams {
      blues,
      family_blues: Blues::default(),
      other_blues: ({
        let Ok(explicit_value) = make_blues([-255, -240]);
        explicit_value
      }),
      family_other_blues: Blues::default(),
      blue_scale: ({
        let Ok(explicit_value) = Fixed::from_f64(0.0500030517578125);
        explicit_value
      }),
      blue_shift: ({
        let Ok(explicit_value) = Fixed::from_f64(7.0);
        explicit_value
      }),
      blue_fuzz: Fixed::ZERO,
      language_group: 0,
    };
    assert_eq!(hinting, expected_hinting);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn subfont_cff2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff2(
        {
          let Ok(explicit_value) = match font.cff2() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        None,
      ),
      "fixture value",
    )?;
    let subfont = crate::test_support::fixture_value(cff.subfont(0, &[]), "fixture value")?;
    assert_eq!(subfont.default_width, None);
    assert_eq!(subfont.nominal_width, Fixed::ZERO);
    assert_eq!(subfont.vs_index, 0);
    assert_eq!(subfont.matrix, None);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn hinted_subfont_cff2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff2(
        {
          let Ok(explicit_value) = match font.cff2() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        None,
      ),
      "fixture value",
    )?;
    let (subfont, hinting) =
      crate::test_support::fixture_value(cff.subfont_hinted(0, &[]), "fixture value")?;
    assert_eq!(subfont.default_width, None);
    assert_eq!(subfont.nominal_width, Fixed::ZERO);
    assert_eq!(subfont.vs_index, 0);
    assert_eq!(subfont.matrix, None);
    let Ok(blues) = make_blues([-10, 0, 482, 492, 694, 704, 739, 749]);
    let expected_hinting = HintingParams {
      blues,
      family_blues: Blues::default(),
      other_blues: ({
        let Ok(explicit_value) = make_blues([-227, -217]);
        explicit_value
      }),
      family_other_blues: Blues::default(),
      blue_scale: ({
        let Ok(explicit_value) = Fixed::from_f64(0.0625);
        explicit_value
      }),
      blue_shift: ({
        let Ok(explicit_value) = Fixed::from_f64(7.0);
        explicit_value
      }),
      blue_fuzz: Fixed::ONE,
      language_group: 0,
    };
    assert_eq!(hinting, expected_hinting);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn subfont_matrix() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET_MATRIX)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    let subfont = crate::test_support::fixture_value(cff.subfont(0, &[]), "fixture value")?;
    assert_eq!(subfont.default_width, None);
    assert_eq!(subfont.nominal_width, Fixed::ZERO);
    assert_eq!(subfont.vs_index, 0);
    let Ok(expected_matrix) = FontMatrix::from_elements([
      ({
        let Ok(explicit_value) = Fixed::from_i32(5);
        explicit_value
      }),
      Fixed::ZERO,
      Fixed::ZERO,
      {
        let Ok(value) = Fixed::from_i32(5);
        value
      },
      Fixed::ZERO,
      Fixed::ZERO,
    ]);
    let expected_scale = 10;
    assert_eq!(
      subfont.matrix,
      Some(ScaledFontMatrix {
        matrix: expected_matrix,
        scale: expected_scale
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn eval_charstring_cff() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    let mut sink = CharstringCommandCounter::default();
    let subfont = crate::test_support::fixture_value(cff.subfont(0, &[]), "fixture value")?;
    crate::test_support::fixture_value(
      cff.evaluate_charstring(
        &subfont,
        {
          let Ok(explicit_value) = GlyphId::new(2);
          explicit_value
        },
        &[],
        &mut sink,
      ),
      "fixture value",
    )?;

    assert_eq!(sink.0, 18);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn eval_charstring_cff2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new_cff2(
        {
          let Ok(explicit_value) = match font.cff2() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        None,
      ),
      "fixture value",
    )?;
    let mut sink = CharstringCommandCounter::default();
    let subfont = crate::test_support::fixture_value(cff.subfont(0, &[]), "fixture value")?;
    crate::test_support::fixture_value(
      cff.evaluate_charstring(
        &subfont,
        {
          let Ok(explicit_value) = GlyphId::new(2);
          explicit_value
        },
        &[],
        &mut sink,
      ),
      "fixture value",
    )?;

    assert_eq!(sink.0, 11);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn select_version() -> ::core::result::Result<(), crate::test_support::Failure> {
    let fixture_value = crate::test_support::fixture_value(
      CffFontRef::new(
        {
          let Ok(explicit_value) = match match FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)
          {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .cff2()
          {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    assert_eq!(fixture_value.version(), 2);

    assert_eq!(
      {
        let matrix_fixture_font = crate::test_support::fixture_value(
          CffFontRef::new(
            {
              let Ok(explicit_value) =
                match match FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET_MATRIX) {
                  ::core::result::Result::Ok(read_value) => read_value,
                  ::core::result::Result::Err(read_error) => {
                    return ::core::result::Result::Err(crate::test_support::Failure::from(
                      read_error,
                    ));
                  }
                }
                .cff()
                {
                  ::core::result::Result::Ok(read_value) => read_value,
                  ::core::result::Result::Err(read_error) => {
                    return ::core::result::Result::Err(crate::test_support::Failure::from(
                      read_error,
                    ));
                  }
                }
                .offset_data()
                .as_bytes();
              explicit_value
            },
            0,
            None,
          ),
          "fixture value",
        )?;
        matrix_fixture_font
      }
      .version(),
      1
    );

    assert!(
      (match &(CffFontRef::new(&[0, 1, 4, 5], 0, None)) {
        Ok(_validated_input) => false,
        Err(_invalid_fixture_input) => true,
      })
    );

    assert!(
      (match &(CffFontRef::new(&[1, 1, 4, 5], 0, None)) {
        Ok(_validated_input) => false,
        Err(_invalid_fixture_input) => true,
      })
    );

    assert!(
      (match &(CffFontRef::new(&[2, 1, 4, 5], 0, None)) {
        Ok(_validated_input) => false,
        Err(_invalid_fixture_input) => true,
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn transform() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET_MATRIX)?;

    let cff = crate::test_support::fixture_value(
      CffFontRef::new(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        Some(512),
      ),
      "fixture value",
    )?;
    let subfont = crate::test_support::fixture_value(cff.subfont(0, &[]), "fixture value")?;

    let expected_matrix = [65536, 0, 5604, 65536, 0, 0].map(|argument| {
      let Ok(value_24) = Fixed::from_bits(argument);

      value_24
    });

    let transform = cff.transform(&subfont, None);
    assert_eq!(
      ({
        let Ok(value) = transform.matrix.elements();
        value
      }),
      expected_matrix
    );
    assert_eq!(
      transform.scale,
      Some({
        let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(32, 16_u32));
        value
      })
    );

    let transform_2 = cff.transform(&subfont, Some(16.0));
    assert_eq!(
      ({
        let Ok(value) = transform_2.matrix.elements();
        value
      }),
      expected_matrix
    );
    assert_eq!(
      transform_2.scale,
      Some({
        let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(1, 16_u32));
        value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cff_encoding() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new(
        {
          let Ok(explicit_value) = match font.cff() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    let encoding = crate::test_support::fixture_value(cff.encoding(), "fixture value")?;
    assert_eq!(encoding.predefined(), Some(PredefinedEncoding::Standard));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn cff2_lacks_encoding() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let cff = crate::test_support::fixture_value(
      CffFontRef::new(
        {
          let Ok(explicit_value) = match font.cff2() {
            ::core::result::Result::Ok(read_value) => read_value,
            ::core::result::Result::Err(read_error) => {
              return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
            }
          }
          .offset_data()
          .as_bytes();
          explicit_value
        },
        0,
        None,
      ),
      "fixture value",
    )?;
    assert!(cff.encoding().is_none());
    ::core::result::Result::Ok(())
  }
}
