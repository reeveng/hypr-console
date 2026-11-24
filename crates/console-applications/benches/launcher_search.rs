use std::hint::black_box;

use console_applications::narrow;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "Criterion invokes this callback with a unit return type"
  )
)]
fn launcher_search(criterion: &mut Criterion) {
  let mut group = criterion.benchmark_group("launcher_search");
  let names = [
    "Firefox",
    "Files",
    "Photo Library",
    "Music Player",
    "Profile Manager",
    "Settings",
    "Calculator",
    "Terminal",
  ];

  for count in [64_u64, 512] {
    let applications: Vec<_> = (0..count)
      .flat_map(|_| names)
      .zip(0..count)
      .map(|(name, index)| format!("{name} {index}"))
      .collect();
    group.throughput(Throughput::Elements(count));
    group.bench_with_input(
      BenchmarkId::new("typed_fi", count),
      &applications,
      |bencher, applications_2| {
        bencher.iter(|| {
          let Ok(matches) = narrow::matching(black_box(applications_2), black_box("fi"));
          matches
        });
      },
    );
  }

  group.finish();
}

criterion_group!(benches, launcher_search);
criterion_main!(benches);
