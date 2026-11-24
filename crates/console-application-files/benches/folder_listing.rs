use std::hint::black_box;

use console_application_files::listing::{self, Entry};
use console_core_never::Never;
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};

fn folder_listing(criterion: &mut Criterion) -> Result<(), Never> {
  let mut group = criterion.benchmark_group("folder_listing");

  for count in [128_u64, 4096] {
    let entries: Vec<_> = (0..count)
      .filter_map(|index| {
        let number = index.wrapping_mul(53).checked_rem(count)?;

        match { ::core::primitive::bool::then_some(number.is_multiple_of(8), ()) } {
          ::core::option::Option::Some(()) => {
            let Ok(folder) = Entry::folder(&format!("Album {number:04}"));
            Some(folder)
          }
          ::core::option::Option::None => {
            let Ok(file) = Entry::file(
              &format!("Photo {number:04}.jpg"),
              920_000_u64.wrapping_add(number),
            );
            Some(file)
          }
        }
      })
      .collect();
    group.throughput(Throughput::Elements(count));
    group.bench_with_input(
      BenchmarkId::new("sort_entries", count),
      &entries,
      |bencher, entries_2| {
        bencher.iter_batched(
          || entries_2.clone(),
          |input| {
            let Ok(sorted) = listing::sorted(black_box(input));
            sorted
          },
          BatchSize::LargeInput,
        );
      },
    );
  }

  group.finish();

  Ok(())
}

fn main() {
  let mut criterion = Criterion::default().configure_from_args();
  let Ok(()) = folder_listing(&mut criterion);
  criterion.final_summary();
}
