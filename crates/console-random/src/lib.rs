//! The same stream of numbers python's `random` gives, for the same seed.
//!
//! The garden is drawn from a seed, so every petal, branch and tuft of grass
//! is decided by this. A different generator would draw a garden that was just
//! as good and not the same one, and then the only way to know whether the
//! drawing had been ported correctly would be to look at two pictures and
//! form an opinion. With the same stream the two files can simply be compared.
//!
//! Mersenne Twister, seeded and drawn from the way CPython does it, which is
//! not quite the way the reference implementation does. The differences are
//! marked where they are.
//!
//! This is the one thing here that is not immutable, and it cannot be: a
//! generator is a value whose whole purpose is to be different next time.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908b0df;
const UPPER: u32 = 0x8000_0000;
const LOWER: u32 = 0x7fff_ffff;

pub struct Random {
    state: [u32; N],
    at: usize,
    /// The other half of the last pair of normals. `gauss` makes two at a time
    /// and keeps one, so a generator that has been asked for an odd number of
    /// them is not in the same place as one asked for an even number.
    spare: Option<f64>,
}

impl Random {
    /// Seeded as python seeds from an integer: the number's own bytes, little
    /// end first, as the key array.
    pub fn seeded(seed: u64) -> Self {
        let key: Vec<u32> = match seed {
            0 => vec![0],
            seed => (0..(64 - seed.leading_zeros()).div_ceil(32))
                .map(|word| (seed >> (word * 32)) as u32)
                .collect(),
        };
        let mut made = Random { state: [0; N], at: N, spare: None };
        made.by_array(&key);
        made
    }

    fn init(&mut self, seed: u32) {
        self.state[0] = seed;
        for i in 1..N {
            let previous = self.state[i - 1];
            self.state[i] = 1812433253u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(i as u32);
        }
        self.at = N;
    }

    fn by_array(&mut self, key: &[u32]) {
        self.init(19650218);
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..N.max(key.len()) {
            let previous = self.state[i - 1];
            self.state[i] = (self.state[i] ^ (previous ^ (previous >> 30)).wrapping_mul(1664525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.state[0] = self.state[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            let previous = self.state[i - 1];
            self.state[i] = (self.state[i]
                ^ (previous ^ (previous >> 30)).wrapping_mul(1566083941))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.state[0] = self.state[N - 1];
                i = 1;
            }
        }
        self.state[0] = UPPER;
    }

    fn twist(&mut self) {
        for i in 0..N {
            let joined = (self.state[i] & UPPER) | (self.state[(i + 1) % N] & LOWER);
            let mixed = self.state[(i + M) % N] ^ (joined >> 1);
            self.state[i] = match joined & 1 {
                0 => mixed,
                _ => mixed ^ MATRIX_A,
            };
        }
        self.at = 0;
    }

    /// One raw draw, tempered.
    pub fn bits32(&mut self) -> u32 {
        if self.at >= N {
            self.twist();
        }
        let mut drawn = self.state[self.at];
        self.at += 1;
        drawn ^= drawn >> 11;
        drawn ^= (drawn << 7) & 0x9d2c_5680;
        drawn ^= (drawn << 15) & 0xefc6_0000;
        drawn ^ (drawn >> 18)
    }

    /// The top `k` bits of one draw. Only the sizes this repository asks for:
    /// nothing here chooses from more than four thousand million of anything.
    pub fn bits(&mut self, k: u32) -> u32 {
        match k {
            0 => 0,
            k => self.bits32() >> (32 - k.min(32)),
        }
    }

    /// A float in [0, 1), from two draws rather than one.
    ///
    /// A double has 53 bits of mantissa and a draw has 32, so python spends
    /// two and throws away eleven. Spending one would give a different number
    /// and half the resolution.
    pub fn random(&mut self) -> f64 {
        let high = (self.bits32() >> 5) as f64;
        let low = (self.bits32() >> 6) as f64;
        (high * 67108864.0 + low) * (1.0 / 9007199254740992.0)
    }

    /// A whole number below `n`, by redrawing rather than by taking a
    /// remainder. A remainder would favour the low end.
    pub fn below(&mut self, n: usize) -> usize {
        match n {
            0 => 0,
            n => {
                let width = usize::BITS - n.leading_zeros();
                loop {
                    let drawn = self.bits(width) as usize;
                    if drawn < n {
                        return drawn;
                    }
                }
            }
        }
    }

    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.random()
    }

    /// One of these, each as likely as the others.
    pub fn choice<'a, T>(&mut self, among: &'a [T]) -> &'a T {
        &among[self.below(among.len())]
    }

    /// A normal, by the polar method, two at a time.
    pub fn gauss(&mut self, mu: f64, sigma: f64) -> f64 {
        let z = match self.spare.take() {
            Some(kept) => kept,
            None => {
                let turn = self.random() * std::f64::consts::TAU;
                let radius = (-2.0 * (1.0 - self.random()).ln()).sqrt();
                self.spare = Some(turn.sin() * radius);
                turn.cos() * radius
            }
        };
        mu + z * sigma
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Taken from `python3 -c "import random; r = random.Random(20260828); ..."`.
    /// If these ever stop matching, the garden is a different garden.
    const SEED: u64 = 20260828;

    #[test]
    fn the_first_draws_are_pythons_first_draws() {
        let mut rng = Random::seeded(SEED);
        let got: Vec<f64> = (0..5).map(|_| rng.random()).collect();
        let want = [
            0.915949727345881,
            0.32970331018806975,
            0.27489759722247775,
            0.458570731197423,
            0.767886240594708,
        ];
        for (got, want) in got.iter().zip(want) {
            assert_eq!(got, &want, "got {got:?}, python says {want:?}");
        }
    }

    #[test]
    fn a_seed_of_nought_is_still_a_seed() {
        let mut rng = Random::seeded(0);
        assert_eq!(rng.random(), 0.8444218515250481);
    }

    #[test]
    fn a_seed_wider_than_one_word_uses_both() {
        let mut rng = Random::seeded(0x1_0000_0000);
        assert_eq!(rng.random(), 0.11299430095636409);
    }

    #[test]
    fn two_generators_with_one_seed_say_the_same_thing() {
        let (mut one, mut other) = (Random::seeded(SEED), Random::seeded(SEED));
        for _ in 0..100 {
            assert_eq!(one.random(), other.random());
        }
    }

    #[test]
    fn a_uniform_is_a_random_stretched_between_two_ends() {
        let (mut one, mut other) = (Random::seeded(SEED), Random::seeded(SEED));
        assert_eq!(one.uniform(3.0, 7.0), 3.0 + 4.0 * other.random());
    }

    #[test]
    fn a_uniform_stays_between_its_ends() {
        let mut rng = Random::seeded(SEED);
        for _ in 0..1000 {
            let got = rng.uniform(-2.5, 4.5);
            assert!((-2.5..4.5).contains(&got), "{got}");
        }
    }

    #[test]
    fn choosing_costs_what_it_costs_python() {
        // Even from a list of one. Asking for a number below one draws a bit,
        // and redraws while the bit is a one, so the answer is never in doubt
        // and the draws are spent anyway. Anything cheaper would move every
        // number after it and draw a different garden.
        let mut chose = Random::seeded(SEED);
        assert_eq!(chose.choice(&[9]), &9);
        assert_eq!(chose.random(), 0.32970331018806975);
    }

    #[test]
    fn the_top_bits_of_a_draw_are_pythons_top_bits() {
        let mut rng = Random::seeded(SEED);
        let got: Vec<u32> = [1, 4, 8, 16, 32].into_iter().map(|k| rng.bits(k)).collect();
        assert_eq!(got, [1, 5, 84, 13600, 1180676164]);
    }

    #[test]
    fn a_normal_is_pythons_normal() {
        let mut rng = Random::seeded(SEED);
        assert_eq!(rng.gauss(0.0, 1.0), 0.7726079193931775);
        assert_eq!(rng.gauss(0.0, 1.0), -0.450717972670029);
    }

    #[test]
    fn choosing_stays_inside_the_list() {
        let mut rng = Random::seeded(SEED);
        let among = [0, 1, 2, 3, 4, 5, 6];
        for _ in 0..1000 {
            assert!(among.contains(rng.choice(&among)));
        }
    }

    #[test]
    fn the_spare_normal_is_kept_and_spent_next() {
        // Two calls cost two draws, not four: the pair is made together.
        let mut paired = Random::seeded(SEED);
        let (first, second) = (paired.gauss(0.0, 1.0), paired.gauss(0.0, 1.0));

        let mut counted = Random::seeded(SEED);
        let turn = counted.random() * std::f64::consts::TAU;
        let radius = (-2.0 * (1.0 - counted.random()).ln()).sqrt();
        assert_eq!(first, turn.cos() * radius);
        assert_eq!(second, turn.sin() * radius);
    }

    #[test]
    fn a_gauss_is_moved_and_stretched_by_what_it_is_asked_for() {
        let (mut one, mut other) = (Random::seeded(SEED), Random::seeded(SEED));
        assert_eq!(one.gauss(5.0, 2.0), 5.0 + 2.0 * other.gauss(0.0, 1.0));
    }
}
