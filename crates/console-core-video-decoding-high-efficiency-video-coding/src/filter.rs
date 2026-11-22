//! In-loop filters and row publication: deblocking (8.7.2) and sample adaptive offset (8.7.3), run
//! row by row behind the CTU decoding so finished CTB rows can be published to later pictures early.
//!
//! Row r is deblocked once rows 0..=r+1 are decoded (row r+1 still needs the unfiltered bottom samples
//! of row r for intra prediction); SAO of row r runs once row r+1 is deblocked; then row r is final.

use crate::picture::{ColMv, FrameRow};
use crate::places::{read, write};
use crate::slicedec::{F_CODED, F_INTRA, PicState, ReferencePrediction, SliceInfo};
use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::index;

mod deblock;
mod sao;

pub fn ctb_decoded(picture: &mut PicState, band: u64) -> Result<(), Never> {
  let Ok(at) = index(band);

  match picture.row_done.get_mut(at) {
    Some(done) => *done = done.wrapping_add(1),
    None => return Ok(()),
  }

  let height = picture.hctb;
  let width = picture.wctb;

  for row in picture.rows_complete..height {
    let Ok(done) = read(&picture.row_done, row);

    match (u64::from(done) == width).then_some(()) {
      Some(()) => picture.rows_complete = row.wrapping_add(1),
      None => break,
    }
  }

  advance(picture)
}

fn advance(picture: &mut PicState) -> Result<(), Never> {
  let height = picture.hctb;

  for row in picture.rows_deblocked..height {
    match (picture.rows_complete >= row.wrapping_add(2).min(height)).then_some(()) {
      Some(()) => {
        let Ok(()) = deblock::deblock(picture, row);
        picture.rows_deblocked = row.wrapping_add(1);
      }
      None => break,
    }
  }

  for row in picture.rows_published..height {
    match (picture.rows_deblocked >= row.wrapping_add(2).min(height)).then_some(()) {
      Some(()) => {
        let Ok(()) = publish_row(picture, row);
        picture.rows_published = row.wrapping_add(1);
      }
      None => break,
    }
  }

  Ok(())
}

pub fn finish(picture: &mut PicState) -> Result<(), Never> {
  let height = picture.hctb;
  picture.rows_complete = height;
  advance(picture)
}

fn slice_at(picture: &PicState, ctb: u64) -> Result<Option<&SliceInfo>, Never> {
  let Ok(slot) = index(ctb);
  let id = match picture.ctb_slice.get(slot) {
    Some(Some(id)) => *id,
    Some(None) | None => return Ok(None),
  };
  let Ok(at) = index(id);
  Ok(picture.slices.get(at))
}

fn publish_row(picture: &mut PicState, band: u64) -> Result<(), Never> {
  let side = 1u64.wrapping_shl(picture.log2_ctb);
  let first = band.wrapping_mul(side);
  let height = picture.height;
  let last = first.wrapping_add(side).min(height);
  let width = picture.width;
  let width16 = width.div_ceil(16);
  let rows = last.saturating_sub(first).div_ceil(16);
  let Ok(capacity) = index(width16.wrapping_mul(rows));
  let mut motion = vec![ColMv::default(); capacity.min(4224)];

  for y in 0..rows {
    for x in 0..width16 {
      let position = Point {
        x: x.wrapping_mul(16),
        y: first.wrapping_add(y.wrapping_mul(16)),
      };
      let Ok(vector) = temporal_motion(picture, position);
      let Ok(()) = write(&mut motion, y.wrapping_mul(width16).wrapping_add(x), vector);
    }
  }

  let Ok([y, cb, cr]) = sao::filter_row(picture, band);
  let row = FrameRow {
    y: y.into_boxed_slice(),
    cb: cb.into_boxed_slice(),
    cr: cr.into_boxed_slice(),
    col: motion.into_boxed_slice(),
  };
  picture.frame.publish(band, row)
}

fn temporal_motion(picture: &PicState, position: Point<u64>) -> Result<ColMv, Never> {
  let blocks = picture.w4;
  let block_index = position
    .y
    .wrapping_shr(2)
    .wrapping_mul(blocks)
    .wrapping_add(position.x.wrapping_shr(2));
  let Ok(block) = read(&picture.blk, block_index);

  match (block.flags & F_CODED == 0 || block.flags & F_INTRA != 0).then_some(()) {
    Some(()) => return Ok(ColMv::default()),
    None => {}
  }

  let width = picture.wctb;
  let shift = picture.log2_ctb;
  let ctb = position
    .y
    .wrapping_shr(shift)
    .wrapping_mul(width)
    .wrapping_add(position.x.wrapping_shr(shift));
  let Ok(slice) = slice_at(picture, ctb);
  let slice_3 = match slice {
    Some(slice_2) => slice_2,
    None => return Ok(ColMv::default()),
  };
  let Ok(field) = read(&picture.mvf, block_index);
  let mut out = ColMv {
    mv: field.mv,
    ref_poc: [0; 2],
    flags: 0,
  };

  let columns = field.ref_idx.into_iter().zip(&mut out.ref_poc);
  let lists = slice_3.ref_pocs.iter().zip(&slice_3.ref_lt);

  for (bit, ((slot, poc), (pocs, long_term))) in (0u32..).zip(columns.zip(lists)) {
    let Ok(id) = console_core_number_conversion::wrapped_index(slot);

    let Ok(prediction) = field.prediction(crate::mvpred::MotionList::from(u64::from(bit)));

    match (prediction, pocs.get(id)) {
      (ReferencePrediction::Used, Some(&value)) => {
        *poc = value;
        out.flags |= 1u8.wrapping_shl(bit);

        match long_term.get(id) {
          Some(true) => out.flags |= 4u8.wrapping_shl(bit),
          Some(false) | None => {}
        }
      }
      _ => {}
    }
  }

  Ok(out)
}
