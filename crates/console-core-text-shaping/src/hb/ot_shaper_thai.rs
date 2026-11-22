use super::buffer::*;
use super::font_funcs::FontFuncsDispatch;
use super::ot_layout::*;
use super::ot_shape_normalize::HB_OT_SHAPE_NORMALIZATION_MODE_AUTO;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper::*;
use super::script;
use super::unicode::GeneralCategory;

pub const THAI_SHAPER: hb_ot_shaper_t = hb_ot_shaper_t {
  collect_features: None,
  override_features: None,
  create_data: None,
  preprocess_text: Some(preprocess_text),
  postprocess_glyphs: None,
  normalization_preference: HB_OT_SHAPE_NORMALIZATION_MODE_AUTO,
  decompose: None,
  compose: None,
  setup_masks: None,
  gpos_tag: None,
  reorder_marks: None,
  zero_width_marks: HB_OT_SHAPE_ZERO_WIDTH_MARKS_BY_GDEF_LATE,
  fallback_position: false,
};

#[derive(Clone, Copy, PartialEq)]
enum Consonant {
  NC = 0,
  AC,
  RC,
  DC,
  NotConsonant,
}

fn get_consonant_type(u: u32) -> Consonant {
  match u {
    0x0E1B | 0x0E1D | 0x0E1F => Consonant::AC,
    0x0E0D | 0x0E10 => Consonant::RC,
    0x0E0E | 0x0E0F => Consonant::DC,
    0x0E01..=0x0E2E => Consonant::NC,
    _ => Consonant::NotConsonant,
  }
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
  AV,
  BV,
  T,
  NotMark,
}

fn get_mark_type(u: u32) -> Mark {
  match u {
    0x0E31 | 0x0E34..=0x0E37 | 0x0E47 | 0x0E4D..=0x0E4E => Mark::AV,
    0x0E38..=0x0E3A => Mark::BV,
    0x0E48..=0x0E4C => Mark::T,
    _ => Mark::NotMark,
  }
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
  NOP,
  /// Shift combining-mark down.
  SD,
  /// Shift combining-mark left.
  SL,
  /// Shift combining-mark down-left.
  SDL,
  /// Remove descender from base.
  RD,
}

#[derive(Clone, Copy)]
struct PuaMapping {
  u: u16,
  win_pua: u16,
  mac_pua: u16,
}

impl PuaMapping {
  const fn new(u: u16, win_pua: u16, mac_pua: u16) -> Self {
    PuaMapping {
      u,
      win_pua,
      mac_pua,
    }
  }
}

static SD_MAPPINGS: &[PuaMapping] = &[
  PuaMapping::new(0x0E48, 0xF70A, 0xF88B),
  PuaMapping::new(0x0E49, 0xF70B, 0xF88E),
  PuaMapping::new(0x0E4A, 0xF70C, 0xF891),
  PuaMapping::new(0x0E4B, 0xF70D, 0xF894),
  PuaMapping::new(0x0E4C, 0xF70E, 0xF897),
  PuaMapping::new(0x0E38, 0xF718, 0xF89B),
  PuaMapping::new(0x0E39, 0xF719, 0xF89C),
  PuaMapping::new(0x0E3A, 0xF71A, 0xF89D),
  PuaMapping::new(0x0000, 0x0000, 0x0000),
];

static SDL_MAPPINGS: &[PuaMapping] = &[
  PuaMapping::new(0x0E48, 0xF705, 0xF88C),
  PuaMapping::new(0x0E49, 0xF706, 0xF88F),
  PuaMapping::new(0x0E4A, 0xF707, 0xF892),
  PuaMapping::new(0x0E4B, 0xF708, 0xF895),
  PuaMapping::new(0x0E4C, 0xF709, 0xF898),
  PuaMapping::new(0x0000, 0x0000, 0x0000),
];

static SL_MAPPINGS: &[PuaMapping] = &[
  PuaMapping::new(0x0E48, 0xF713, 0xF88A),
  PuaMapping::new(0x0E49, 0xF714, 0xF88D),
  PuaMapping::new(0x0E4A, 0xF715, 0xF890),
  PuaMapping::new(0x0E4B, 0xF716, 0xF893),
  PuaMapping::new(0x0E4C, 0xF717, 0xF896),
  PuaMapping::new(0x0E31, 0xF710, 0xF884),
  PuaMapping::new(0x0E34, 0xF701, 0xF885),
  PuaMapping::new(0x0E35, 0xF702, 0xF886),
  PuaMapping::new(0x0E36, 0xF703, 0xF887),
  PuaMapping::new(0x0E37, 0xF704, 0xF888),
  PuaMapping::new(0x0E47, 0xF712, 0xF889),
  PuaMapping::new(0x0E4D, 0xF711, 0xF899),
  PuaMapping::new(0x0000, 0x0000, 0x0000),
];

static RD_MAPPINGS: &[PuaMapping] = &[
  PuaMapping::new(0x0E0D, 0xF70F, 0xF89A),
  PuaMapping::new(0x0E10, 0xF700, 0xF89E),
  PuaMapping::new(0x0000, 0x0000, 0x0000),
];

fn pua_shape(u: u32, action: Action, face: &mut FontFuncsDispatch) -> u32 {
  let mappings = match action {
    Action::NOP => return u,
    Action::SD => SD_MAPPINGS,
    Action::SL => SL_MAPPINGS,
    Action::SDL => SDL_MAPPINGS,
    Action::RD => RD_MAPPINGS,
  };

  for m in mappings {
    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(m.u);
      migrated_value
    }) == u
    {
      true => {
        match face
          .nominal_glyph(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(m.win_pua);
              migrated_value
            }),
          )
          .is_some()
        {
          true => {
            return ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(m.win_pua);
              migrated_value
            });
          }
          false => {}
        }

        match face
          .nominal_glyph(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(m.mac_pua);
              migrated_value
            }),
          )
          .is_some()
        {
          true => {
            return ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(m.mac_pua);
              migrated_value
            });
          }
          false => {}
        }

        break;
      }
      false => {}
    }
  }

  u
}

#[derive(Clone, Copy)]
enum AboveState {
  T0,
  T1,
  T2,
  T3,
}

static ABOVE_START_STATE: &[AboveState] = &[
  AboveState::T0,
  AboveState::T1,
  AboveState::T0,
  AboveState::T0,
  AboveState::T3,
];

#[derive(Clone, Copy)]
struct AboveStateMachineEdge {
  action: Action,
  next_state: AboveState,
}

impl AboveStateMachineEdge {
  const fn new(action: Action, next_state: AboveState) -> Self {
    AboveStateMachineEdge { action, next_state }
  }
}

type ASME = AboveStateMachineEdge;

static ABOVE_STATE_MACHINE: &[[ASME; 3]] = &[
  [
    ASME::new(Action::NOP, AboveState::T3),
    ASME::new(Action::NOP, AboveState::T0),
    ASME::new(Action::SD, AboveState::T3),
  ],
  [
    ASME::new(Action::SL, AboveState::T2),
    ASME::new(Action::NOP, AboveState::T1),
    ASME::new(Action::SDL, AboveState::T2),
  ],
  [
    ASME::new(Action::NOP, AboveState::T3),
    ASME::new(Action::NOP, AboveState::T2),
    ASME::new(Action::SL, AboveState::T3),
  ],
  [
    ASME::new(Action::NOP, AboveState::T3),
    ASME::new(Action::NOP, AboveState::T3),
    ASME::new(Action::NOP, AboveState::T3),
  ],
];

#[derive(Clone, Copy)]
enum BelowState {
  /// No descender.
  B0,
  /// Removable descender.
  B1,
  /// Strict descender.
  B2,
}

static BELOW_START_STATE: &[BelowState] = &[
  BelowState::B0,
  BelowState::B0,
  BelowState::B1,
  BelowState::B2,
  BelowState::B2,
];

#[derive(Clone, Copy)]
struct BelowStateMachineEdge {
  action: Action,
  next_state: BelowState,
}

impl BelowStateMachineEdge {
  const fn new(action: Action, next_state: BelowState) -> Self {
    BelowStateMachineEdge { action, next_state }
  }
}

type BSME = BelowStateMachineEdge;

static BELOW_STATE_MACHINE: &[[BSME; 3]] = &[
  [
    BSME::new(Action::NOP, BelowState::B0),
    BSME::new(Action::NOP, BelowState::B2),
    BSME::new(Action::NOP, BelowState::B0),
  ],
  [
    BSME::new(Action::NOP, BelowState::B1),
    BSME::new(Action::RD, BelowState::B2),
    BSME::new(Action::NOP, BelowState::B1),
  ],
  [
    BSME::new(Action::NOP, BelowState::B2),
    BSME::new(Action::SD, BelowState::B2),
    BSME::new(Action::NOP, BelowState::B2),
  ],
];

fn do_pua_shaping(face: &mut FontFuncsDispatch, buffer: &mut hb_buffer_t) {
  let mut above_state = ABOVE_START_STATE[Consonant::NotConsonant as usize];
  let mut below_state = BELOW_START_STATE[Consonant::NotConsonant as usize];
  let mut base = 0;

  for i in 0..buffer.len {
    let mt = get_mark_type(buffer.info[i].glyph_id);

    match mt == Mark::NotMark {
      true => {
        let ct = get_consonant_type(buffer.info[i].glyph_id);
        above_state = ABOVE_START_STATE[ct as usize];
        below_state = BELOW_START_STATE[ct as usize];
        base = i;
        continue;
      }
      false => {}
    }

    let above_edge = ABOVE_STATE_MACHINE[above_state as usize][mt as usize];
    let below_edge = BELOW_STATE_MACHINE[below_state as usize][mt as usize];
    above_state = above_edge.next_state;
    below_state = below_edge.next_state;

    let action = match above_edge.action != Action::NOP {
      true => above_edge.action,
      false => below_edge.action,
    };

    buffer.unsafe_to_break(Some(base), Some(i));
    match action == Action::RD {
      true => {
        buffer.info[base].glyph_id = pua_shape(buffer.info[base].glyph_id, action, face);
      }
      false => {
        buffer.info[i].glyph_id = pua_shape(buffer.info[i].glyph_id, action, face);
      }
    }
  }
}

fn preprocess_text(
  plan: &hb_ot_shape_plan_t,
  face: &mut FontFuncsDispatch,
  buffer: &mut hb_buffer_t,
) {
  #[inline]
  fn is_sara_am(u: u32) -> bool {
    (u & !0x0080) == 0x0E33
  }
  #[inline]
  fn nikhahit_from_sara_am(u: u32) -> u32 {
    u - 0x0E33 + 0x0E4D
  }
  #[inline]
  fn sara_aa_from_sara_am(u: u32) -> u32 {
    u - 1
  }
  #[inline]
  fn is_above_base_mark(u: u32) -> bool {
    let u = u & !0x0080;
    matches!(u, 0x0E34..=0x0E37 | 0x0E47..=0x0E4E | 0x0E31..=0x0E31 | 0x0E3B..=0x0E3B)
  }

  buffer.clear_output();
  buffer.idx = 0;
  while buffer.idx < buffer.len {
    let u = buffer.cur(0).glyph_id;
    match !is_sara_am(u) {
      true => {
        buffer.next_glyph();
        continue;
      }
      false => {}
    }

    buffer.output_glyph(nikhahit_from_sara_am(u));
    {
      let out_idx = buffer.out_len - 1;
      let mut info = buffer.out_info_mut()[out_idx];
      info.set_continuation(&mut buffer.scratch_flags);
    }
    buffer.replace_glyph(sara_aa_from_sara_am(u));

    let end = buffer.out_len;

    buffer.out_info_mut()[end - 2].set_general_category(GeneralCategory::NON_SPACING_MARK);

    let mut start = end - 2;
    while start > 0 && is_above_base_mark(buffer.out_info()[start - 1].glyph_id) {
      start -= 1;
    }

    match start + 2 < end {
      true => {
        buffer.merge_out_clusters(start, end);
        let t = buffer.out_info()[end - 2];
        for i in 0..(end - start - 2) {
          buffer.out_info_mut()[i + start + 1] = buffer.out_info()[i + start];
        }
        buffer.out_info_mut()[start] = t;
      }
      false => {}
    }

    match start != 0 {
      true => {
        buffer.merge_out_grapheme_clusters(start - 1, end);
      }
      false => {}
    }
  }

  buffer.sync();

  match plan.script == Some(script::THAI) && !plan.ot_map.found_script(TableIndex::GSUB) {
    true => {
      do_pua_shaping(face, buffer);
    }
    false => {}
  }
}
