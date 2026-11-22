//! The fonts and tables the font reader, metrics and hinting crates are tested against.
//!
//! This is fontations' `font-test-data` 0.7.0, carried because all three crates read the same
//! fixtures and a copy in each would drift. A whole font is a static of its bytes, included from
//! `test_data/`; a table transcribed from the specification's examples is a static array in the
//! module named for its table; and a table a test needs to vary is written with
//! [`be_buffer!`] in the order its fields are laid down. `colrv1_json::expected` is what each
//! `COLR` test font is expected to draw, looked up by the font's name, and a name it does not know
//! is a fault rather than an empty drawing.

pub mod bebuffer;
pub mod cmap;
pub mod gdef;
pub mod gpos;
pub mod gsub;
pub mod ift;
pub mod layout;
pub mod type1;

pub static CMAP12_FONT1: &[u8] = include_bytes!("../test_data/ttf/cmap12_font1.ttf");

pub static CMAP14_FONT1: &[u8] = include_bytes!("../test_data/ttf/cmap14_font1.ttf");

pub static CMAP4_SYMBOL_PUA: &[u8] = include_bytes!("../test_data/ttf/cmap4_symbol_pua.ttf");

pub static CMAP6: &[u8] = include_bytes!("../test_data/ttf/cmap6.ttf");

pub static CMAP10: &[u8] = include_bytes!("../test_data/ttf/cmap10.ttf");

pub static COLR_GRADIENT_RECT: &[u8] =
  include_bytes!("../test_data/ttf/linear_gradient_rect_colr_1.ttf");

pub static VAZIRMATN_VAR: &[u8] = include_bytes!("../test_data/ttf/vazirmatn_var_trimmed.ttf");

pub static NAMES_ONLY: &[u8] = include_bytes!("../test_data/ttf/names_only.ttf");

pub static VAZIRMATN_VAR_GLYPHS: &str =
  include_str!("../test_data/extracted/vazirmatn_var_trimmed-glyphs.txt");

pub static SIMPLE_GLYF: &[u8] = include_bytes!("../test_data/ttf/simple_glyf.ttf");

pub static CUBIC_GLYF: &[u8] = include_bytes!("../test_data/ttf/cubic_glyf.ttf");

pub static NOTO_SERIF_DISPLAY_TRIMMED: &[u8] =
  include_bytes!("../test_data/ttf/noto_serif_display_trimmed.ttf");

pub static NOTO_SERIF_DISPLAY_TRIMMED_GLYPHS: &str =
  include_str!("../test_data/extracted/noto_serif_display_trimmed-glyphs.txt");

pub static NOTO_SANS_JP_CFF: &[u8] =
  include_bytes!("../test_data/ttf/NotoSansJP-Regular.subset.otf");

pub static CANTARELL_VF_TRIMMED: &[u8] =
  include_bytes!("../test_data/ttf/cantarell_vf_trimmed.ttf");

pub static CANTARELL_VF_TRIMMED_GLYPHS: &str =
  include_str!("../test_data/extracted/cantarell_vf_trimmed-glyphs.txt");

pub static CHARSTRING_PATH_OPS: &[u8] =
  include_bytes!("../test_data/ttf/charstring_path_ops.ttf");

pub static EMBEDDED_BITMAPS: &[u8] = include_bytes!("../test_data/ttf/embedded_bitmaps.ttf");
pub static CBDT: &[u8] = include_bytes!("../test_data/ttf/cbdt.ttf");

pub static HVAR_WITH_TRUNCATED_ADVANCE_INDEX_MAP: &[u8] =
  include_bytes!("../test_data/ttf/hvar_with_truncated_adv_index_map.ttf");

pub static COLRV0V1: &[u8] = include_bytes!("../test_data/ttf/test_glyphs-glyf_colr_1.ttf");

pub static COLRV0V1_VARIABLE: &[u8] =
  include_bytes!("../test_data/ttf/test_glyphs-glyf_colr_1_variable.ttf");

pub static COLRV1_NO_CLIPLIST: &[u8] =
  include_bytes!("../test_data/ttf/test_glyphs-glyf_colr_1_no_cliplist.subset.ttf");

pub static CVAR: &[u8] = include_bytes!("../test_data/ttf/cvar.ttf");

pub static STARTING_OFF_CURVE: &[u8] = include_bytes!("../test_data/ttf/starts_off_curve.ttf");

pub static MOSTLY_OFF_CURVE: &[u8] = include_bytes!("../test_data/ttf/mostly_off_curve.ttf");

pub static INTERPOLATE_THIS: &[u8] = include_bytes!("../test_data/ttf/interpolate_this.ttf");

pub static MATERIAL_SYMBOLS_SUBSET: &[u8] =
  include_bytes!("../test_data/ttf/material_symbols_subset.ttf");

pub static GLYF_COMPONENTS: &[u8] = include_bytes!("../test_data/ttf/glyf_components.ttf");

pub static AUTOHINT_CMAP: &[u8] = include_bytes!("../test_data/ttf/autohint_cmap.ttf");

pub static NOTOSERIFHEBREW_AUTOHINT_METRICS: &[u8] =
  include_bytes!("../test_data/ttf/notoserifhebrew_autohint_metrics.ttf");

pub static NOTOSERIFTC_AUTOHINT_METRICS: &[u8] =
  include_bytes!("../test_data/ttf/notoseriftc_autohint_metrics.ttf");

pub static NOTOSERIF_AUTOHINT_SHAPING: &[u8] =
  include_bytes!("../test_data/ttf/notoserif_autohint_shaping.ttf");

pub static TTHINT_SUBSET: &[u8] = include_bytes!("../test_data/ttf/tthint_subset.ttf");

pub static VORG: &[u8] = include_bytes!("../test_data/ttf/vorg.ttf");

pub static AHEM: &[u8] = include_bytes!("../test_data/ttf/ahem.ttf");

pub static AVAR2_CHECKER: &[u8] = include_bytes!("../test_data/ttf/avar2checker.ttf");
pub static AMSTELVAR_AVAR2_A: &[u8] = include_bytes!("../test_data/ttf/Amstelvar-avar2.A.ttf");

pub static MATERIAL_ICONS_SUBSET: &[u8] =
  include_bytes!("../test_data/ttf/material_icons_subset.ttf");

pub static MATERIAL_ICONS_SUBSET_MATRIX: &[u8] =
  include_bytes!("../test_data/ttf/material_icons_subset_matrix.ttf");

pub static TINOS_SUBSET: &[u8] = include_bytes!("../test_data/ttf/tinos_subset.ttf");

pub static NOTO_HANDWRITING_SBIX: &[u8] =
  include_bytes!("../test_data/ttf/noto_handwriting-sbix.ttf");

pub static COUSINE_HINT_SUBSET: &[u8] =
  include_bytes!("../test_data/ttf/cousine_hint_subset.ttf");

pub mod morx {

  pub static ONE: &[u8] = include_bytes!("../test_data/ttf/TestMORXOne.ttf");
  pub static FOUR: &[u8] = include_bytes!("../test_data/ttf/TestMORXFour.ttf");
  pub static EIGHTEEN: &[u8] = include_bytes!("../test_data/ttf/TestMORXEighteen.ttf");
  pub static THIRTY_FOUR: &[u8] = include_bytes!("../test_data/ttf/TestMORXThirtyfour.ttf");
  pub static FORTY_ONE: &[u8] = include_bytes!("../test_data/ttf/TestMORXFourtyone.ttf");
}

pub mod varc {
  pub static CJK_6868: &[u8] = include_bytes!("../test_data/ttf/varc-6868.ttf");
  pub static CONDITIONALS: &[u8] = include_bytes!("../test_data/ttf/varc-ac01-conditional.ttf");
}

pub mod closure {
  pub static SIMPLE: &[u8] = include_bytes!("../test_data/ttf/simple_closure.ttf");
  pub static SIMPLE_GLYPHS: &str = include_str!("../test_data/fea/simple_closure_glyphs.txt");
  pub static RECURSIVE: &[u8] = include_bytes!("../test_data/ttf/recursive_closure.ttf");
  pub static RECURSIVE_GLYPHS: &str =
    include_str!("../test_data/fea/recursive_closure_glyphs.txt");
  pub static CONTEXTUAL: &[u8] = include_bytes!("../test_data/ttf/context_closure.ttf");
  pub static CONTEXTUAL_GLYPHS: &str =
    include_str!("../test_data/fea/context_closure_glyphs.txt");
  pub static RECURSIVE_CONTEXTUAL: &[u8] =
    include_bytes!("../test_data/ttf/recursive_context_closure.ttf");
  pub static RECURSIVE_CONTEXTUAL_GLYPHS: &str =
    include_str!("../test_data/fea/recursive_context_closure_glyphs.txt");

  pub static CYCLIC_CONTEXTUAL: &[u8] =
    include_bytes!("../test_data/ttf/cyclic_context_closure.ttf");
  pub static CHAIN_CONTEXT_FORMAT3_BITS: &[u8] =
    include_bytes!("../test_data/ttf/context_closure_unreachable_rule.ttf");
  pub static CHAIN_CONTEXT_FORMAT3_BITS_GLYPHS: &str =
    include_str!("../test_data/fea/context_closure_unreachable_rule_glyphs.txt");
  pub static VARIATIONS_CLOSURE: &[u8] =
    include_bytes!("../test_data/ttf/variations_closure.ttf");
  pub static VARIATIONS_GLYPHS: &str =
    include_str!("../test_data/fea/variations_closure_glyphs.txt");
  pub static CONTEXT_ONLY_REACHABLE: &[u8] =
    include_bytes!("../test_data/ttf/context_only_reachable.ttf");
  pub static CONTEXT_ONLY_REACHABLE_GLYPHS: &str =
    include_str!("../test_data/fea/context_only_reachable_glyphs.txt");
}

pub mod post {

  #[rustfmt::skip]
    pub static SIMPLE: &[u8] = &[
        0x00, 0x02, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0xFF, 0xb5,
        0x00, 0x32,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x0A,

        0x00, 0x00,
        0x00, 0x00,
        0x00, 0x03,
        0x00, 0x04,
        0x00, 0x06,
        0x00, 0x07,
        0x00, 0x08,
        0x01, 0x02,
        0x01, 0x03,
        0x01, 0x04,
        0x05, 0x68, 0x65, 0x6c, 0x6c, 0x6f,
        0x02, 0x68, 0x69,
        0x4, 0x68, 0x6f, 0x6c, 0x61,
    ];
}

pub mod cff2 {

  pub static EXAMPLE: &[u8] = &[
    0x02, 0x00, 0x05, 0x00, 0x07, 0xCF, 0x0C, 0x24, 0xC3, 0x11, 0x9B, 0x18, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x26, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0C, 0x00, 0x01, 0x00, 0x00, 0x00, 0x1C,
    0x00, 0x01, 0x00, 0x02, 0xC0, 0x00, 0xE0, 0x00, 0x00, 0x00, 0xC0, 0x00, 0xC0, 0x00, 0xE0,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
    0x01, 0x01, 0x03, 0x05, 0x20, 0x0A, 0x20, 0x0A, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x05,
    0xF7, 0x06, 0xDA, 0x12, 0x77, 0x9F, 0xF8, 0x6C, 0x9D, 0xAE, 0x9A, 0xF4, 0x9A, 0x95, 0x9F,
    0xB3, 0x9F, 0x8B, 0x8B, 0x8B, 0x8B, 0x85, 0x9A, 0x8B, 0x8B, 0x97, 0x73, 0x8B, 0x8B, 0x8C,
    0x80, 0x8B, 0x8B, 0x8B, 0x8D, 0x8B, 0x8B, 0x8C, 0x8A, 0x8B, 0x8B, 0x97, 0x17, 0x06, 0xFB,
    0x8E, 0x95, 0x86, 0x9D, 0x8B, 0x8B, 0x8D, 0x17, 0x07, 0x77, 0x9F, 0xF8, 0x6D, 0x9D, 0xAD,
    0x9A, 0xF3, 0x9A, 0x95, 0x9F, 0xB3, 0x9F, 0x08, 0xFB, 0x8D, 0x95, 0x09, 0x1E, 0xA0, 0x37,
    0x5F, 0x0C, 0x09, 0x8B, 0x0C, 0x0B, 0xC2, 0x6E, 0x9E, 0x8C, 0x17, 0x0A, 0xDB, 0x57, 0xF7,
    0x02, 0x8C, 0x17, 0x0B, 0xB3, 0x9A, 0x77, 0x9F, 0x82, 0x8A, 0x8D, 0x17, 0x0C, 0x0C, 0xDB,
    0x95, 0x57, 0xF7, 0x02, 0x85, 0x8B, 0x8D, 0x17, 0x0C, 0x0D, 0xF7, 0x06, 0x13, 0x00, 0x00,
    0x00, 0x01, 0x01, 0x01, 0x1B, 0xBD, 0xBD, 0xEF, 0x8C, 0x10, 0x8B, 0x15, 0xF8, 0x88, 0x27,
    0xFB, 0x5C, 0x8C, 0x10, 0x06, 0xF8, 0x88, 0x07, 0xFC, 0x88, 0xEF, 0xF7, 0x5C, 0x8C, 0x10,
    0x06,
  ];
}

pub mod colrv1_json {

  #[derive(Debug)]
  pub struct MissingExpectedData {
    pub key: String,
  }

  impl std::fmt::Display for MissingExpectedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "No expected fixture for {}", self.key)
    }
  }

  impl std::error::Error for MissingExpectedData {}

  pub fn expected(
    set_name: &str,
    settings: &[(&str, f32)],
  ) -> Result<&'static str, MissingExpectedData> {
    let mut key_parts = Vec::with_capacity(settings.len().saturating_add(1));
    key_parts.push("colrv1_".to_string() + &set_name.to_ascii_lowercase());
    key_parts.extend(settings.iter().map(|(t, v)| format!("{t}_{v}")));
    let key = key_parts.join("_");

    Ok(match key.as_str() {
      "colrv1_clipbox" => include_str!("../test_data/colrv1_json/colrv1_clipbox"),
      "colrv1_clipbox_CLIO_200" => {
        include_str!("../test_data/colrv1_json/colrv1_clipbox_CLIO_200")
      }
      "colrv1_colored_circles_v0" => {
        include_str!("../test_data/colrv1_json/colrv1_colored_circles_v0")
      }
      "colrv1_composite_mode" => {
        include_str!("../test_data/colrv1_json/colrv1_composite_mode")
      }
      "colrv1_extend_mode" => include_str!("../test_data/colrv1_json/colrv1_extend_mode"),
      "colrv1_extend_mode_COL1_-0.25_COL3_0.25" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL1_-0.25_COL3_0.25")
      }
      "colrv1_extend_mode_COL1_0.5_COL3_-0.5" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL1_0.5_COL3_-0.5")
      }
      "colrv1_extend_mode_COL1_-1.5" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL1_-1.5")
      }
      "colrv1_extend_mode_COL2_-0.3" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL2_-0.3")
      }
      "colrv1_extend_mode_COL3_0.5" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL3_0.5")
      }
      "colrv1_extend_mode_COL3_1" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL3_1")
      }
      "colrv1_extend_mode_COL3_1_COL2_1.5_COL1_2" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_COL3_1_COL2_1.5_COL1_2")
      }
      "colrv1_extend_mode_GRR0_-200_GRR1_-300" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_GRR0_-200_GRR1_-300")
      }
      "colrv1_extend_mode_GRR0_430_GRR1_40" => {
        include_str!("../test_data/colrv1_json/colrv1_extend_mode_GRR0_430_GRR1_40")
      }
      "colrv1_extend_mode_GRR0_-50_COL3_-2_COL2_-2_COL1_-0.9" => include_str!(
        "../test_data/colrv1_json/colrv1_extend_mode_GRR0_-50_COL3_-2_COL2_-2_COL1_-0.9"
      ),
      "colrv1_extend_mode_GRR0_-50_COL3_-2_COL2_-2_COL1_-1.1" => include_str!(
        "../test_data/colrv1_json/colrv1_extend_mode_GRR0_-50_COL3_-2_COL2_-2_COL1_-1.1"
      ),
      "colrv1_extend_mode_GRX0_1000_GRX1_-1000_GRR0_-1000_GRR1_200" => include_str!(
        "../test_data/colrv1_json/colrv1_extend_mode_GRX0_1000_GRX1_-1000_GRR0_-1000_GRR1_200"
      ),
      "colrv1_extend_mode_GRX0_-1000_GRX1_-1000_GRR0_-1000_GRR1_-900" => include_str!(
        "../test_data/colrv1_json/colrv1_extend_mode_GRX0_-1000_GRX1_-1000_GRR0_-1000_GRR1_-900"
      ),
      "colrv1_foreground_color" => {
        include_str!("../test_data/colrv1_json/colrv1_foreground_color")
      }
      "colrv1_gradient_p2_skewed" => {
        include_str!("../test_data/colrv1_json/colrv1_gradient_p2_skewed")
      }
      "colrv1_gradient_stops_repeat" => {
        include_str!("../test_data/colrv1_json/colrv1_gradient_stops_repeat")
      }
      "colrv1_no_cycle_multi_colrglyph" => {
        include_str!("../test_data/colrv1_json/colrv1_no_cycle_multi_colrglyph")
      }
      "colrv1_paint_glyph_nested" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_glyph_nested")
      }
      "colrv1_paint_rotate" => include_str!("../test_data/colrv1_json/colrv1_paint_rotate"),
      "colrv1_paint_rotate_ROTA_40" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_rotate_ROTA_40")
      }
      "colrv1_paint_rotate_ROTX_-250_ROTY_-250" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_rotate_ROTX_-250_ROTY_-250")
      }
      "colrv1_paint_scale" => include_str!("../test_data/colrv1_json/colrv1_paint_scale"),
      "colrv1_paint_scale_SCOX_200_SCOY_200" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_scale_SCOX_200_SCOY_200")
      }
      "colrv1_paint_scale_SCSX_0.25_SCOY_0.25" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_scale_SCSX_0.25_SCOY_0.25")
      }
      "colrv1_paint_scale_SCSX_-1_SCOY_-1" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_scale_SCSX_-1_SCOY_-1")
      }
      "colrv1_paint_skew" => include_str!("../test_data/colrv1_json/colrv1_paint_skew"),
      "colrv1_paint_skew_SKCX_200_SKCY_200" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_skew_SKCX_200_SKCY_200")
      }
      "colrv1_paint_skew_SKXA_20" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_skew_SKXA_20")
      }
      "colrv1_paint_skew_SKYA_20" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_skew_SKYA_20")
      }
      "colrv1_paint_transform" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_transform")
      }
      "colrv1_paint_translate" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_translate")
      }
      "colrv1_paint_translate_TLDX_100_TLDY_100" => {
        include_str!("../test_data/colrv1_json/colrv1_paint_translate_TLDX_100_TLDY_100")
      }
      "colrv1_sweep_coincident" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_coincident")
      }
      "colrv1_sweep_varsweep" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_varsweep")
      }
      "colrv1_sweep_varsweep_SWC1_-0.25_SWC2_0.083333336_SWC3_0.083333336_SWC4_0.25" => {
        include_str!(
          "../test_data/colrv1_json/colrv1_sweep_varsweep_SWC1_-0.25_SWC2_0.083333336_SWC3_0.083333336_SWC4_0.25"
        )
      }
      "colrv1_sweep_varsweep_SWPE_-45" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_varsweep_SWPE_-45")
      }
      "colrv1_sweep_varsweep_SWPE_-90" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_varsweep_SWPE_-90")
      }
      "colrv1_sweep_varsweep_SWPS_0" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_varsweep_SWPS_0")
      }
      "colrv1_sweep_varsweep_SWPS_-45_SWPE_45" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_varsweep_SWPS_-45_SWPE_45")
      }
      "colrv1_sweep_varsweep_SWPS_45_SWPE_-45_SWC1_-0.25_SWC2_-0.416687_SWC3_-0.583313_SWC4_-0.75" =>
      {
        include_str!(
          "../test_data/colrv1_json/colrv1_sweep_varsweep_SWPS_45_SWPE_-45_SWC1_-0.25_SWC2_-0.416687_SWC3_-0.583313_SWC4_-0.75"
        )
      }
      "colrv1_sweep_varsweep_SWPS_90" => {
        include_str!("../test_data/colrv1_json/colrv1_sweep_varsweep_SWPS_90")
      }
      "colrv1_variable_alpha" => {
        include_str!("../test_data/colrv1_json/colrv1_variable_alpha")
      }
      "colrv1_variable_alpha_APH1_-0.7" => {
        include_str!("../test_data/colrv1_json/colrv1_variable_alpha_APH1_-0.7")
      }
      "colrv1_variable_alpha_APH2_-0.7_APH3_-0.2" => {
        include_str!("../test_data/colrv1_json/colrv1_variable_alpha_APH2_-0.7_APH3_-0.2")
      }
      _ => return Err(MissingExpectedData { key }),
    })
  }
}

pub mod ttc {
  pub static TTC: &[u8] = include_bytes!("../test_data/ttc/TTC.ttc");
}

pub mod meta {

  #[rustfmt::skip]
    pub static SIMPLE_META_TABLE: &[u8] = &[
        0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x28,
        0x00, 0x00, 0x00, 0x02,
        0x64, 0x6c, 0x6e, 0x67,
        0x00, 0x00, 0x00, 0x28,
        0x00, 0x00, 0x00, 0x0d,
        0x73, 0x6c, 0x6e, 0x67,
        0x00, 0x00, 0x00, 0x35,
        0x00, 0x00, 0x00, 0x04,
        0x65, 0x6e, 0x2d, 0x6c,
        0x61, 0x74, 0x6e, 0x2c,
        0x20, 0x6c, 0x61, 0x74,
        0x6e, 0x6c, 0x61, 0x74,
        0x6e, 0x00, 0x00, 0x00,
    ];
}

pub mod gvar {
  pub static USE_MY_METRICS: &[u8] = include_bytes!("../test_data/ttf/gvar_use_my_metrics.ttf");
}
