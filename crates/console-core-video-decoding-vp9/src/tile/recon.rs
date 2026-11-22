//! Residual syntax (6.4.21 - 6.4.26) with the prediction and reconstruction processes it
//! triggers (8.5, 8.6).

use super::{Blk, BlockPrediction, ContextChoice, TileDecoder, count_choice};
use crate::boolcoder::BoolDecoder;
use crate::error::Error;
use crate::frame::{Mv, Place, Samples, Subsampling, WideMv};
use crate::inter::{self, Motion, RefPlane};
use crate::intra::{self, AdjacentEdges, IntraBlock, IntraEdge, RightExtension};
use crate::places::{after_mut, length, read, row, span, span_mut, write};
use crate::tables::*;
use crate::transform;
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

fn round_q2(v: i32) -> core::result::Result<i32, console_core_never::Never> {
  Ok(::core::primitive::i32::wrapping_div(
    match ::core::primitive::bool::then_some(v < 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_sub(v, 1),
      ::core::option::Option::None => i32::wrapping_add(v, 1),
    },
    2,
  ))
}

fn round_q4(v: i32) -> core::result::Result<i32, console_core_never::Never> {
  Ok(::core::primitive::i32::wrapping_div(
    match ::core::primitive::bool::then_some(v < 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_sub(v, 2),
      ::core::option::Option::None => i32::wrapping_add(v, 2),
    },
    4,
  ))
}

impl<'a> TileDecoder<'a> {
  pub(super) fn residual(&mut self, bd: &mut BoolDecoder, block: &mut Blk) -> Result<(), Never> {
    let chroma = self.subsampling;
    let chroma_transform = match block.size {
      ..=2 => 0,
      _ => {
        let Ok(small) = subsampled_size(block.size, chroma);
        let Ok(maximum) = read(&MAX_TXSIZE, u64::from(small));
        block.tx_size.min(maximum)
      }
    };

    for plane in 0u64..3 {
      let Ok(subsampling) = chroma.of_plane(plane);
      let transform_size = match plane {
        0 => block.tx_size,
        _ => chroma_transform,
      };
      let Ok(geometry) = self.transform_plane(
        block,
        plane,
        TransformSampling {
          subsampling,
          transform_size,
        },
      );
      let Ok(()) = self.reconstruct_plane(bd, block, geometry);
    }

    Ok(())
  }

  fn transform_plane(
    &self,
    block: &Blk,
    plane: u64,
    sampling: TransformSampling,
  ) -> Result<TransformPlane, Never> {
    let Subsampling { x, y } = sampling.subsampling;
    let full_size = block.size.max(BLOCK_8X8);
    let Ok(block_size) = subsampled_size(full_size, sampling.subsampling);
    let Ok(width) = read(&NUM_4X4_WIDE, u64::from(block_size));
    let Ok(height) = read(&NUM_4X4_HIGH, u64::from(block_size));
    Ok(TransformPlane {
      plane,
      subsampling: sampling.subsampling,
      width_units: u64::from(width),
      height_units: u64::from(height),
      step: 1u64.wrapping_shl(u32::from(sampling.transform_size)),
      size: sampling.transform_size,
      base: Place {
        x: block.c.wrapping_mul(8).wrapping_shr(x),
        y: block.r.wrapping_mul(8).wrapping_shr(y),
      },
      max_x: self.mi_cols.wrapping_mul(8).wrapping_shr(x),
      max_y: self.mi_rows.wrapping_mul(8).wrapping_shr(y),
    })
  }

  fn reconstruct_plane(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
    geometry: TransformPlane,
  ) -> Result<(), Never> {
    match (block.prediction == BlockPrediction::Inter).then_some(()) {
      Some(()) => {
        let Ok(()) = self.predict_plane(block, &geometry);
      }
      None => {}
    }

    let Ok(step) = index(geometry.step);
    let mut block_index = 0u64;

    for row in (0..geometry.height_units).step_by(step.max(1)) {
      for column in (0..geometry.width_units).step_by(step.max(1)) {
        let origin = Place {
          x: geometry.base.x.wrapping_add(column.wrapping_mul(4)),
          y: geometry.base.y.wrapping_add(row.wrapping_mul(4)),
        };
        let at = ResidualPosition {
          row,
          column,
          origin,
          index: block_index,
        };
        let Ok(nonzero) = self.reconstruct_block(bd, block, &geometry, at);
        let Ok(()) = self.record_nonzero(&geometry, origin, nonzero);
        block_index = block_index.wrapping_add(1);
      }
    }

    Ok(())
  }

  fn predict_plane(&mut self, block: &Blk, geometry: &TransformPlane) -> Result<(), Never> {
    match block.size {
      ..=2 => {
        for row in 0..geometry.height_units {
          for column in 0..geometry.width_units {
            let start = Place {
              x: geometry.base.x.wrapping_add(column.wrapping_mul(4)),
              y: geometry.base.y.wrapping_add(row.wrapping_mul(4)),
            };
            let block_index = row.wrapping_mul(geometry.width_units).wrapping_add(column);
            let Ok(()) = self.predict_inter(
              block,
              InterBlock {
                plane: geometry.plane,
                subsampling: geometry.subsampling,
                start,
                width: 4,
                height: 4,
                block_index,
              },
            );
          }
        }
      }
      _ => {
        let Ok(()) = self.predict_inter(
          block,
          InterBlock {
            plane: geometry.plane,
            subsampling: geometry.subsampling,
            start: geometry.base,
            width: geometry.width_units.wrapping_mul(4),
            height: geometry.height_units.wrapping_mul(4),
            block_index: 0,
          },
        );
      }
    }

    Ok(())
  }

  fn reconstruct_block(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
    geometry: &TransformPlane,
    at: ResidualPosition,
  ) -> Result<u8, Never> {
    match (at.origin.x < geometry.max_x && at.origin.y < geometry.max_y).then_some(()) {
      Some(()) => {}
      None => return Ok(0),
    }

    let Ok(kind) = self.tx_type(block, geometry, at.index);
    let transform = Transform {
      size: geometry.size,
      kind,
    };

    match (block.prediction == BlockPrediction::Inter).then_some(()) {
      Some(()) => {}
      None => {
        let Ok(()) = self.predict_intra_block(block, geometry, &at);
      }
    }

    match block.skip.then_some(()) {
      Some(()) => return Ok(0),
      None => {}
    }

    let input = CoefficientBlock {
      plane: geometry.plane,
      origin: at.origin,
      transform,
      x_limit: geometry.max_x.wrapping_shr(2),
      y_limit: geometry.max_y.wrapping_shr(2),
    };
    let Ok(nonzero) = self.tokens(bd, block, input);

    match nonzero.end {
      0 => Ok(0),
      _ => {
        block.eob_total = block.eob_total.wrapping_add(1);
        let Ok(()) = self.apply_residual(geometry.plane, at.origin, transform, nonzero);
        Ok(1)
      }
    }
  }

  fn predict_intra_block(
    &mut self,
    block: &Blk,
    geometry: &TransformPlane,
    at: &ResidualPosition,
  ) -> Result<(), Never> {
    let Ok(mode) = match geometry.plane {
      0 => intra_mode(block, at.index),
      _ => Ok(block.uv_mode),
    };
    let right = match (at.column.wrapping_add(geometry.step) < geometry.width_units).then_some(())
    {
      Some(()) => RightExtension::ReadSamples,
      None => RightExtension::RepeatEdge,
    };
    let edge = IntraEdge {
      adjacent: AdjacentEdges::from((
        block.neighbors.left.is_some() || at.column > 0,
        block.neighbors.above.is_some() || at.row > 0,
      )),
      right,
      tx_size: geometry.size,
      max_x: geometry.max_x.wrapping_sub(1),
      max_y: geometry.max_y.wrapping_sub(1),
      bit_depth: self.bit_depth,
    };
    let Ok((stride, band_start)) = strip_layout(&self.strip, geometry.plane);
    let Ok(native_plane) = index(geometry.plane);

    match self.strip.planes.get_mut(native_plane) {
      Some(samples) => {
        let destination = IntraBlock {
          stride,
          band_start,
          x: at.origin.x,
          y: at.origin.y,
        };
        intra::predict(samples, destination, mode, &edge)
      }
      None => Ok(()),
    }
  }

  fn apply_residual(
    &mut self,
    plane: u64,
    origin: Place,
    transform: Transform,
    coefficients: Reconstructed,
  ) -> Result<(), Never> {
    let Ok((stride, band_start)) = strip_layout(&self.strip, plane);
    let Ok(native_plane) = index(plane);

    match self.strip.planes.get_mut(native_plane) {
      Some(samples) => {
        let target = Samples {
          data: samples,
          stride,
          x_offset: band_start,
        };
        let Ok(offset) = target.offset(origin);
        let Ok(destination) = after_mut(target.data, offset);
        let Ok(quantization) = self.h.quantization();
        let Ok(()) = transform::inverse_transform_add(
          &transform::Residual {
            coefficients: &self.coefs,
            transform,
            quantization,
            end: coefficients.end,
            rows: coefficients.rows,
            bit_depth: self.bit_depth,
          },
          transform::Destination {
            samples: destination,
            stride,
          },
        );
      }
      None => {}
    }

    let Ok(order) = scan(transform);
    let Ok(used) = index(coefficients.end);

    for &position in order.iter().take(used) {
      let Ok(value) = wrapped::<_, u64>(position);
      let Ok(()) = write(&mut self.coefs, value, 0);
    }

    Ok(())
  }

  fn record_nonzero(
    &mut self,
    plane: &TransformPlane,
    origin: Place,
    nonzero: u8,
  ) -> Result<(), Never> {
    let Ok(at) = index(plane.plane);

    for (buffers, position) in [
      (&mut self.above_nonzero, origin.x),
      (&mut self.left_nonzero, origin.y),
    ] {
      match buffers.get_mut(at) {
        Some(state) => {
          let Ok(length) = length(state);
          let start = position.wrapping_shr(2).min(length);
          let end = start.wrapping_add(plane.step).min(length);
          let Ok(run) = span_mut(state, start..end);
          run.fill(nonzero);
        }
        None => {}
      }
    }

    Ok(())
  }

  fn tx_type(&self, block: &Blk, plane: &TransformPlane, index: u64) -> Result<u8, Never> {
    let Ok(quantization) = self.h.quantization();

    match (plane.plane, plane.size, quantization, block.prediction) {
      (1.., _, _, _) | (_, TX_32X32, _, _) => Ok(DCT_DCT),
      (_, TX_4X4, transform::Quantization::Lossless, _)
      | (_, TX_4X4, _, BlockPrediction::Inter) => Ok(DCT_DCT),
      (_, TX_4X4, _, BlockPrediction::Intra) => {
        let Ok(mode) = intra_mode(block, index);
        read(&MODE2TXFM, u64::from(mode))
      }
      _ => read(&MODE2TXFM, u64::from(block.y_mode)),
    }
  }

  fn tokens(
    &mut self,
    bd: &mut BoolDecoder,
    block: &Blk,
    input: CoefficientBlock,
  ) -> Result<Reconstructed, Never> {
    let transform = input.transform;
    let exponent = u32::from(transform.size).wrapping_add(2);
    let segment_limit = 16u64.wrapping_shl(u32::from(transform.size).wrapping_mul(2));
    let Ok(limit) = index(segment_limit);
    let Ok(order) = scan(transform);
    let bands: &[u8] = match transform.size {
      TX_4X4 => &COEFBAND_4X4,
      _ => &COEFBAND_8X8PLUS,
    };
    let plane_type = u64::from(input.plane > 0);
    let reference_type = u64::from(block.prediction == BlockPrediction::Inter);
    let Ok(first_context) = self.initial_coefficient_context(input);
    let Ok(segment) = row(&self.s.seg_q, u64::from(block.seg_id));
    let Ok(q) = row(segment, plane_type);
    let Ok(dc) = read(q, 0);
    let Ok(ac) = read(q, 1);
    let mut result = Reconstructed { end: 0, rows: 1 };
    let mut check = EndCheck::Read;
    let mut last_row = 0u64;

    for ((count, &coefficient), &band) in (0u64..).zip(order.iter()).zip(bands).take(limit) {
      let Ok(position) = wrapped::<_, u64>(coefficient);
      let Ok(context) = match count {
        0 => Ok(first_context),
        _ => self.following_coefficient_context(transform, position),
      };
      let key = CoefficientContext {
        size: transform.size,
        plane_type,
        reference_type,
        band,
        context,
      };
      let Ok(address) = key.position();
      let Ok([more, present, one]) = self.coefficient_probabilities(address);

      match check {
        EndCheck::Read => {
          let Ok(bit) = bd.read_bool(more);
          let Ok(()) = self.count_end(ContextChoice {
            context: address,
            value: u64::from(bit != 0),
          });

          match bit {
            0 => break,
            _ => {}
          }
        }
        EndCheck::Skip => {}
      }

      let Ok(exists) = bd.read_bool(present);
      let Ok(()) = match exists {
        0 => self.count_token(ContextChoice {
          context: address,
          value: 0,
        }),
        _ => Ok(()),
      };

      match exists {
        0 => {
          let Ok(()) = write(&mut self.token_cache, position, 0);
          check = EndCheck::Skip;
          result.end = count.wrapping_add(1);
          continue;
        }
        _ => {}
      }

      check = EndCheck::Read;
      let Ok(larger) = bd.read_bool(one);
      let (token, magnitude) = match larger {
        0 => {
          let Ok(()) = self.count_token(ContextChoice {
            context: address,
            value: 1,
          });
          (1, 1)
        }
        _ => {
          let Ok(()) = self.count_token(ContextChoice {
            context: address,
            value: 2,
          });
          let Ok(decoded) = read_coefficient(
            bd,
            self::ReadCoefficientArguments {
              probability: one,
              depth: self.bit_depth,
            },
          );
          decoded
        }
      };
      let quantizer = match position {
        0 => dc,
        _ => ac,
      };
      let shift = u32::from(transform.size == TX_32X32);
      let value = CoefficientValue {
        position,
        token,
        magnitude,
        quantizer,
        shift,
      };
      let Ok(()) = self.store_coefficient(bd, value);
      last_row = last_row.max(position.wrapping_shr(exponent));
      result.end = count.wrapping_add(1);
    }

    result.rows = last_row.wrapping_add(1);
    Ok(result)
  }

  fn following_coefficient_context(
    &self,
    transform: Transform,
    position: u64,
  ) -> Result<u8, Never> {
    let Ok(neighbors) = neighbours(transform, position);
    let Ok(above) = read(&self.token_cache, u64::from(neighbors.above));
    let Ok(left) = read(&self.token_cache, u64::from(neighbors.left));
    Ok(1u8.wrapping_add(above).wrapping_add(left).wrapping_shr(1))
  }

  fn coefficient_probabilities(&self, address: u64) -> Result<[u8; 3], Never> {
    let cube = self
      .fc
      .coef
      .as_flattened()
      .as_flattened()
      .as_flattened()
      .as_flattened();
    let Ok(probabilities) = row(cube, address);
    Ok(match probabilities {
      &[more, present, one] => [more, present, one],
      _ => [0; 3],
    })
  }

  fn store_coefficient(
    &mut self,
    bd: &mut BoolDecoder,
    coefficient: CoefficientValue,
  ) -> Result<(), Never> {
    let CoefficientValue {
      position,
      token,
      magnitude,
      quantizer,
      shift,
    } = coefficient;
    let Ok(energy) = read(&ENERGY_CLASS, u64::from(token));
    let Ok(()) = write(&mut self.token_cache, position, energy);
    let Ok(value) = wrapped::<_, i32>(
      magnitude
        .wrapping_mul(i64::from(quantizer))
        .wrapping_shr(shift),
    );
    let Ok(sign) = bd.read_bool(128);
    let signed = match sign {
      0 => value,
      _ => value.wrapping_neg(),
    };
    write(&mut self.coefs, position, signed)
  }

  fn initial_coefficient_context(&self, input: CoefficientBlock) -> Result<u8, Never> {
    let side = 1u64.wrapping_shl(u32::from(input.transform.size));
    let Ok(at) = index(input.plane);
    let mut initial = 0u8;

    for (planes, position, bound) in [
      (
        &self.above_nonzero,
        input.origin.x.wrapping_shr(2),
        input.x_limit,
      ),
      (
        &self.left_nonzero,
        input.origin.y.wrapping_shr(2),
        input.y_limit,
      ),
    ] {
      match planes.get(at) {
        Some(neighbors) => {
          let end = position.wrapping_add(side).min(bound);
          let mut nonzero = 0u8;

          for slot in position..end {
            let Ok(value) = read(neighbors, slot);
            nonzero |= value;
          }

          initial = initial.wrapping_add(nonzero);
        }
        None => {}
      }
    }

    Ok(initial)
  }

  fn count_end(&mut self, choice: ContextChoice) -> Result<(), Never> {
    let counts = self
      .strip
      .counts
      .more_coefs
      .as_flattened_mut()
      .as_flattened_mut()
      .as_flattened_mut()
      .as_flattened_mut();
    count_choice(self.s.counting, counts, choice)
  }

  fn count_token(&mut self, choice: ContextChoice) -> Result<(), Never> {
    let counts = self
      .strip
      .counts
      .token
      .as_flattened_mut()
      .as_flattened_mut()
      .as_flattened_mut()
      .as_flattened_mut();
    count_choice(self.s.counting, counts, choice)
  }

  fn predict_inter(&mut self, block: &Blk, output: InterBlock) -> Result<(), Never> {
    let [first, second] = block.ref_frame;
    let [first_vectors, second_vectors] = block.block_mvs;
    let Ok(()) =
      self.predict_reference(block, output, first, &first_vectors, PredictionSlot::First);

    match (second > INTRA_FRAME).then_some(()) {
      Some(()) => {
        let Ok(()) = self.predict_reference(
          block,
          output,
          second,
          &second_vectors,
          PredictionSlot::Second,
        );
        self.blend_prediction(output, PredictionSlot::Second)
      }
      None => self.blend_prediction(output, PredictionSlot::First),
    }
  }

  fn predict_reference(
    &mut self,
    block: &Blk,
    output: InterBlock,
    reference: i8,
    vectors: &[Mv; 4],
    slot: PredictionSlot,
  ) -> Result<(), Never> {
    let destination = match slot {
      PredictionSlot::First => &mut self.pred,
      PredictionSlot::Second => &mut self.pred2,
    };
    let Ok(at) = index(reference.wrapping_sub(1).clamp(0, 2));
    let source = match self.s.refs.get(at) {
      Some(Some(source)) => source,
      Some(None) | None => {
        match &self.error {
          Some(_error) => {}
          None => {
            self.error = Some(Error::MissingReference(
              crate::error::MissingReference::Unusable { reference },
            ));
          }
        }

        let len = output.width.wrapping_mul(output.height);
        let Ok(samples) = span_mut(destination, 0..len);
        samples.fill(1u16.wrapping_shl(u32::from(self.bit_depth.wrapping_sub(1))));
        return Ok(());
      }
    };

    let Ok(mv) = prediction_vector(block.size, output.subsampling, vectors, output.block_index);
    let Ok(WideMv {
      row: motion_y,
      col: motion_x,
    }) = mv.wide();
    let Ok(block_width) = read(&NUM_8X8_WIDE, u64::from(block.size));
    let Ok(block_height) = read(&NUM_8X8_HIGH, u64::from(block.size));
    let Ok(cmv_x) = clamped_motion(
      motion_x,
      self::ClampedMotionArguments {
        block_origin: block.c,
        frame_extent: self.mi_cols,
      },
      block_width,
      output.subsampling.x,
    );
    let Ok(cmv_y) = clamped_motion(
      motion_y,
      self::ClampedMotionArguments {
        block_origin: block.r,
        frame_extent: self.mi_rows,
      },
      block_height,
      output.subsampling.y,
    );

    let (x, y) = match (source.x_step, source.y_step, source.x_scale, source.y_scale) {
      (16, 16, 16384, 16384) => {
        let Ok(x) = wrapped::<_, i32>(output.start.x);
        let Ok(y) = wrapped::<_, i32>(output.start.y);
        (
          x.wrapping_shl(4).wrapping_add(cmv_x),
          y.wrapping_shl(4).wrapping_add(cmv_y),
        )
      }
      _ => {
        self.strip.scaled_blocks = self.strip.scaled_blocks.wrapping_add(1);
        let Ok(x) = scaled_motion(
          output.start.x,
          output.subsampling.x,
          self::ScaledMotionArguments {
            scale: source.x_scale,
            vector: cmv_x,
          },
        );
        let Ok(y) = scaled_motion(
          output.start.y,
          output.subsampling.y,
          self::ScaledMotionArguments {
            scale: source.y_scale,
            vector: cmv_y,
          },
        );
        (x, y)
      }
    };

    let frame = source.frame;
    let Ok(width) = wrapped::<_, i32>(frame.info.size.width);
    let Ok(height) = wrapped::<_, i32>(frame.info.size.height);
    let Subsampling { x: sx, y: sy } = output.subsampling;
    let plane = RefPlane {
      frame,
      plane: output.plane,
      last_x: width
        .wrapping_add(i32::from(sx != 0))
        .wrapping_shr(sx)
        .wrapping_sub(1),
      last_y: height
        .wrapping_add(i32::from(sy != 0))
        .wrapping_shr(sy)
        .wrapping_sub(1),
    };
    let motion = Motion {
      x,
      y,
      step_x: source.x_step,
      step_y: source.y_step,
      width: output.width,
      height: output.height,
      filter: block.interp_filter,
      bit_depth: self.bit_depth,
    };

    inter::predict(
      &plane,
      motion,
      destination,
      &mut self.mc_tmp,
      &mut self.mc_win,
    )
  }

  fn blend_prediction(
    &mut self,
    block: InterBlock,
    predictors: PredictionSlot,
  ) -> Result<(), Never> {
    let Ok((stride, band_start)) = strip_layout(&self.strip, block.plane);
    let Ok(at) = index(block.plane);
    let destination = match self.strip.planes.get_mut(at) {
      Some(samples) => samples,
      None => return Ok(()),
    };
    let mut offset = block
      .start
      .y
      .wrapping_mul(stride)
      .wrapping_add(block.start.x)
      .wrapping_sub(band_start);

    for y in 0..block.height {
      let start = y.wrapping_mul(block.width);
      let end = start.wrapping_add(block.width);
      let Ok(first) = span(&self.pred, start..end);
      let Ok(output) = span_mut(destination, offset..offset.wrapping_add(block.width));
      offset = offset.wrapping_add(stride);

      match predictors {
        PredictionSlot::First => {
          for (sample, &prediction) in output.iter_mut().zip(first) {
            *sample = prediction;
          }
        }
        PredictionSlot::Second => {
          let Ok(second) = span(&self.pred2, start..end);

          for ((sample, &a), &b) in output.iter_mut().zip(first).zip(second) {
            *sample = u16::midpoint(a, b).wrapping_add((a ^ b) & 1);
          }
        }
      }
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
struct InterBlock {
  plane: u64,
  subsampling: Subsampling,
  start: Place,
  width: u64,
  height: u64,
  block_index: u64,
}

#[derive(Clone, Copy)]
enum PredictionSlot {
  First,
  Second,
}

fn prediction_vector(
  size: u8,
  subsampling: Subsampling,
  vectors: &[Mv; 4],
  position: u64,
) -> Result<Mv, Never> {
  let Ok(at) = index(position);
  let first = match vectors.get(at) {
    Some(&mv) => mv,
    None => Mv::ZERO,
  };

  match (size, subsampling.x, subsampling.y) {
    (BLOCK_8X8.., _, _) | (_, 0, 0) => Ok(first),
    (_, 0, 1) => average_pair(vectors, first, position.wrapping_add(2)),
    (_, 1, 0) => average_pair(vectors, first, position.wrapping_add(1)),
    _ => {
      let mut total = WideMv { row: 0, col: 0 };

      for vector in vectors {
        let Ok(wide) = vector.wide();
        total.row = total.row.wrapping_add(wide.row);
        total.col = total.col.wrapping_add(wide.col);
      }

      let Ok(row) = round_q4(total.row);
      let Ok(col) = round_q4(total.col);
      Mv::narrowed(WideMv { row, col })
    }
  }
}

fn average_pair(vectors: &[Mv; 4], first: Mv, place: u64) -> Result<Mv, Never> {
  let Ok(at) = index(place);
  let second = match vectors.get(at) {
    Some(&mv) => mv,
    None => Mv::ZERO,
  };
  let Ok(a) = first.wide();
  let Ok(b) = second.wide();
  let Ok(row) = round_q2(a.row.wrapping_add(b.row));
  let Ok(col) = round_q2(a.col.wrapping_add(b.col));
  Mv::narrowed(WideMv { row, col })
}

fn clamped_motion(
  component: i32,
  ClampedMotionArguments {
    block_origin,
    frame_extent,
  }: ClampedMotionArguments,
  block_extent: u8,
  shift: u32,
) -> Result<i32, Never> {
  let Ok(origin) = wrapped::<_, i32>(block_origin);
  let Ok(extent) = wrapped::<_, i32>(frame_extent);
  let width = i32::from(block_extent);
  let first = origin.wrapping_mul(128).wrapping_neg().wrapping_shr(shift);
  let last = extent
    .wrapping_sub(width)
    .wrapping_sub(origin)
    .wrapping_mul(128)
    .wrapping_shr(shift);
  let border = INTERP_EXTEND
    .wrapping_add(width.wrapping_mul(8).wrapping_shr(shift))
    .wrapping_shl(4);
  let low = first.wrapping_sub(border);
  let high = last.wrapping_add(border.wrapping_sub(16)).max(low);
  Ok(
    component
      .wrapping_mul(2)
      .wrapping_shr(shift)
      .clamp(low, high),
  )
}

struct ClampedMotionArguments {
  block_origin: u64,
  frame_extent: u64,
}

fn scaled_motion(
  origin: u64,
  subsampling: u32,
  ScaledMotionArguments { scale, vector }: ScaledMotionArguments,
) -> Result<i32, Never> {
  let Ok(start) = wrapped::<_, i64>(origin);
  let factor = i64::from(scale);
  let base = start.wrapping_mul(factor).wrapping_shr(14);
  let luma = start.wrapping_shl(subsampling);
  let fraction = luma.wrapping_mul(16).wrapping_mul(factor).wrapping_shr(14) & 15;
  let offset = i64::from(vector).wrapping_mul(factor).wrapping_shr(14);
  wrapped(
    base
      .wrapping_shl(4)
      .wrapping_add(offset)
      .wrapping_add(fraction),
  )
}

struct ScaledMotionArguments {
  scale: i32,
  vector: i32,
}

struct TransformSampling {
  subsampling: Subsampling,
  transform_size: u8,
}

struct TransformPlane {
  plane: u64,
  subsampling: Subsampling,
  width_units: u64,
  height_units: u64,
  step: u64,
  size: u8,
  base: Place,
  max_x: u64,
  max_y: u64,
}

struct ResidualPosition {
  row: u64,
  column: u64,
  origin: Place,
  index: u64,
}

struct CoefficientBlock {
  plane: u64,
  origin: Place,
  transform: Transform,
  x_limit: u64,
  y_limit: u64,
}

struct CoefficientValue {
  position: u64,
  token: u8,
  magnitude: i64,
  quantizer: i32,
  shift: u32,
}

struct Reconstructed {
  end: u64,
  rows: u64,
}

enum EndCheck {
  Read,
  Skip,
}

struct CoefficientContext {
  size: u8,
  plane_type: u64,
  reference_type: u64,
  band: u8,
  context: u8,
}

impl CoefficientContext {
  fn position(self) -> Result<u64, Never> {
    Ok(
      u64::from(self.size)
        .wrapping_mul(2)
        .wrapping_add(self.plane_type)
        .wrapping_mul(2)
        .wrapping_add(self.reference_type)
        .wrapping_mul(6)
        .wrapping_add(u64::from(self.band))
        .wrapping_mul(6)
        .wrapping_add(u64::from(self.context)),
    )
  }
}

fn strip_layout(strip: &super::Strip, plane: u64) -> Result<(u64, u64), Never> {
  let Ok(stride) = read(&strip.strides, plane);
  let Ok(origin) = read(&strip.x_off, plane);
  Ok((stride, origin))
}

fn intra_mode(block: &Blk, index: u64) -> Result<u8, Never> {
  match block.size {
    ..=2 => read(&block.sub_modes, index),
    _ => Ok(block.y_mode),
  }
}

const LARGE_COEFFICIENT_TREE: [i8; 16] =
  [2, 6, -2, 4, -3, -4, 8, 10, -5, -6, 12, 14, -7, -8, -9, -10];

fn read_coefficient(
  bd: &mut BoolDecoder,
  ReadCoefficientArguments { probability, depth }: ReadCoefficientArguments,
) -> Result<(u8, i64), Never> {
  let Ok(probabilities) = pareto(probability);
  let Ok(token) = bd.read_tree(&LARGE_COEFFICIENT_TREE, &probabilities);
  let Ok(token_index) = index(token);
  let (category, extra, base) = match EXTRA_BITS.get(token_index) {
    Some(&entry) => entry,
    None => (0, 0, 0),
  };
  let mut magnitude = i64::from(base);

  match token {
    10 => {
      for bit in 0..u32::from(depth.saturating_sub(8)) {
        let Ok(value) = bd.read_bool(255);
        let shift = 5u32.wrapping_add(u32::from(depth)).wrapping_sub(bit);
        magnitude = magnitude.wrapping_add(i64::from(value).wrapping_shl(shift));
      }
    }
    _ => {}
  }

  let Ok(slot) = index(category);
  let probabilities_2 = match CAT_PROBS.get(slot) {
    Some(&category_probabilities) => category_probabilities,
    None => &[],
  };
  let Ok(length) = index(extra);

  for (bit, &probability_2) in (0..u32::from(extra))
    .rev()
    .zip(probabilities_2.iter().take(length))
  {
    let Ok(value) = bd.read_bool(probability_2);
    magnitude = magnitude.wrapping_add(i64::from(value).wrapping_shl(bit));
  }

  Ok((token, magnitude))
}

struct ReadCoefficientArguments {
  probability: u8,
  depth: u8,
}
