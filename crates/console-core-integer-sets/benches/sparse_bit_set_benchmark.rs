//! Sparse-set decoding is measured against every supported branch factor.

mod bench_helper;

use bench_helper::parameters;
use console_core_integer_sets::{
  IntSet,
  sparse_bit_set::{EncodingError, to_sparse_bit_set_with_bf},
};
use criterion::{BenchmarkId, Criterion};
use std::hint::black_box;

enum Factor {
  Two,
  Four,
  Eight,
  ThirtyTwo,
}

impl Factor {
  fn encode(&self, set: &IntSet<u32>) -> Result<Vec<u8>, EncodingError> {
    match self {
      Self::Two => to_sparse_bit_set_with_bf::<2>(set),
      Self::Four => to_sparse_bit_set_with_bf::<4>(set),
      Self::Eight => to_sparse_bit_set_with_bf::<8>(set),
      Self::ThirtyTwo => to_sparse_bit_set_with_bf::<32>(set),
    }
  }
}

impl core::fmt::Display for Factor {
  fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    output.write_str(match self {
      Self::Two => "2",
      Self::Four => "4",
      Self::Eight => "8",
      Self::ThirtyTwo => "32",
    })
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut criterion = Criterion::default().configure_from_args();

  for factor in [Factor::Two, Factor::Four, Factor::Eight, Factor::ThirtyTwo] {
    let Ok(inputs) = parameters();

    for sample in inputs.filter(|sample| sample.count <= 32_768 && sample.density <= 8) {
      let values = sample.values()?;
      let set: IntSet<u32> = values.collect();
      let encoded = factor.encode(&set)?;

      criterion.bench_with_input(
        BenchmarkId::new("BM_SparseSetDecode", format!("bf{factor}_{sample}")),
        &encoded,
        |bench, encoded_2| {
          bench.iter(|| black_box(IntSet::<u32>::from_sparse_bit_set(encoded_2)));
        },
      );
    }
  }

  criterion.final_summary();

  Ok(())
}
