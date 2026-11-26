//! Keep a bounded window of individual timings for tail latency.

use std::collections::VecDeque;
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

const WINDOW: u64 = 16_384;

#[derive(Default)]
pub struct Samples {
  elapsed: VecDeque<Duration>,
}

impl Samples {
  pub fn add(&mut self, sample: Duration) -> Result<(), Never> {
    let Ok(length) = fixed_index(self.elapsed.len());

    match (length >= WINDOW).then_some(()) {
      Some(()) => {
        let _oldest = self.elapsed.pop_front();
      }
      None => {}
    }

    self.elapsed.push_back(sample);

    Ok(())
  }

  pub fn report(mut self) -> Result<(), Never> {
    let elapsed = self.elapsed.make_contiguous();
    elapsed.sort_unstable();

    let Ok(length) = fixed_index(elapsed.len());
    let last = length.saturating_sub(1);
    let Ok(mid) = index(last.wrapping_div(2));
    let Ok(high) = index(last.saturating_mul(99).wrapping_div(100));

    match (elapsed.get(mid), elapsed.get(high)) {
      (Some(median), Some(tail)) => {
        eprintln!("individual latency samples={length}, p50={median:?}, p99={tail:?}");
      }
      (None, _) | (_, None) => {}
    }

    Ok(())
  }
}
