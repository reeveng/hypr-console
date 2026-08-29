// The same stream, printed, so python's can be diffed against it.
use console_random::Random;

fn main() {
    for seed in [0u64, 1, 20260828, 20260833, 4294967296, 123456789012345] {
        let mut rng = Random::seeded(seed);
        let among: Vec<u32> = (0..7).collect();
        for turn in 0..200 {
            match turn % 5 {
                0 => println!("{seed} {turn} random {:.17}", rng.random()),
                1 => println!("{seed} {turn} uniform {:.17}", rng.uniform(-3.5, 9.25)),
                2 => println!("{seed} {turn} gauss {:.17}", rng.gauss(0.4, 1.7)),
                3 => println!("{seed} {turn} choice {}", rng.choice(&among)),
                _ => println!("{seed} {turn} bits {}", rng.bits(1 + (turn as u32 % 31))),
            }
        }
    }
}
