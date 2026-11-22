// run-rustfix
// rustfix-only-machine-applicable
// edition:2024
#![warn(explicit014_no_index_slice)]

struct Reading {
    value: u32,
    label: String,
}

struct Station {
    readings: Vec<Reading>,
}

impl Station {
    fn total(&self) -> u32 {
        let mut total = 0;
        for i in 0..self.readings.len() {
            total += self.readings[i].value;
            total += self.readings[i].label.len() as u32;
        }
        total
    }

    fn reset(&mut self) {
        for i in 0..self.readings.len() {
            self.readings[i].value = if i == 0 { 100 } else { 0 };
        }
    }

    fn resize(&mut self) {
        for i in 0..self.readings.len() {
            self.readings[i].value = 0;
            self.readings.push(Reading { value: 1, label: String::new() });
        }
    }
}

fn main() {
    let mut station = Station { readings: vec![Reading { value: 2, label: String::from("first") }] };
    assert_eq!(station.total(), 7);
    station.reset();
    assert_eq!(station.total(), 105);
    station.resize();
    assert_eq!(station.total(), 6);
}
