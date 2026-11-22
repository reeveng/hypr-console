// edition:2024

#![allow(dead_code)]

#[cfg(all())]
type Scalar = u8;
#[cfg(any())]
type Scalar = f32;

fn threshold(left: Scalar, right: Scalar) -> u8 {
    if left > right { 1 } else { 0 }
}

#[cfg(all())]
mod state {
    pub enum Stage {
        Ready,
        Pending,
    }
}

fn stage(value: state::Stage) -> u8 {
    if let state::Stage::Ready = value { 1 } else { 0 }
}

struct Counter;
impl Counter {
    fn value(self) -> u8 { 1 }
}

#[cfg(all())]
fn sample() -> Counter { Counter }

fn receiver() -> u8 {
    if sample().value() > 0 { 1 } else { 0 }
}

fn shadow() -> u8 {
    let left = 1_u8;
    let right = 2_u8;
    #[cfg(any())]
    let (left, right) = (1.0_f32, 2.0_f32);
    if left > right { 1 } else { 0 }
}

fn main() {
    let missing = None::<u8>;
    let skip = missing.is_none();
    if skip {
        println!("skipped");
    }
    if missing.is_none() {
        println!("missing");
    }
}
