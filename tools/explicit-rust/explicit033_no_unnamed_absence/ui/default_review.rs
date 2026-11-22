// edition:2024

#![allow(dead_code)]

enum Mode {
    Standard,
}

#[cfg(all())]
impl Default for Mode {
    fn default() -> Self {
        Self::Standard
    }
}

fn configured(input: Option<Mode>) -> Mode {
    input.unwrap_or_default()
}

enum Effectful {
    Standard,
}

impl Default for Effectful {
    fn default() -> Self {
        eprintln!("default was used");
        Self::Standard
    }
}

fn observed(input: Option<Effectful>) -> Effectful {
    input.unwrap_or_default()
}

#[derive(Default)]
enum Unconditional {
    #[default]
    Ready,
}

#[cfg(all())]
type Alias = Unconditional;
#[cfg(any())]
type Alias = Mode;

fn chosen(input: Option<Alias>) -> Alias {
    input.unwrap_or_default()
}

fn main() {}
