//! Point updates, ordered extension, lookup and iteration across sparse densities.

mod bench_helper;

use bench_helper::parameters;
use console_core_integer_sets::{IntSet, Presence, U32Set};
use console_core_never::Never;
use criterion::{BatchSize, BenchmarkId, Criterion};
use std::hint::black_box;
use std::num::NonZeroU32;
use std::time::Instant;

fn update_benchmarks(criterion: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
  let Ok(inputs) = parameters();

  for sample in inputs {
    let limit = sample.limit()?;
    let values = sample.values()?;
    let set: IntSet<u32> = values.collect();

    criterion.bench_with_input(
      BenchmarkId::new("BM_SetInsert_1000", sample),
      &set,
      |bench, initial| {
        bench.iter_batched(
          || initial.clone(),
          |mut set_2| {
            for index in 0..1000u32 {
              let value = index.wrapping_mul(2_654_435_761) % limit;
              let Ok(_inserted) = set_2.insert(value);
            }
          },
          BatchSize::SmallInput,
        );
      },
    );
    criterion.bench_with_input(
      BenchmarkId::new("BM_SetOrderedInsert_1000", sample),
      &set,
      |bench, initial| {
        bench.iter_batched(
          || initial.clone(),
          |mut set_3| set_3.extend(0..1000),
          BatchSize::SmallInput,
        );
      },
    );
  }

  Ok(())
}

enum Lookup {
  Random,
  Ordered,
}

fn lookup_benchmark(
  criterion: &mut Criterion,
  query: impl Fn(u32) -> Result<Presence, Never>,
  limit: NonZeroU32,
  label: BenchmarkId,
  order: Lookup,
) -> Result<(), Never> {
  let step = match order {
    Lookup::Random => 12345,
    Lookup::Ordered => 3,
  };

  criterion.bench_with_input(label, &(), |bench, _unit| {
    bench.iter_custom(|iterations| {
      let mut needle = limit.get() / NonZeroU32::MIN.saturating_add(1);
      let start = Instant::now();

      for _iteration in 0..iterations {
        needle = needle.wrapping_add(step);
        let Ok(found) = query(needle % limit);

        black_box(found);
      }

      start.elapsed()
    });
  });

  Ok(())
}

fn read_benchmarks(criterion: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
  let Ok(inputs) = parameters();

  for sample in inputs {
    let limit = sample.limit()?;
    let values = sample.values()?;
    let set: IntSet<u32> = values.collect();
    let values_2 = sample.values()?;
    let pages: U32Set = values_2.collect();
    let Ok(()) = lookup_benchmark(
      criterion,
      |value| set.contains(value),
      limit,
      BenchmarkId::new("BM_SetLookup/random", sample),
      Lookup::Random,
    );
    let Ok(()) = lookup_benchmark(
      criterion,
      |value| set.contains(value),
      limit,
      BenchmarkId::new("BM_SetLookup/ordered", sample),
      Lookup::Ordered,
    );
    let Ok(()) = lookup_benchmark(
      criterion,
      |value| pages.contains(value),
      limit,
      BenchmarkId::new("BM_SetLookup/ordered_u32", sample),
      Lookup::Ordered,
    );

    criterion.bench_with_input(
      BenchmarkId::new("BM_SetIteration", sample),
      &set,
      |bench, set_2| {
        bench.iter(|| {
          let Ok(values_3) = set_2.iter();

          for value in values_3 {
            black_box(value);
          }
        });
      },
    );
  }

  Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut criterion = Criterion::default().configure_from_args();

  update_benchmarks(&mut criterion)?;
  read_benchmarks(&mut criterion)?;
  criterion.final_summary();

  Ok(())
}
