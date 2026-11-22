// edition:2024
#![allow(unused)]

fn conditionally_short() {
    #[cfg(feature = "other")]
    let mut scratch = [0; 2];
    #[cfg(not(feature = "other"))]
    let mut scratch = [0; 8];
    for x in 0..8 { scratch[x] = x; }
}

fn different_input() {
    #[cfg(feature = "other")]
    let seed = [0; 2];
    #[cfg(not(feature = "other"))]
    let seed = [0; 8];
    {
        let mut scratch = seed;
        for x in 0..8 { scratch[x] = x; }
    }
}

fn other_use() {
    for _ in 0..2 {
        let mut scratch = [0; 8];
        for x in 0..8 {
            #[cfg(feature = "other")]
            scratch.reverse();
            scratch[x] = x;
        }
    }
}

fn conditional_macro() {
    #[cfg(feature = "other")]
    macro_rules! resize { ($s:ident) => { let mut $s = [0; 2]; } }
    #[cfg(not(feature = "other"))]
    macro_rules! resize { ($s:ident) => {} }
    {
        let mut scratch = [0; 8];
        resize!(scratch);
        for x in 0..8 { scratch[x] = x; }
    }
}

fn main() {}
