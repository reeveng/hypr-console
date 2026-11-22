//! Probability tables (a "frame context"), syntax element counters and the backward adaptation
//! processes of section 8.4.
//!
//! The specification merges the probabilities of a tree by recursion from its root. A child in
//! every tree it describes is written after its parent, so walking the nodes from the last back
//! to the first meets every child before its parent, and the subtotals the recursion handed up
//! are kept in a list instead.

use std::num::NonZeroU32;
use std::num::NonZeroU64;

use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index};

use crate::header::{FrameHeader, Precision};
use crate::places::{read, write};
use crate::tables::*;

pub type CoefProbs = [[[[[[u8; 3]; 6]; 6]; 2]; 2]; 4];

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrameContext {
  pub tx: [[[u8; 3]; 2]; 4],
  pub coef: CoefProbs,
  pub skip: [u8; 3],
  pub inter_mode: [[u8; 3]; 7],
  pub interp_filter: [[u8; 2]; 4],
  pub is_inter: [u8; 4],
  pub comp_mode: [u8; 5],
  pub single_ref: [[u8; 2]; 5],
  pub comp_ref: [u8; 5],
  pub y_mode: [[u8; 9]; 4],
  pub uv_mode: [[u8; 9]; 10],
  pub partition: [[u8; 3]; 16],
  pub mv_joint: [u8; 3],
  pub mv_sign: [u8; 2],
  pub mv_class: [[u8; 10]; 2],
  pub mv_class0_bit: [u8; 2],
  pub mv_bits: [[u8; 10]; 2],
  pub mv_class0_fr: [[[u8; 3]; 2]; 2],
  pub mv_fr: [[u8; 3]; 2],
  pub mv_class0_hp: [u8; 2],
  pub mv_hp: [u8; 2],
}

fn loaded(into: &mut [u8], from: &[u8]) -> Result<(), Never> {
  for (probability, written) in into.iter_mut().zip(from) {
    *probability = *written;
  }

  Ok(())
}

impl Default for FrameContext {
  fn default() -> Self {
    let mut context = FrameContext {
      tx: [[[128; 3]; 2]; 4],
      coef: [[[[[[128; 3]; 6]; 6]; 2]; 2]; 4],
      skip: DEFAULT_SKIP_PROB,
      inter_mode: [[128; 3]; 7],
      interp_filter: [[128; 2]; 4],
      is_inter: DEFAULT_IS_INTER_PROB,
      comp_mode: DEFAULT_COMP_MODE_PROB,
      single_ref: [[128; 2]; 5],
      comp_ref: DEFAULT_COMP_REF_PROB,
      y_mode: [[128; 9]; 4],
      uv_mode: [[128; 9]; 10],
      partition: [[128; 3]; 16],
      mv_joint: DEFAULT_MV_JOINT_PROBS,
      mv_sign: DEFAULT_MV_SIGN_PROB,
      mv_class: [[128; 10]; 2],
      mv_class0_bit: DEFAULT_MV_CLASS0_BIT_PROB,
      mv_bits: [[128; 10]; 2],
      mv_class0_fr: [[[128; 3]; 2]; 2],
      mv_fr: [[128; 3]; 2],
      mv_class0_hp: DEFAULT_MV_CLASS0_HP_PROB,
      mv_hp: DEFAULT_MV_HP_PROB,
    };

    let Ok(()) = loaded(
      context
        .coef
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut(),
      &DEFAULT_COEF_PROBS,
    );
    let Ok(()) = loaded(
      context.tx.as_flattened_mut().as_flattened_mut(),
      &DEFAULT_TX_PROBS,
    );
    let Ok(()) = loaded(
      context.inter_mode.as_flattened_mut(),
      &DEFAULT_INTER_MODE_PROBS,
    );
    let Ok(()) = loaded(
      context.interp_filter.as_flattened_mut(),
      &DEFAULT_INTERP_FILTER_PROBS,
    );
    let Ok(()) = loaded(
      context.single_ref.as_flattened_mut(),
      &DEFAULT_SINGLE_REF_PROB,
    );
    let Ok(()) = loaded(context.y_mode.as_flattened_mut(), &DEFAULT_Y_MODE_PROBS);
    let Ok(()) = loaded(context.uv_mode.as_flattened_mut(), &DEFAULT_UV_MODE_PROBS);
    let Ok(()) = loaded(
      context.partition.as_flattened_mut(),
      &DEFAULT_PARTITION_PROBS,
    );
    let Ok(()) = loaded(context.mv_class.as_flattened_mut(), &DEFAULT_MV_CLASS_PROBS);
    let Ok(()) = loaded(context.mv_bits.as_flattened_mut(), &DEFAULT_MV_BITS_PROB);
    let Ok(()) = loaded(
      context.mv_class0_fr.as_flattened_mut().as_flattened_mut(),
      &DEFAULT_MV_CLASS0_FR_PROBS,
    );
    let Ok(()) = loaded(context.mv_fr.as_flattened_mut(), &DEFAULT_MV_FR_PROBS);

    context
  }
}

#[derive(Clone, Default)]
pub struct Counts {
  pub intra_mode: [[u32; 10]; 4],
  pub uv_mode: [[u32; 10]; 10],
  pub partition: [[u32; 4]; 16],
  pub interp_filter: [[u32; 3]; 4],
  pub inter_mode: [[u32; 4]; 7],
  pub tx: [[[u32; 4]; 2]; 4],
  pub is_inter: [[u32; 2]; 4],
  pub comp_mode: [[u32; 2]; 5],
  pub single_ref: [[[u32; 2]; 2]; 5],
  pub comp_ref: [[u32; 2]; 5],
  pub skip: [[u32; 2]; 3],
  pub mv_joint: [u32; 4],
  pub mv_sign: [[u32; 2]; 2],
  pub mv_class: [[u32; 11]; 2],
  pub mv_class0_bit: [[u32; 2]; 2],
  pub mv_class0_fr: [[[u32; 4]; 2]; 2],
  pub mv_class0_hp: [[u32; 2]; 2],
  pub mv_bits: [[[u32; 2]; 10]; 2],
  pub mv_fr: [[u32; 4]; 2],
  pub mv_hp: [[u32; 2]; 2],
  pub token: [[[[[[u32; 3]; 6]; 6]; 2]; 2]; 4],
  pub more_coefs: [[[[[[u32; 2]; 6]; 6]; 2]; 2]; 4],
}

fn summed(into: &mut [u32], from: &[u32]) -> Result<(), Never> {
  for (count, other) in into.iter_mut().zip(from) {
    *count = count.wrapping_add(*other);
  }

  Ok(())
}

impl Counts {
  pub fn add(&mut self, other: &Counts) -> Result<(), Never> {
    let Ok(()) = summed(
      self.intra_mode.as_flattened_mut(),
      other.intra_mode.as_flattened(),
    );
    let Ok(()) = summed(
      self.uv_mode.as_flattened_mut(),
      other.uv_mode.as_flattened(),
    );
    let Ok(()) = summed(
      self.partition.as_flattened_mut(),
      other.partition.as_flattened(),
    );
    let Ok(()) = summed(
      self.interp_filter.as_flattened_mut(),
      other.interp_filter.as_flattened(),
    );
    let Ok(()) = summed(
      self.inter_mode.as_flattened_mut(),
      other.inter_mode.as_flattened(),
    );
    let Ok(()) = summed(
      self.tx.as_flattened_mut().as_flattened_mut(),
      other.tx.as_flattened().as_flattened(),
    );
    let Ok(()) = summed(
      self.is_inter.as_flattened_mut(),
      other.is_inter.as_flattened(),
    );
    let Ok(()) = summed(
      self.comp_mode.as_flattened_mut(),
      other.comp_mode.as_flattened(),
    );
    let Ok(()) = summed(
      self.single_ref.as_flattened_mut().as_flattened_mut(),
      other.single_ref.as_flattened().as_flattened(),
    );
    let Ok(()) = summed(
      self.comp_ref.as_flattened_mut(),
      other.comp_ref.as_flattened(),
    );
    let Ok(()) = summed(self.skip.as_flattened_mut(), other.skip.as_flattened());
    let Ok(()) = self.add_motion(other);
    self.add_coefficients(other)
  }

  fn add_motion(&mut self, other: &Counts) -> Result<(), Never> {
    let Ok(()) = summed(&mut self.mv_joint, &other.mv_joint);
    let Ok(()) = summed(
      self.mv_sign.as_flattened_mut(),
      other.mv_sign.as_flattened(),
    );
    let Ok(()) = summed(
      self.mv_class.as_flattened_mut(),
      other.mv_class.as_flattened(),
    );
    let Ok(()) = summed(
      self.mv_class0_bit.as_flattened_mut(),
      other.mv_class0_bit.as_flattened(),
    );
    let Ok(()) = summed(
      self.mv_class0_fr.as_flattened_mut().as_flattened_mut(),
      other.mv_class0_fr.as_flattened().as_flattened(),
    );
    let Ok(()) = summed(
      self.mv_class0_hp.as_flattened_mut(),
      other.mv_class0_hp.as_flattened(),
    );
    let Ok(()) = summed(
      self.mv_bits.as_flattened_mut().as_flattened_mut(),
      other.mv_bits.as_flattened().as_flattened(),
    );
    let Ok(()) = summed(self.mv_fr.as_flattened_mut(), other.mv_fr.as_flattened());
    let Ok(()) = summed(self.mv_hp.as_flattened_mut(), other.mv_hp.as_flattened());

    Ok(())
  }

  fn add_coefficients(&mut self, other: &Counts) -> Result<(), Never> {
    let Ok(()) = summed(
      self
        .token
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut(),
      other
        .token
        .as_flattened()
        .as_flattened()
        .as_flattened()
        .as_flattened()
        .as_flattened(),
    );

    summed(
      self
        .more_coefs
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut()
        .as_flattened_mut(),
      other
        .more_coefs
        .as_flattened()
        .as_flattened()
        .as_flattened()
        .as_flattened()
        .as_flattened(),
    )
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Update {
  pub saturation: NonZeroU32,
  pub factor: u32,
}

const SATURATION: NonZeroU32 = match NonZeroU32::new(COUNT_SAT) {
  Some(count) => count,
  None => NonZeroU32::MIN,
};

const COEFFICIENT_SATURATION: NonZeroU32 = match NonZeroU32::new(24) {
  Some(count) => count,
  None => NonZeroU32::MIN,
};

pub const ORDINARY: Update = Update {
  saturation: SATURATION,
  factor: MAX_UPDATE_FACTOR,
};

#[derive(Clone, Copy, Debug)]
pub struct Tally {
  pub zeros: u32,
  pub ones: u32,
}

pub fn merge_prob(previous: u8, tally: Tally, update: Update) -> Result<u8, Never> {
  let total = tally.zeros.wrapping_add(tally.ones);

  let observed = match NonZeroU64::new(u64::from(total)) {
    Some(denominator) => (u64::from(tally.zeros)
      .wrapping_mul(256)
      .wrapping_add(u64::from(total).wrapping_shr(1))
      / denominator)
      .clamp(1, 255),
    None => 128,
  };

  let count = total.min(update.saturation.get());
  let factor = update.factor.wrapping_mul(count) / update.saturation;
  let blended = u64::from(previous)
    .wrapping_mul(u64::from(256_u32.wrapping_sub(factor)))
    .wrapping_add(observed.wrapping_mul(u64::from(factor)))
    .wrapping_add(128)
    .wrapping_shr(8);

  fitted::<u64, u8>(blended)
}

fn leaf_or_subtotal(node: i8, counts: &[u32], subtotals: &[u32; 32]) -> Result<u32, Never> {
  match ::core::primitive::bool::then_some(node.is_positive(), ()) {
    ::core::option::Option::Some(()) => read(subtotals, u64::from(node.unsigned_abs())),
    ::core::option::Option::None => read(counts, u64::from(node.unsigned_abs())),
  }
}

pub fn merge_probs(
  tree: &[i8],
  start: u64,
  probabilities: &mut [u8],
  counts: &[u32],
  update: Update,
) -> Result<(), Never> {
  let mut subtotals = [0_u32; 32];

  for (pair, [near, far]) in tree.as_chunks::<2>().0.iter().enumerate().rev() {
    let Ok(node) = fixed_index(pair);
    let place = node.wrapping_mul(2);

    match ::core::primitive::bool::then_some(place < start, ()) {
      ::core::option::Option::Some(()) => {}

      ::core::option::Option::None => {
        let Ok(left) = leaf_or_subtotal(*near, counts, &subtotals);
        let Ok(right) = leaf_or_subtotal(*far, counts, &subtotals);
        let Ok(previous) = read(probabilities, node);
        let Ok(merged) = merge_prob(
          previous,
          Tally {
            zeros: left,
            ones: right,
          },
          update,
        );
        let Ok(()) = write(probabilities, node, merged);
        let Ok(()) = write(&mut subtotals, place, left.wrapping_add(right));
      }
    }
  }

  Ok(())
}

fn adapt_probs(tree: &[i8], probabilities: &mut [u8], counts: &[u32]) -> Result<(), Never> {
  merge_probs(tree, 0, probabilities, counts, ORDINARY)
}

fn adapt_prob(probability: &mut u8, counts: &[u32; 2]) -> Result<(), Never> {
  let [zeros, ones] = *counts;
  let Ok(merged) = merge_prob(*probability, Tally { zeros, ones }, ORDINARY);

  *probability = merged;

  Ok(())
}

pub fn adapt_coef_probs(
  context: &mut FrameContext,
  counts: &Counts,
  factor: u32,
) -> Result<(), Never> {
  let update = Update {
    saturation: COEFFICIENT_SATURATION,
    factor,
  };

  let probabilities = context.coef.as_flattened_mut().as_flattened_mut();
  let tokens = counts.token.as_flattened().as_flattened();
  let more = counts.more_coefs.as_flattened().as_flattened();

  for ((bands, tokens_2), more_2) in probabilities.iter_mut().zip(tokens).zip(more) {
    for (band, ((contexts, tokens_3), more_3)) in
      bands.iter_mut().zip(tokens_2).zip(more_2).enumerate()
    {
      let Ok(used) = index(match band {
        0 => 3_u64,
        _later => 6,
      });

      for ((probabilities_2, tokens_4), more_4) in
        contexts.iter_mut().zip(tokens_3).zip(more_3).take(used)
      {
        let Ok(()) = merge_probs(&SMALL_TOKEN_TREE, 2, probabilities_2, tokens_4, update);
        let Ok(()) = merge_probs(&BINARY_TREE, 0, probabilities_2, more_4, update);
      }
    }
  }

  Ok(())
}

fn adapt_rows<const N: usize, const M: usize>(
  tree: &[i8],
  probabilities: &mut [[u8; N]],
  counts: &[[u32; M]],
) -> Result<(), Never> {
  for (row, tally) in probabilities.iter_mut().zip(counts) {
    let Ok(()) = adapt_probs(tree, row, tally);
  }

  Ok(())
}

fn adapt_each(probabilities: &mut [u8], counts: &[[u32; 2]]) -> Result<(), Never> {
  for (probability, tally) in probabilities.iter_mut().zip(counts) {
    let Ok(()) = adapt_prob(probability, tally);
  }

  Ok(())
}

pub fn adapt_noncoef_probs(
  context: &mut FrameContext,
  counts: &Counts,
  header: &FrameHeader,
) -> Result<(), Never> {
  let Ok(()) = adapt_each(&mut context.is_inter, &counts.is_inter);
  let Ok(()) = adapt_each(&mut context.comp_mode, &counts.comp_mode);
  let Ok(()) = adapt_each(&mut context.comp_ref, &counts.comp_ref);
  let Ok(()) = adapt_each(
    context.single_ref.as_flattened_mut(),
    counts.single_ref.as_flattened(),
  );
  let Ok(()) = adapt_rows(
    &INTER_MODE_TREE,
    &mut context.inter_mode,
    &counts.inter_mode,
  );
  let Ok(()) = adapt_rows(&INTRA_MODE_TREE, &mut context.y_mode, &counts.intra_mode);
  let Ok(()) = adapt_rows(&INTRA_MODE_TREE, &mut context.uv_mode, &counts.uv_mode);
  let Ok(()) = adapt_rows(&PARTITION_TREE, &mut context.partition, &counts.partition);
  let Ok(()) = adapt_each(&mut context.skip, &counts.skip);

  match header.interp_filter {
    SWITCHABLE => {
      let Ok(()) = adapt_rows(
        &INTERP_FILTER_TREE,
        &mut context.interp_filter,
        &counts.interp_filter,
      );
    }
    _fixed => {}
  }

  match header.tx_mode {
    TX_MODE_SELECT => {
      let trees: [&[i8]; 4] = [
        &TX_SIZE_8_TREE,
        &TX_SIZE_8_TREE,
        &TX_SIZE_16_TREE,
        &TX_SIZE_32_TREE,
      ];

      for ((probabilities, tally), tree) in
        context.tx.iter_mut().zip(&counts.tx).zip(trees).skip(1)
      {
        let Ok(()) = adapt_rows(tree, probabilities, tally);
      }
    }
    _fixed => {}
  }

  let Ok(()) = adapt_probs(&MV_JOINT_TREE, &mut context.mv_joint, &counts.mv_joint);
  let Ok(()) = adapt_each(&mut context.mv_sign, &counts.mv_sign);
  let Ok(()) = adapt_rows(&MV_CLASS_TREE, &mut context.mv_class, &counts.mv_class);
  let Ok(()) = adapt_each(&mut context.mv_class0_bit, &counts.mv_class0_bit);
  let Ok(()) = adapt_each(
    context.mv_bits.as_flattened_mut(),
    counts.mv_bits.as_flattened(),
  );
  let Ok(()) = adapt_rows(
    &MV_FR_TREE,
    context.mv_class0_fr.as_flattened_mut(),
    counts.mv_class0_fr.as_flattened(),
  );
  let Ok(()) = adapt_rows(&MV_FR_TREE, &mut context.mv_fr, &counts.mv_fr);

  match header.precision {
    Precision::High => {
      let Ok(()) = adapt_each(&mut context.mv_class0_hp, &counts.mv_class0_hp);
      let Ok(()) = adapt_each(&mut context.mv_hp, &counts.mv_hp);
    }
    Precision::Low => {}
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn merge_prob_identity_without_counts() {
    for previous in 1..=255_u8 {
      assert_eq!(
        merge_prob(previous, Tally { zeros: 0, ones: 0 }, ORDINARY),
        Ok(previous)
      );
    }

    assert_eq!(
      merge_prob(128, Tally { zeros: 20, ones: 0 }, ORDINARY),
      Ok(192)
    );
  }

  #[test]
  fn defaults_load() {
    let context = FrameContext::default();

    assert_eq!(
      context
        .coef
        .first()
        .and_then(|t| t.first())
        .and_then(|p| p.first())
        .and_then(|r| r.first())
        .and_then(|b| b.first()),
      Some(&[195, 29, 183])
    );
    assert_eq!(context.tx.get(3).and_then(|t| t.get(1)), Some(&[5, 52, 13]));
    assert_eq!(
      context.mv_class0_fr.get(1).and_then(|c| c.get(1)),
      Some(&[96, 112, 64])
    );
  }

  #[test]
  fn a_tree_merged_from_its_leaves_matches_the_recursion() {
    let counts = [3_u32, 9, 27, 81, 5, 7, 11, 13, 17, 19, 23];
    let mut walked = [128_u8; 10];
    let Ok(()) = merge_probs(&MV_CLASS_TREE, 0, &mut walked, &counts, ORDINARY);
    let mut recursed = [128_u8; 10];
    let Ok(_total) = recursion(&MV_CLASS_TREE, 0, &mut recursed, &counts);

    assert_eq!(walked, recursed);
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    allow(
      explicit049_no_recursion,
      reason = "the specification's own recursive merge, kept in the test as the reference the walk is compared against"
    )
  )]
  fn recursion(
    tree: &[i8],
    node: u64,
    probabilities: &mut [u8],
    counts: &[u32],
  ) -> Result<u32, Never> {
    let Ok(near) = read(tree, node);
    let Ok(far) = read(tree, node.wrapping_add(1));

    let left = match ::core::primitive::bool::then_some(near.is_positive(), ()) {
      ::core::option::Option::Some(()) => {
        recursion(tree, u64::from(near.unsigned_abs()), probabilities, counts)
      }
      ::core::option::Option::None => read(counts, u64::from(near.unsigned_abs())),
    };

    let right = match ::core::primitive::bool::then_some(far.is_positive(), ()) {
      ::core::option::Option::Some(()) => {
        recursion(tree, u64::from(far.unsigned_abs()), probabilities, counts)
      }
      ::core::option::Option::None => read(counts, u64::from(far.unsigned_abs())),
    };

    let (Ok(zeros), Ok(ones)) = (left, right);
    let Ok(previous) = read(probabilities, node.wrapping_shr(1));
    let Ok(merged) = merge_prob(previous, Tally { zeros, ones }, ORDINARY);
    let Ok(()) = write(probabilities, node.wrapping_shr(1), merged);

    Ok(zeros.wrapping_add(ones))
  }
}
