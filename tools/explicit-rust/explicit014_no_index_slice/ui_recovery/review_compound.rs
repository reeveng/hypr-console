// edition:2024

struct Counter(u8);

impl core::ops::AddAssign<u8> for Counter {
    fn add_assign(&mut self, step: u8) {
        self.0 = self.0.wrapping_add(step);
    }
}

fn main() {
    let mut counters = [Counter(4)];
    counters[0] += counters[0].0;

    let counters = core::array::from_fn(|_| Counter(4));
    let _first = counters[0].0;
    let _: [Counter; 2] = counters;
}
