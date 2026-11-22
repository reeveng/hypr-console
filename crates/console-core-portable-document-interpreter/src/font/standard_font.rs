use crate::FontResolverFn;
use crate::font::blob::{CffFontBlob, OpenTypeFontBlob};
use crate::font::generated::metrics;
use crate::font::true_type::{Width, read_encoding, read_widths};
use crate::font::{
  Encoding, FontData, FontQuery, glyph_name_to_unicode, normalized_glyph_name,
  retained_font_data, stretch_glyph, strip_subset_prefix,
};
use console_core_font_encodings::{standard, symbol, zapf_dings};
use console_core_font_metrics::GlyphId;
use console_core_font_metrics::raw::TableProvider;
use console_core_glyph_names as glyph_names;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::dict::keys::{
  BASE_FONT, FONT_DESC, FONT_WEIGHT, ITALIC_ANGLE,
};
use kurbo::BezPath;
use std::cell::Cell;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug)]
pub enum StandardFont {
  Helvetica,

  HelveticaBold,

  HelveticaOblique,

  HelveticaBoldOblique,

  Courier,

  CourierBold,

  CourierOblique,

  CourierBoldOblique,

  TimesRoman,

  TimesBold,

  TimesItalic,

  TimesBoldItalic,

  ZapfDingBats,

  Symbol,
}

impl StandardFont {
  pub(crate) fn code_to_name(
    &self,
    code: u8,
  ) -> core::result::Result<Option<&'static str>, console_core_never::Never> {
    Ok(match self {
      Self::Symbol => (match symbol::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten(),

      Self::ZapfDingBats => (match zapf_dings::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten(),
      Self::Helvetica
      | Self::HelveticaBold
      | Self::HelveticaOblique
      | Self::HelveticaBoldOblique
      | Self::Courier
      | Self::CourierBold
      | Self::CourierOblique
      | Self::CourierBoldOblique
      | Self::TimesRoman
      | Self::TimesBold
      | Self::TimesItalic
      | Self::TimesBoldItalic => (match standard::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten(),
    })
  }

  pub(crate) fn get_width(
    &self,
    mut name: &str,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(name == ".notdef", ()) {
      ::core::option::Option::Some(()) => {
        return Ok(Some(250.0));
      }
      ::core::option::Option::None => {}
    }

    name = {
      let Ok(explicit_value) = normalized_glyph_name(name);
      explicit_value
    };

    match self {
      Self::Helvetica => metrics::HELVETICA.get(name),
      Self::HelveticaBold => metrics::HELVETICA_BOLD.get(name),
      Self::HelveticaOblique => metrics::HELVETICA_OBLIQUE.get(name),
      Self::HelveticaBoldOblique => metrics::HELVETICA_BOLD_OBLIQUE.get(name),
      Self::Courier => metrics::COURIER.get(name),
      Self::CourierBold => metrics::COURIER_BOLD.get(name),
      Self::CourierOblique => metrics::COURIER_OBLIQUE.get(name),
      Self::CourierBoldOblique => metrics::COURIER_BOLD_OBLIQUE.get(name),
      Self::TimesRoman => metrics::TIMES_ROMAN.get(name),
      Self::TimesBold => metrics::TIMES_BOLD.get(name),
      Self::TimesItalic => metrics::TIMES_ITALIC.get(name),
      Self::TimesBoldItalic => metrics::TIMES_BOLD_ITALIC.get(name),
      Self::ZapfDingBats => metrics::ZAPF_DING_BATS.get(name),
      Self::Symbol => metrics::SYMBOL.get(name),
    }
  }

  pub(crate) fn as_str(&self) -> core::result::Result<&'static str, console_core_never::Never> {
    Ok(match self {
      Self::Helvetica => "Helvetica",
      Self::HelveticaBold => "Helvetica Bold",
      Self::HelveticaOblique => "Helvetica Oblique",
      Self::HelveticaBoldOblique => "Helvetica Bold Oblique",
      Self::Courier => "Courier",
      Self::CourierBold => "Courier Bold",
      Self::CourierOblique => "Courier Oblique",
      Self::CourierBoldOblique => "Courier Bold Oblique",
      Self::TimesRoman => "Times Roman",
      Self::TimesBold => "Times Bold",
      Self::TimesItalic => "Times Italic",
      Self::TimesBoldItalic => "Times Bold Italic",
      Self::ZapfDingBats => "Zapf Dingbats",
      Self::Symbol => "Symbol",
    })
  }

  pub fn postscript_name(&self) -> core::result::Result<&'static str, console_core_never::Never> {
    Ok(match self {
      Self::Helvetica => "Helvetica",
      Self::HelveticaBold => "Helvetica-Bold",
      Self::HelveticaOblique => "Helvetica-Oblique",
      Self::HelveticaBoldOblique => "Helvetica-BoldOblique",
      Self::Courier => "Courier",
      Self::CourierBold => "Courier-Bold",
      Self::CourierOblique => "Courier-Oblique",
      Self::CourierBoldOblique => "Courier-BoldOblique",
      Self::TimesRoman => "Times-Roman",
      Self::TimesBold => "Times-Bold",
      Self::TimesItalic => "Times-Italic",
      Self::TimesBoldItalic => "Times-BoldItalic",
      Self::ZapfDingBats => "ZapfDingbats",
      Self::Symbol => "Symbol",
    })
  }

  pub(crate) fn is_bold(&self) -> core::result::Result<crate::Weight, console_core_never::Never> {
    Ok(match self {
      Self::HelveticaBold
      | Self::HelveticaBoldOblique
      | Self::CourierBold
      | Self::CourierBoldOblique
      | Self::TimesBold
      | Self::TimesBoldItalic => crate::Weight::Bold,
      Self::Helvetica
      | Self::HelveticaOblique
      | Self::Courier
      | Self::CourierOblique
      | Self::TimesRoman
      | Self::TimesItalic
      | Self::ZapfDingBats
      | Self::Symbol => crate::Weight::Regular,
    })
  }

  pub(crate) fn is_italic(
    &self,
  ) -> core::result::Result<crate::Slant, console_core_never::Never> {
    Ok(match self {
      Self::HelveticaOblique
      | Self::HelveticaBoldOblique
      | Self::CourierOblique
      | Self::CourierBoldOblique
      | Self::TimesItalic
      | Self::TimesBoldItalic => crate::Slant::Italic,
      Self::Helvetica
      | Self::HelveticaBold
      | Self::Courier
      | Self::CourierBold
      | Self::TimesRoman
      | Self::TimesBold
      | Self::ZapfDingBats
      | Self::Symbol => crate::Slant::Upright,
    })
  }

  pub(crate) fn is_serif(
    &self,
  ) -> core::result::Result<crate::Serifs, console_core_never::Never> {
    Ok(match self {
      Self::TimesRoman | Self::TimesBold | Self::TimesItalic | Self::TimesBoldItalic => {
        crate::Serifs::Serif
      }
      Self::Helvetica
      | Self::HelveticaBold
      | Self::HelveticaOblique
      | Self::HelveticaBoldOblique
      | Self::Courier
      | Self::CourierBold
      | Self::CourierOblique
      | Self::CourierBoldOblique
      | Self::ZapfDingBats
      | Self::Symbol => crate::Serifs::SansSerif,
    })
  }

  pub(crate) fn is_monospace(
    &self,
  ) -> core::result::Result<crate::Spacing, console_core_never::Never> {
    Ok(match self {
      Self::Courier | Self::CourierBold | Self::CourierOblique | Self::CourierBoldOblique => {
        crate::Spacing::Monospace
      }
      Self::Helvetica
      | Self::HelveticaBold
      | Self::HelveticaOblique
      | Self::HelveticaBoldOblique
      | Self::TimesRoman
      | Self::TimesBold
      | Self::TimesItalic
      | Self::TimesBoldItalic
      | Self::ZapfDingBats
      | Self::Symbol => crate::Spacing::Proportional,
    })
  }

  #[cfg(feature = "embed-fonts")]
  pub fn get_font_data(
    &self,
  ) -> core::result::Result<(FontData, u32), console_core_never::Never> {
    use crate::font::owned_font_data;

    let data = match self {
      Self::Helvetica => {
        ::core::convert::AsRef::<[_]>::as_ref(&*(include_bytes!("../../assets/FoxitSans.pfb")))
      }
      Self::HelveticaBold => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitSansBold.pfb")),
      ),
      Self::HelveticaOblique => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitSansItalic.pfb")),
      ),
      Self::HelveticaBoldOblique => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitSansBoldItalic.pfb")),
      ),
      Self::Courier => {
        ::core::convert::AsRef::<[_]>::as_ref(&*(include_bytes!("../../assets/FoxitFixed.pfb")))
      }
      Self::CourierBold => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitFixedBold.pfb")),
      ),
      Self::CourierOblique => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitFixedItalic.pfb")),
      ),
      Self::CourierBoldOblique => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitFixedBoldItalic.pfb")),
      ),
      Self::TimesRoman => {
        ::core::convert::AsRef::<[_]>::as_ref(&*(include_bytes!("../../assets/FoxitSerif.pfb")))
      }
      Self::TimesBold => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitSerifBold.pfb")),
      ),
      Self::TimesItalic => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitSerifItalic.pfb")),
      ),
      Self::TimesBoldItalic => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitSerifBoldItalic.pfb")),
      ),
      Self::ZapfDingBats => ::core::convert::AsRef::<[_]>::as_ref(
        &*(include_bytes!("../../assets/FoxitDingbats.pfb")),
      ),
      Self::Symbol => {
        include_bytes!("../../assets/FoxitSymbol.pfb")
      }
    };

    let Ok(backing) = owned_font_data(data);
    Ok((backing, 0))
  }
}

enum StandardFontFamily {
  Helvetica,
  Courier,
  Times,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StandardFontMatch {
  Exact,
  Substitute,
}

pub(crate) fn select_standard_font(
  dict: &Dict<'_>,
  descriptor: &Dict<'_>,
) -> core::result::Result<Option<(StandardFont, StandardFontMatch)>, console_core_never::Never> {
  let Ok(base_font_entry) = dict.get::<Name<'_>>(BASE_FONT);
  let base_font = match base_font_entry {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let Ok(name) = strip_subset_prefix({
    let Ok(explicit_value) = base_font.as_str();
    explicit_value
  });

  match name {
    "Helvetica" => return Ok(Some((StandardFont::Helvetica, StandardFontMatch::Exact))),
    "Helvetica-Bold" => {
      return Ok(Some((
        StandardFont::HelveticaBold,
        StandardFontMatch::Exact,
      )));
    }
    "Helvetica-Oblique" => {
      return Ok(Some((
        StandardFont::HelveticaOblique,
        StandardFontMatch::Exact,
      )));
    }
    "Helvetica-BoldOblique" => {
      return Ok(Some((
        StandardFont::HelveticaBoldOblique,
        StandardFontMatch::Exact,
      )));
    }
    "Courier" => return Ok(Some((StandardFont::Courier, StandardFontMatch::Exact))),
    "Courier-Bold" => return Ok(Some((StandardFont::CourierBold, StandardFontMatch::Exact))),
    "Courier-Oblique" => {
      return Ok(Some((
        StandardFont::CourierOblique,
        StandardFontMatch::Exact,
      )));
    }
    "Courier-BoldOblique" => {
      return Ok(Some((
        StandardFont::CourierBoldOblique,
        StandardFontMatch::Exact,
      )));
    }
    "Times-Roman" => return Ok(Some((StandardFont::TimesRoman, StandardFontMatch::Exact))),
    "Times-Bold" => return Ok(Some((StandardFont::TimesBold, StandardFontMatch::Exact))),
    "Times-Italic" => return Ok(Some((StandardFont::TimesItalic, StandardFontMatch::Exact))),
    "Times-BoldItalic" => {
      return Ok(Some((
        StandardFont::TimesBoldItalic,
        StandardFontMatch::Exact,
      )));
    }
    "Symbol" => return Ok(Some((StandardFont::Symbol, StandardFontMatch::Exact))),
    "ZapfDingbats" => return Ok(Some((StandardFont::ZapfDingBats, StandardFontMatch::Exact))),
    _ => {}
  }

  let lower = name.to_ascii_lowercase();

  let weight = crate::Weight::from(
    ({
      let Ok(explicit_value) = descriptor.get::<u32>(FONT_WEIGHT);
      explicit_value
    })
    .is_some_and(|w| w >= 700)
      || lower.contains("bold")
      || lower.contains("demi"),
  );
  let slant = crate::Slant::from(
    ({
      let Ok(explicit_value) = descriptor.get::<f32>(ITALIC_ANGLE);
      explicit_value
    })
    .is_some_and(|a| {
      !::core::matches!(
        ::core::primitive::f32::classify(a),
        ::core::num::FpCategory::Zero
      )
    }) || lower.contains("italic")
      || lower.contains("oblique"),
  );

  let Ok(matched) = recognize_family(&lower);
  let (family, exact) = match matched {
    Some(match_) => match_,
    None => {
      return Ok(
        (lower.contains("zapfdingbats") || lower.contains("dingbats"))
          .then_some((StandardFont::ZapfDingBats, StandardFontMatch::Substitute)),
      );
    }
  };

  let font = match (family, weight, slant) {
    (StandardFontFamily::Helvetica, crate::Weight::Regular, crate::Slant::Upright) => {
      StandardFont::Helvetica
    }
    (StandardFontFamily::Helvetica, crate::Weight::Bold, crate::Slant::Upright) => {
      StandardFont::HelveticaBold
    }
    (StandardFontFamily::Helvetica, crate::Weight::Regular, crate::Slant::Italic) => {
      StandardFont::HelveticaOblique
    }
    (StandardFontFamily::Helvetica, crate::Weight::Bold, crate::Slant::Italic) => {
      StandardFont::HelveticaBoldOblique
    }
    (StandardFontFamily::Courier, crate::Weight::Regular, crate::Slant::Upright) => {
      StandardFont::Courier
    }
    (StandardFontFamily::Courier, crate::Weight::Bold, crate::Slant::Upright) => {
      StandardFont::CourierBold
    }
    (StandardFontFamily::Courier, crate::Weight::Regular, crate::Slant::Italic) => {
      StandardFont::CourierOblique
    }
    (StandardFontFamily::Courier, crate::Weight::Bold, crate::Slant::Italic) => {
      StandardFont::CourierBoldOblique
    }
    (StandardFontFamily::Times, crate::Weight::Regular, crate::Slant::Upright) => {
      StandardFont::TimesRoman
    }
    (StandardFontFamily::Times, crate::Weight::Bold, crate::Slant::Upright) => {
      StandardFont::TimesBold
    }
    (StandardFontFamily::Times, crate::Weight::Regular, crate::Slant::Italic) => {
      StandardFont::TimesItalic
    }
    (StandardFontFamily::Times, crate::Weight::Bold, crate::Slant::Italic) => {
      StandardFont::TimesBoldItalic
    }
  };

  Ok(Some((font, exact)))
}

fn recognize_family(
  lower: &str,
) -> Result<Option<(StandardFontFamily, StandardFontMatch)>, console_core_never::Never> {
  let families: [(&[&str], StandardFontFamily, StandardFontMatch); 6] = [
    (
      &["helvetica"],
      StandardFontFamily::Helvetica,
      StandardFontMatch::Exact,
    ),
    (
      &["arial", "sans"],
      StandardFontFamily::Helvetica,
      StandardFontMatch::Substitute,
    ),
    (
      &["courier"],
      StandardFontFamily::Courier,
      StandardFontMatch::Exact,
    ),
    (
      &["mono"],
      StandardFontFamily::Courier,
      StandardFontMatch::Substitute,
    ),
    (
      &["times"],
      StandardFontFamily::Times,
      StandardFontMatch::Exact,
    ),
    (
      &["serif"],
      StandardFontFamily::Times,
      StandardFontMatch::Substitute,
    ),
  ];
  let matched = families
    .into_iter()
    .find(|(names, _, _)| names.iter().any(|candidate| lower.contains(candidate)));

  Ok(matched.map(|(_names, family, exact)| (family, exact)))
}

#[derive(Debug)]
pub(crate) enum StandardFontBlob {
  Cff(CffFontBlob),
  Otf(OpenTypeFontBlob, HashMap<String, GlyphId>),
}

impl StandardFontBlob {
  pub(crate) fn from_data(
    data: FontData,
    index: u32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(retained) = retained_font_data(&data);
    let Ok(cff_font_blob) = CffFontBlob::new(retained);

    Ok(match cff_font_blob {
      Some(blob) => {
        let Ok(standard_font_blob) = Self::new_cff(blob);

        Some(standard_font_blob)
      }
      None => {
        let Ok(open_type_font_blob) = OpenTypeFontBlob::new(data, index);

        open_type_font_blob.map(|explicit_argument_0| {
          let Ok(standard_font_blob) = Self::new_otf(explicit_argument_0);

          standard_font_blob
        })
      }
    })
  }

  pub(crate) fn new_cff(
    blob: CffFontBlob,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self::Cff(blob))
  }

  pub(crate) fn new_otf(
    blob: OpenTypeFontBlob,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(glyph_names) = blob.glyph_names();

    Ok(Self::Otf(blob, glyph_names))
  }
}

impl StandardFontBlob {
  pub(crate) fn name_to_glyph(
    &self,
    name: &str,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    Ok(match self {
      Self::Cff(blob) => {
        let Ok(glyph_index_by_name) = blob.glyph_index_by_name(name);

        glyph_index_by_name
      }
      Self::Otf(_, glyph_names) => glyph_names.get(name).copied(),
    })
  }

  pub(crate) fn unicode_to_glyph(
    &self,
    code: u32,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    Ok(match self {
      Self::Cff(_) => None,
      Self::Otf(blob, _) => {
        let Ok(font_ref) = blob.font_ref();

        match font_ref.cmap() {
          Ok(parsed_value) => Some(parsed_value),
          Err(_missing_cmap_table) => None,
        }
        .and_then(|c| c.map_codepoint(code))
      }
    })
  }

  pub(crate) fn advance_width(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(match self {
      Self::Cff(_) => None,
      Self::Otf(blob, _) => {
        let Ok(glyph_metrics) = blob.glyph_metrics();

        glyph_metrics.advance_width(glyph)
      }
    })
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    match ::core::primitive::bool::then_some(glyph == GlyphId::NOTDEF, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(BezPath::new());
      }
      ::core::option::Option::None => {}
    }

    Ok(match self {
      Self::Cff(blob) => {
        let Ok(outline_glyph) = blob.outline_glyph(glyph);

        outline_glyph
      }
      Self::Otf(blob, _) => {
        let Ok(outline_glyph) = blob.outline_glyph(glyph);

        outline_glyph
      }
    })
  }
}

pub(crate) struct StandardKind {
  base_font: StandardFont,
  base_font_blob: StandardFontBlob,
  encoding: Encoding,
  widths: Vec<Width>,
  missing_width: crate::font::FontUnits,
  font_match: StandardFontMatch,
  glyph_to_code: Cell<HashMap<GlyphId, u8>>,
  encodings: HashMap<u8, String>,
}

impl std::fmt::Debug for StandardKind {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("StandardKind")
      .field("base_font", &self.base_font)
      .field("font_match", &self.font_match)
      .finish_non_exhaustive()
  }
}

impl StandardKind {
  pub(crate) fn new(
    dict: &Dict<'_>,
    resolver: &FontResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(descriptor_entry) = dict.get::<Dict<'_>>(FONT_DESC);
    let descriptor = match descriptor_entry {
      Some(present_value) => present_value,
      None => Default::default(),
    };
    let Ok(matched_font) = select_standard_font(dict, &descriptor);
    let (font, font_match) = match matched_font {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(standard_kind) = Self::new_with_standard(dict, font, font_match, resolver);

    Ok(standard_kind)
  }

  pub(crate) fn new_with_standard(
    dict: &Dict<'_>,
    base_font: StandardFont,
    font_match: StandardFontMatch,
    resolver: &FontResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(descriptor_entry) = dict.get::<Dict<'_>>(FONT_DESC);
    let descriptor = match descriptor_entry {
      Some(present_value) => present_value,
      None => Default::default(),
    };
    let Ok(measured_widths) = read_widths(dict, &descriptor);
    let (widths, missing_width) = match measured_widths {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok((mut encoding, encoding_map)) = read_encoding(dict);

    match base_font {
      StandardFont::Symbol | StandardFont::ZapfDingBats => encoding = Encoding::BuiltIn,
      StandardFont::Helvetica
      | StandardFont::HelveticaBold
      | StandardFont::HelveticaOblique
      | StandardFont::HelveticaBoldOblique
      | StandardFont::Courier
      | StandardFont::CourierBold
      | StandardFont::CourierOblique
      | StandardFont::CourierBoldOblique
      | StandardFont::TimesRoman
      | StandardFont::TimesBold
      | StandardFont::TimesItalic
      | StandardFont::TimesBoldItalic => {}
    }

    let (blob, index) = match resolver(&FontQuery::Standard(base_font)) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(standard_font_blob) = StandardFontBlob::from_data(blob, index);
    let base_font_blob = match standard_font_blob {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Self {
      base_font,
      base_font_blob,
      widths,
      missing_width,
      encodings: encoding_map,
      glyph_to_code: Cell::new(HashMap::new()),
      font_match,
      encoding,
    }))
  }

  fn code_to_ps_name(
    &self,
    code: u8,
  ) -> core::result::Result<Option<&str>, console_core_never::Never> {
    let bf = self.base_font;

    Ok(
      self
        .encodings
        .get(&code)
        .map(String::as_str)
        .or_else(|| match self.encoding {
          Encoding::BuiltIn => {
            let Ok(code_to_name) = bf.code_to_name(code);

            code_to_name
          }
          Encoding::Standard | Encoding::MacRoman | Encoding::WinAnsi | Encoding::MacExpert => {
            let Ok(map_code) = self.encoding.map_code(code);

            map_code
          }
        }),
    )
  }

  pub(crate) fn map_code(
    &self,
    code: u8,
  ) -> core::result::Result<GlyphId, console_core_never::Never> {
    let Ok(code_to_ps_name) = self.code_to_ps_name(code);
    let result = match code_to_ps_name.and_then(|c| {
      let Ok(name_to_glyph) = self.base_font_blob.name_to_glyph(c);

      name_to_glyph.or_else(|| {
        let Ok(name) = glyph_names::get(c);
        let first = name.and_then(|text| text.chars().next())?;
        let Ok(glyph) = self.base_font_blob.unicode_to_glyph(u32::from(first));
        glyph
      })
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => GlyphId::NOTDEF,
    };
    let mut codes = self.glyph_to_code.take();
    codes.insert(result, code);
    self.glyph_to_code.set(codes);

    Ok(result)
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(path) = self.base_font_blob.outline_glyph(glyph);

    let code = {
      let codes = self.glyph_to_code.take();
      let optional_code = codes.get(&glyph).copied();
      self.glyph_to_code.set(codes);

      match optional_code {
        Some(matched_code) => matched_code,
        ::core::option::Option::None => return Ok(path),
      }
    };

    let Ok(advance_width) = self.base_font_blob.advance_width(glyph);
    let actual_width = advance_width.or_else(|| {
      let Ok(name) = self.code_to_ps_name(code);
      let glyph_name = name?;
      let Ok(width) = self.base_font.get_width(glyph_name);
      width
    });

    let actual_width_2 = match actual_width {
      Some(matched_actual_width) => matched_actual_width,
      ::core::option::Option::None => return Ok(path),
    };

    let should_width = match self.font_match {
      StandardFontMatch::Substitute => {
        let Ok(code_2) = console_core_number_conversion::wrapped_index(code);

        match self.widths.get(code_2) {
          Some(Width::Value(width)) => *width,
          Some(Width::Missing) | None => return Ok(path),
        }
      }
      StandardFontMatch::Exact => {
        let Ok(name) = self.code_to_ps_name(code);
        let width = name.and_then(|name_2| {
          let Ok(width) = self.base_font.get_width(name_2);
          width
        });

        match width {
          Some(width_2) => width_2,
          None => return Ok(path),
        }
      }
    };

    stretch_glyph(
      path,
      crate::font::GlyphWidths {
        expected: crate::font::FontUnits(should_width),
        actual: crate::font::FontUnits(actual_width_2),
      },
    )
  }

  pub(crate) fn glyph_width(
    &self,
    code: u8,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(
      match self
        .widths
        .get({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(code);
          numeric_value
        })
        .copied()
      {
        Some(Width::Value(w)) => Some(w),
        Some(Width::Missing) => Some(self.missing_width.0),
        None => {
          let Ok(code_to_ps_name) = self.code_to_ps_name(code);

          code_to_ps_name.and_then(|c| {
            let Ok(width) = self.base_font.get_width(c);

            width
          })
        }
      },
    )
  }

  pub(crate) fn char_code_to_unicode(
    &self,
    code: u8,
  ) -> core::result::Result<Option<char>, console_core_never::Never> {
    let Ok(code_to_ps_name) = self.code_to_ps_name(code);

    Ok(code_to_ps_name.and_then(|explicit_argument_0| {
      let Ok(explicit_value) = glyph_name_to_unicode(explicit_argument_0);
      explicit_value
    }))
  }

  pub(crate) fn is_italic(
    &self,
  ) -> core::result::Result<crate::Slant, console_core_never::Never> {
    self.base_font.is_italic()
  }

  pub(crate) fn is_bold(&self) -> core::result::Result<crate::Weight, console_core_never::Never> {
    self.base_font.is_bold()
  }

  pub(crate) fn is_serif(
    &self,
  ) -> core::result::Result<crate::Serifs, console_core_never::Never> {
    self.base_font.is_serif()
  }

  pub(crate) fn is_monospace(
    &self,
  ) -> core::result::Result<crate::Spacing, console_core_never::Never> {
    self.base_font.is_monospace()
  }
}
