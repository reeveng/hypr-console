#![allow(
  dead_code,
  non_upper_case_globals,
  unused_assignments,
  unused_parens,
  while_true,
  clippy::assign_op_pattern,
  clippy::collapsible_if,
  clippy::comparison_chain,
  clippy::double_parens,
  clippy::unnecessary_cast,
  clippy::single_match,
  clippy::never_loop
)]

use super::buffer::{HB_BUFFER_SCRATCH_FLAG_HAS_BROKEN_SYLLABLE, hb_buffer_t};

static _myanmar_syllable_machine_trans_keys: [u8; 108] = [
  0, 22, 1, 22, 3, 22, 3, 22, 1, 22, 3, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 3, 22, 0, 8, 1,
  22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 3, 22, 3, 22,
  1, 22, 3, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 3, 22, 0, 8, 1, 22, 1, 22, 1, 22, 1, 22, 1,
  22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 1, 22, 0, 22, 0, 8, 0, 0,
];
static _myanmar_syllable_machine_char_class: [i8; 59] = [
  0, 0, 1, 2, 3, 3, 4, 5, 6, 7, 7, 4, 4, 4, 8, 4, 4, 9, 4, 10, 11, 12, 13, 4, 4, 4, 4, 4, 4, 4,
  4, 14, 4, 4, 15, 16, 17, 18, 19, 20, 21, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 22, 0, 0,
];
static _myanmar_syllable_machine_index_offsets: [i16; 55] = [
  0, 23, 45, 65, 85, 107, 127, 149, 171, 193, 215, 237, 257, 266, 288, 310, 332, 354, 376, 398,
  420, 442, 464, 486, 508, 530, 550, 570, 592, 612, 634, 656, 678, 700, 722, 742, 751, 773, 795,
  817, 839, 861, 883, 905, 927, 949, 971, 993, 1015, 1037, 1059, 1081, 1104, 0, 0,
];
static _myanmar_syllable_machine_indices: [i8; 1115] = [
  2, 3, 4, 5, 1, 6, 7, 2, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 24, 25, 26,
  23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 27, 26, 23, 27, 23, 23,
  23, 23, 23, 23, 23, 32, 41, 23, 23, 23, 23, 38, 23, 23, 27, 26, 23, 27, 23, 23, 23, 23, 23, 23,
  23, 23, 23, 23, 23, 23, 23, 38, 23, 23, 27, 42, 23, 26, 23, 27, 38, 23, 23, 23, 23, 23, 23, 23,
  27, 23, 23, 23, 23, 38, 23, 23, 27, 26, 23, 27, 23, 23, 23, 23, 23, 23, 23, 23, 27, 23, 23, 23,
  23, 38, 23, 23, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 43, 23, 23, 32, 44, 45, 23, 23, 23, 38,
  23, 44, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 23, 23, 23, 32, 23, 23, 23, 23, 23, 38, 23, 23,
  27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 43, 23, 23, 32, 23, 23, 23, 23, 23, 38, 23, 23, 27, 24,
  23, 26, 23, 27, 28, 23, 23, 23, 43, 23, 23, 32, 44, 23, 23, 23, 23, 38, 23, 23, 27, 24, 23, 26,
  23, 27, 28, 23, 23, 23, 43, 23, 23, 32, 44, 23, 23, 23, 23, 38, 23, 44, 27, 26, 23, 27, 23, 23,
  23, 23, 23, 23, 23, 32, 23, 23, 23, 23, 23, 38, 23, 23, 27, 2, 23, 23, 23, 23, 23, 23, 23, 2,
  24, 23, 26, 23, 27, 28, 23, 23, 23, 29, 30, 23, 32, 23, 23, 23, 23, 23, 38, 23, 23, 27, 24, 23,
  26, 23, 27, 28, 23, 23, 23, 23, 30, 23, 32, 23, 23, 23, 23, 23, 38, 23, 23, 27, 24, 23, 26, 23,
  27, 28, 23, 23, 23, 29, 30, 31, 32, 23, 23, 23, 23, 23, 38, 46, 23, 27, 24, 23, 26, 23, 27, 28,
  23, 23, 23, 29, 30, 31, 32, 23, 23, 23, 23, 23, 38, 23, 23, 27, 24, 23, 26, 23, 27, 28, 23, 23,
  23, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 23, 40, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 29,
  30, 31, 32, 46, 23, 23, 23, 23, 38, 23, 40, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31,
  32, 46, 23, 23, 23, 23, 38, 23, 23, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 23,
  34, 23, 36, 23, 38, 23, 40, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 46, 34, 23,
  23, 23, 38, 23, 40, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 47, 34, 35, 36, 23,
  38, 23, 40, 27, 24, 23, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 23, 34, 35, 36, 23, 38, 23,
  40, 27, 24, 25, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 23, 40, 27,
  49, 48, 6, 48, 48, 48, 48, 48, 48, 48, 13, 50, 48, 48, 48, 48, 19, 48, 48, 6, 49, 51, 6, 51,
  51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 19, 51, 51, 6, 52, 48, 49, 48, 6, 19, 48, 48,
  48, 48, 48, 48, 48, 6, 48, 48, 48, 48, 19, 48, 48, 6, 49, 48, 6, 48, 48, 48, 48, 48, 48, 48,
  48, 6, 48, 48, 48, 48, 19, 48, 48, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 53, 48, 48, 13, 54, 55,
  48, 48, 48, 19, 48, 54, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 48, 48, 48, 13, 48, 48, 48, 48, 48,
  19, 48, 48, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 53, 48, 48, 13, 48, 48, 48, 48, 48, 19, 48, 48,
  6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 53, 48, 48, 13, 54, 48, 48, 48, 48, 19, 48, 48, 6, 3, 48,
  49, 48, 6, 7, 48, 48, 48, 53, 48, 48, 13, 54, 48, 48, 48, 48, 19, 48, 54, 6, 49, 48, 6, 48, 48,
  48, 48, 48, 48, 48, 13, 48, 48, 48, 48, 48, 19, 48, 48, 6, 56, 48, 48, 48, 48, 48, 48, 48, 56,
  3, 4, 49, 48, 6, 7, 48, 48, 48, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 6, 3, 48, 49,
  48, 6, 7, 48, 48, 48, 10, 11, 48, 13, 48, 48, 48, 48, 48, 19, 48, 48, 6, 3, 48, 49, 48, 6, 7,
  48, 48, 48, 48, 11, 48, 13, 48, 48, 48, 48, 48, 19, 48, 48, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48,
  10, 11, 12, 13, 48, 48, 48, 48, 48, 19, 57, 48, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 10, 11, 12,
  13, 48, 48, 48, 48, 48, 19, 48, 48, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 10, 11, 12, 13, 14, 15,
  16, 17, 18, 19, 48, 21, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 10, 11, 12, 13, 57, 48, 48, 48, 48,
  19, 48, 21, 6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 10, 11, 12, 13, 57, 48, 48, 48, 48, 19, 48, 48,
  6, 3, 48, 49, 48, 6, 7, 48, 48, 48, 10, 11, 12, 13, 48, 15, 48, 17, 48, 19, 48, 21, 6, 3, 48,
  49, 48, 6, 7, 48, 48, 48, 10, 11, 12, 13, 57, 15, 48, 48, 48, 19, 48, 21, 6, 3, 48, 49, 48, 6,
  7, 48, 48, 48, 10, 11, 12, 13, 58, 15, 16, 17, 48, 19, 48, 21, 6, 3, 48, 49, 48, 6, 7, 48, 48,
  48, 10, 11, 12, 13, 48, 15, 16, 17, 48, 19, 48, 21, 6, 3, 4, 49, 48, 6, 7, 48, 48, 48, 10, 11,
  12, 13, 14, 15, 16, 17, 18, 19, 48, 21, 6, 24, 25, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32,
  59, 34, 35, 36, 37, 38, 39, 40, 27, 24, 60, 26, 23, 27, 28, 23, 23, 23, 29, 30, 31, 32, 33, 34,
  35, 36, 37, 38, 23, 40, 27, 2, 3, 4, 49, 48, 6, 7, 2, 2, 48, 10, 11, 12, 13, 14, 15, 16, 17,
  18, 19, 20, 21, 6, 2, 61, 61, 61, 61, 61, 61, 2, 2, 0, 0,
];
static _myanmar_syllable_machine_index_defaults: [i8; 55] = [
  1, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23,
  23, 48, 51, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
  48, 23, 23, 48, 61, 0, 0,
];
static _myanmar_syllable_machine_cond_targs: [i8; 64] = [
  0, 0, 1, 25, 35, 0, 26, 30, 49, 52, 37, 38, 39, 29, 41, 42, 44, 45, 46, 27, 48, 43, 26, 0, 2,
  12, 0, 3, 7, 13, 14, 15, 6, 17, 18, 20, 21, 22, 4, 24, 19, 11, 5, 8, 9, 10, 16, 23, 0, 0, 34,
  0, 28, 31, 32, 33, 36, 40, 47, 50, 51, 0, 0, 0,
];
static _myanmar_syllable_machine_cond_actions: [i8; 64] = [
  0, 3, 0, 0, 0, 4, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 7, 0, 0, 8, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 10, 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 0,
  0,
];
static _myanmar_syllable_machine_to_state_actions: [i8; 55] = [
  1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
static _myanmar_syllable_machine_from_state_actions: [i8; 55] = [
  2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
static _myanmar_syllable_machine_eof_trans: [i8; 55] = [
  1, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
  24, 49, 52, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49, 49,
  49, 24, 24, 49, 62, 0, 0,
];
static myanmar_syllable_machine_start: i32 = 0;
static myanmar_syllable_machine_first_final: i32 = 0;
static myanmar_syllable_machine_error: i32 = -1;
static myanmar_syllable_machine_en_main: i32 = 0;
#[derive(Clone, Copy)]
pub enum SyllableType {
  ConsonantSyllable = 0,
  PunctuationCluster,
  BrokenCluster,
  NonMyanmarCluster,
}

pub fn find_syllables_myanmar(buffer: &mut hb_buffer_t) {
  let mut cs = 0;
  let mut ts = 0;
  let mut te;
  let mut act = 0;
  let mut p = 0;
  let pe = buffer.len;
  let eof = buffer.len;
  let mut syllable_serial = 1u8;

  macro_rules! found_syllable {
    ($kind:expr) => {{
      found_syllable(ts, te, &mut syllable_serial, $kind, buffer);
    }};
  }

  {
    cs = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i32>((myanmar_syllable_machine_start));
      migrated_value
    });
    ts = 0;
    te = 0;
    act = 0;
  }

  {
    let mut _trans = 0;
    let mut _keys: i32 = 0;
    let mut _inds: i32 = 0;
    let mut _ic = 0;
    '_resume: while (p != pe || p == eof) {
      '_again: while (true) {
        match (_myanmar_syllable_machine_from_state_actions[({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index((cs));
          migrated_value
        })]) {
          2 => {
            ts = p;
          }

          _ => {}
        }
        match (p == eof) {
          true => match (_myanmar_syllable_machine_eof_trans[({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index((cs));
            migrated_value
          })]
            > 0)
          {
            true => {
              _trans = ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                  (_myanmar_syllable_machine_eof_trans[({
                    let Ok(migrated_value) = console_core_number_conversion::wrapped_index((cs));
                    migrated_value
                  })]),
                );
                migrated_value
              }) - 1;
            }
            false => {}
          },
          false => {
            _keys = ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>((cs << 1));
              migrated_value
            });
            _inds = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                (_myanmar_syllable_machine_index_offsets[({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index((cs));
                  migrated_value
                })]),
              );
              migrated_value
            });
            match (({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(
                buffer.info[p].myanmar_category(),
              );
              migrated_value
            }) <= 57
              && ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(
                  buffer.info[p].myanmar_category(),
                );
                migrated_value
              }) >= 1)
            {
              true => {
                _ic = ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                    (_myanmar_syllable_machine_char_class[({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
                        (({
                          let Ok(myanmar_category) =
                            console_core_number_conversion::wrapped::<_, u8>(
                              buffer.info[p].myanmar_category(),
                            );
                          let Ok(migrated_value) =
                            console_core_number_conversion::wrapped::<_, i32>(myanmar_category);
                          migrated_value
                        }) - 1),
                      );
                      migrated_value
                    })]),
                  );
                  migrated_value
                });
                match (_ic
                  <= ({
                    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                      (_myanmar_syllable_machine_trans_keys[({
                        let Ok(migrated_value) =
                          console_core_number_conversion::wrapped_index((_keys + 1));
                        migrated_value
                      })]),
                    );
                    migrated_value
                  })
                  && _ic
                    >= ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                        (_myanmar_syllable_machine_trans_keys[({
                          let Ok(migrated_value) =
                            console_core_number_conversion::wrapped_index((_keys));
                          migrated_value
                        })]),
                      );
                      migrated_value
                    })) {
                  true => {
                    _trans = ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                        (_myanmar_syllable_machine_indices[({
                          let Ok(keys) = console_core_number_conversion::wrapped_index((_keys));
                          let Ok(myanmar_syllable_machine_trans_key) =
                            console_core_number_conversion::wrapped::<_, i32>(
                              (_myanmar_syllable_machine_trans_keys[keys]),
                            );
                          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
                            (_inds
                              + ({
                                let Ok(migrated_value) =
                                  console_core_number_conversion::wrapped::<_, i32>(
                                    (_ic - myanmar_syllable_machine_trans_key),
                                  );
                                migrated_value
                              })),
                          );
                          migrated_value
                        })]),
                      );
                      migrated_value
                    });
                  }
                  false => {
                    _trans = ({
                      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                        (_myanmar_syllable_machine_index_defaults[({
                          let Ok(migrated_value) =
                            console_core_number_conversion::wrapped_index((cs));
                          migrated_value
                        })]),
                      );
                      migrated_value
                    });
                  }
                }
              }
              false => {
                _trans = ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                    (_myanmar_syllable_machine_index_defaults[({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped_index((cs));
                      migrated_value
                    })]),
                  );
                  migrated_value
                });
              }
            }
          }
        }
        cs = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
            (_myanmar_syllable_machine_cond_targs[({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index((_trans));
              migrated_value
            })]),
          );
          migrated_value
        });
        match (_myanmar_syllable_machine_cond_actions[({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index((_trans));
          migrated_value
        })]
          != 0)
        {
          true => match (_myanmar_syllable_machine_cond_actions[({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index((_trans));
            migrated_value
          })]) {
            8 => {
              te = p + 1;
              {
                found_syllable!(SyllableType::ConsonantSyllable);
              }
            }
            4 => {
              te = p + 1;
              {
                found_syllable!(SyllableType::NonMyanmarCluster);
              }
            }
            10 => {
              te = p + 1;
              {
                found_syllable!(SyllableType::BrokenCluster);
                buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_BROKEN_SYLLABLE;
              }
            }
            3 => {
              te = p + 1;
              {
                found_syllable!(SyllableType::NonMyanmarCluster);
              }
            }
            7 => {
              te = p;
              p = p - 1;
              {
                found_syllable!(SyllableType::ConsonantSyllable);
              }
            }
            9 => {
              te = p;
              p = p - 1;
              {
                found_syllable!(SyllableType::BrokenCluster);
                buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_BROKEN_SYLLABLE;
              }
            }
            12 => {
              te = p;
              p = p - 1;
              {
                found_syllable!(SyllableType::NonMyanmarCluster);
              }
            }
            11 => match (act) {
              2 => {
                p = (te) - 1;
                {
                  found_syllable!(SyllableType::NonMyanmarCluster);
                }
              }
              3 => {
                p = (te) - 1;
                {
                  found_syllable!(SyllableType::BrokenCluster);
                  buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_BROKEN_SYLLABLE;
                }
              }

              _ => {}
            },
            6 => {
              {
                {
                  te = p + 1;
                }
              }
              {
                {
                  act = 2;
                }
              }
            }
            5 => {
              {
                {
                  te = p + 1;
                }
              }
              {
                {
                  act = 3;
                }
              }
            }

            _ => {}
          },
          false => {}
        }
        break '_again;
      }
      match (p == eof) {
        true => match (cs >= 0) {
          true => {
            break '_resume;
          }
          false => {}
        },
        false => {
          match (_myanmar_syllable_machine_to_state_actions[({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index((cs));
            migrated_value
          })]) {
            1 => {
              ts = 0;
            }

            _ => {}
          }
          p += 1;
          continue '_resume;
        }
      }
      break '_resume;
    }
  }
}

#[inline]
fn found_syllable(
  start: usize,
  end: usize,
  syllable_serial: &mut u8,
  kind: SyllableType,
  buffer: &mut hb_buffer_t,
) {
  for i in start..end {
    buffer.info[i].set_syllable((*syllable_serial << 4) | kind as u8);
  }

  *syllable_serial += 1;

  match *syllable_serial == 16 {
    true => {
      *syllable_serial = 1;
    }
    false => {}
  }
}
