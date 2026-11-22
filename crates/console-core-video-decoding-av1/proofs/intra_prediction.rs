//! Intra-mode selection and edge upsampling equivalence checks.
//! Run with `cargo kani -p console-core-video-decoding-av1 --lib --harness intra_prediction_proofs`.
//! With Kani 0.68, set `RUSTFLAGS="--cfg no_zerocopy_allocator_api_1_100_0"`
//! to select the allocator API available on its pinned compiler.

use super::*;

// Retain the previous branch structure to verify the table-based selector.
fn previous_mode(
  mut mode: IntraPredMode,
  angle: &mut c_int,
  have_top: bool,
  have_left: bool,
) -> Result<IntraPredMode, console_core_never::Never> {
  match mode {
    VERT_PRED..=VERT_LEFT_PRED => {
      let base_angle: i32 = match mode {
        VERT_PRED => 90,
        HOR_PRED => 180,
        DIAG_DOWN_LEFT_PRED => 45,
        DIAG_DOWN_RIGHT_PRED => 135,
        VERT_RIGHT_PRED => 113,
        HOR_DOWN_PRED => 157,
        HOR_UP_PRED => 203,
        VERT_LEFT_PRED => 67,
        _ => return Ok(mode),
      };
      *angle = base_angle.wrapping_add(3i32.wrapping_mul(*angle));

      match ::core::primitive::bool::then_some(*angle <= 90, ()) {
        ::core::option::Option::Some(()) => {
          mode = match ::core::primitive::bool::then_some(*angle < 90 && have_top, ()) {
            ::core::option::Option::Some(()) => Z1_PRED,
            ::core::option::Option::None => VERT_PRED,
          };
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(*angle < 180, ()) {
          ::core::option::Option::Some(()) => {
            mode = Z2_PRED;
          }
          ::core::option::Option::None => {
            mode = match ::core::primitive::bool::then_some(*angle > 180 && have_left, ()) {
              ::core::option::Option::Some(()) => Z3_PRED,
              ::core::option::Option::None => HOR_PRED,
            };
          }
        },
      }
    }
    DC_PRED | PAETH_PRED => {
      mode = match (have_left, have_top, mode) {
        (false, false, _) => DC_128_PRED,
        (false, true, DC_PRED) => TOP_DC_PRED,
        (false, true, _) => VERT_PRED,
        (true, false, DC_PRED) => LEFT_DC_PRED,
        (true, false, _) => HOR_PRED,
        (true, true, unchanged) => unchanged,
      };
    }
    _ => {}
  }
  Ok(mode)
}
#[kani::proof]
fn intra_mode_equivalence() {
  let mode: IntraPredMode = kani::any();
  let has_top: bool = kani::any();
  let has_left: bool = kani::any();
  let input_angle: i32 = kani::any();
  let mut angle_before = input_angle;
  let mut angle_after = input_angle;
  let Ok(above) = AboveEdge::select(EdgeAvailability::from(has_top), None);
  let config = IntraEdgePreparation {
    above,
    has_left,
    position: Point { x: 0, y: 0 },
    frame: Size { width: 0, height: 0 },
    transform: Size { width: 0, height: 0 },
    available: EdgeFlags::EMPTY,
    filter_corner: 0,
  };
  let Ok(mode_before) = previous_mode(mode, &mut angle_before, has_top, has_left);
  let Ok(mode_after) = prepare_intra_mode(&config, mode, &mut angle_after);
  assert_eq!(mode_before, mode_after);
  assert_eq!(angle_before, angle_after);
  kani::cover!(mode_before == Z1_PRED && mode == HOR_PRED && has_top);
  kani::cover!(mode_before == Z3_PRED && mode == VERT_PRED && has_left);
}


fn reference_upsampled_sample([first, second, third, fourth]: [u8; 4]) -> u8 {
  let average = (
    9 * (i32::from(second) + i32::from(third)) - i32::from(first) - i32::from(fourth) + 8
  ) >> 4;
  let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(average.clamp(0, 255));
  sample
}

#[kani::proof]
#[kani::unwind(8)]
fn upsampling_a_four_pixel_edge_matches_the_reference() {
  use crate::src::ipred::{EdgeUpsampling, PredictionEdge, upsample_edge};

  let input: [u8; 5] = kani::any();
  let [a, b, c, d, e] = input;
  let mut output: [u8; 16] = kani::any();
  let [_, _, _, _, _, _, _, _, _, s9, s10, s11, s12, s13, s14, s15] = output;
  let expected = [
    a, reference_upsampled_sample([a, a, b, c]),
    b, reference_upsampled_sample([a, b, c, d]),
    c, reference_upsampled_sample([b, c, d, e]),
    d, reference_upsampled_sample([c, d, e, e]),
    e, s9, s10, s11, s12, s13, s14, s15,
  ];

  let result = upsample_edge(
    &mut output,
    PredictionEdge { pixels: &input, origin: 0, window: 0..5 },
    EdgeUpsampling { count: 5, ceiling: 255 },
  );

  assert_eq!(result, Ok(()));
  assert_eq!(u128::from_ne_bytes(output), u128::from_ne_bytes(expected));
  kani::cover!(a != e);
}
