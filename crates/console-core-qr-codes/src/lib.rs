//! Some bytes, as the squares of a QR code a phone camera reads.
//!
//! One thing asks for this: the Wi-Fi settings showing the network as a code
//! a phone joins by pointing at it. That is a short line of text, so what is
//! here is the part of the standard that line needs and no more -- bytes
//! rather than the digit and letter modes, error correction at M, and the
//! first ten sizes, which hold 213 bytes and so any network name and password
//! a router accepts. `qrencode` would have drawn it too, as one more package
//! for a machine rebuilt from its manifest to carry; the encoder is a few
//! tables and a Reed-Solomon remainder, and it is written here.
//!
//! The steps are the standard's, in its order and after Project Nayuki's
//! reading of it: the bytes behind a mode and a count, padded to what the
//! size holds; split into blocks, each given its remainder, and interleaved;
//! laid round the fixed patterns in the zigzag; then each of the eight masks
//! tried and the one with the fewest look-alike patterns kept. Of the
//! standard's four penalties the one for a run that looks like a finder is
//! left out: it only makes a good mask better, and any mask decodes.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    pub size: u32,
    pub modules: Vec<Vec<Module>>,
}

impl Code {
    pub fn at(&self, at: Point<u32>) -> Result<Module, Never> {
        let Ok(found) = cell(&self.modules, at);

        Ok(match found {
            Some(module) => module,
            None => Module::Light,
        })
    }
}

pub const QUIET: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub side: u32,
    pub rgb: Vec<u8>,
}

impl Code {
    pub fn drawn(&self, scale: u32) -> Result<Bitmap, Never> {
        let scale = scale.max(1);
        let side = self.size.saturating_add(QUIET.saturating_mul(2)).saturating_mul(scale);
        let Ok(width) = index(side);
        let mut rgb = Vec::with_capacity(width.saturating_mul(width).saturating_mul(3));

        for down in 0..side {
            for across in 0..side {
                let within = |at: u32| at.checked_div(scale).and_then(|module| module.checked_sub(QUIET));
                let module = match (within(across), within(down)) {
                    (Some(across), Some(down)) => {
                        let Ok(module) = self.at(Point { x: across, y: down });

                        module
                    },
                    (None, _) | (_, None) => Module::Light,
                };
                let shade = match module {
                    Module::Dark => 0,
                    Module::Light => u8::MAX,
                };

                rgb.extend_from_slice(&[shade; 3]);
            }
        }

        Ok(Bitmap { side, rgb })
    }
}

struct Size {
    total: u32,
    blocks: u32,
    correcting: u32,
    aligned: &'static [u32],
}

const SIZES: [Size; 10] = [
    Size { total: 26, blocks: 1, correcting: 10, aligned: &[] },
    Size { total: 44, blocks: 1, correcting: 16, aligned: &[6, 18] },
    Size { total: 70, blocks: 1, correcting: 26, aligned: &[6, 22] },
    Size { total: 100, blocks: 2, correcting: 18, aligned: &[6, 26] },
    Size { total: 134, blocks: 2, correcting: 24, aligned: &[6, 30] },
    Size { total: 172, blocks: 4, correcting: 16, aligned: &[6, 34] },
    Size { total: 196, blocks: 4, correcting: 18, aligned: &[6, 22, 38] },
    Size { total: 242, blocks: 4, correcting: 22, aligned: &[6, 24, 42] },
    Size { total: 292, blocks: 5, correcting: 22, aligned: &[6, 26, 46] },
    Size { total: 346, blocks: 5, correcting: 26, aligned: &[6, 28, 50] },
];

impl Size {
    fn data(&self) -> Result<u32, Never> {
        Ok(self.total.saturating_sub(self.blocks.saturating_mul(self.correcting)))
    }
}

const BYTE_MODE: u32 = 0b0100;

const LEVEL_M: u32 = 0b00;

pub fn encoded(bytes: &[u8]) -> Result<Option<Code>, Never> {
    let Ok(many) = fitted::<_, u32>(bytes.len());

    let chosen = SIZES.iter().zip(1_u32..).find(|(size, version)| {
        let counted = match *version < 10 {
            true => 8,
            false => 16,
        };
        let Ok(room) = size.data();
        let needed = 4_u32.saturating_add(counted).saturating_add(many.saturating_mul(8));

        needed <= room.saturating_mul(8)
    });

    let (size, version) = match chosen {
        Some(chosen) => chosen,
        None => return Ok(None),
    };

    let Ok(data) = codewords(bytes, size, version);
    let Ok(interleaved) = corrected(&data, size);
    let Ok(grid) = Grid::fixed(version, size.aligned);
    let side = grid.side;
    let Ok(grid) = grid.filled(&interleaved);
    let Ok(best) = masked(&grid);

    Ok(Some(Code { size: side, modules: best.dark }))
}

fn side_of(version: u32) -> Result<u32, Never> {
    Ok(version.saturating_mul(4).saturating_add(17))
}

struct Bits(Vec<u8>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field {
    value: u32,
    width: u32,
}

impl Bits {
    fn push(&mut self, Field { value, width }: Field) -> Result<(), Never> {
        for at in (0..width).rev() {
            self.0.push(match (value.wrapping_shr(at)) & 1 {
                0 => 0,
                _one => 1,
            });
        }

        Ok(())
    }
}

fn codewords(bytes: &[u8], size: &Size, version: u32) -> Result<Vec<u8>, Never> {
    let Ok(room) = size.data();
    let room_bits = room.saturating_mul(8);
    let Ok(many) = fitted::<_, u32>(bytes.len());
    let mut bits = Bits(Vec::new());

    let Ok(()) = bits.push(Field { value: BYTE_MODE, width: 4 });
    let Ok(()) = bits.push(Field {
        value: many,
        width: match version < 10 {
            true => 8,
            false => 16,
        },
    });

    for byte in bytes {
        let Ok(()) = bits.push(Field { value: u32::from(*byte), width: 8 });
    }

    let Ok(written) = fitted::<_, u32>(bits.0.len());
    let ended = room_bits.saturating_sub(written).min(4);
    let Ok(()) = bits.push(Field { value: 0, width: ended });
    let Ok(written) = fitted::<_, u32>(bits.0.len());
    let to_a_byte = written.wrapping_neg() & 7;
    let Ok(()) = bits.push(Field { value: 0, width: to_a_byte });

    for pad in [0xEC_u32, 0x11].into_iter().cycle() {
        let Ok(written) = fitted::<_, u32>(bits.0.len());

        match written < room_bits {
            true => {
                let Ok(()) = bits.push(Field { value: pad, width: 8 });
            }
            false => break,
        }
    }

    Ok(bits
        .0
        .chunks(8)
        .map(|byte| byte.iter().fold(0_u8, |made, bit| made.wrapping_shl(1) | *bit))
        .collect())
}

fn times([one, other]: [u8; 2]) -> Result<u8, Never> {
    let mut made: u8 = 0;

    for at in (0..8).rev() {
        let carried = match made & 0x80 {
            0 => 0,
            _high => 0x1D,
        };

        made = made.wrapping_shl(1) ^ carried;

        match other.wrapping_shr(at) & 1 {
            0 => {},
            _one => made ^= one,
        }
    }

    Ok(made)
}

fn divisor(degree: u32) -> Result<Vec<u8>, Never> {
    let Ok(many) = index(degree);
    let mut made = vec![0_u8; many];
    let mut root: u8 = 1;

    match made.last_mut() {
        Some(last) => *last = 1,
        None => return Ok(made),
    }

    for _ in 0..degree {
        let after: Vec<u8> = made.iter().skip(1).copied().chain(std::iter::once(0)).collect();

        for (term, next) in made.iter_mut().zip(after) {
            let Ok(product) = times([*term, root]);

            *term = product ^ next;
        }

        let Ok(doubled) = times([root, 0x02]);

        root = doubled;
    }

    Ok(made)
}

fn remainder(data: &[u8], divisor: &[u8]) -> Result<Vec<u8>, Never> {
    let mut made = vec![0_u8; divisor.len()];

    for byte in data {
        let first = match made.first() {
            Some(first) => *first,
            None => return Ok(made),
        };
        let factor = byte ^ first;

        made.remove(0);
        made.push(0);

        for (term, coefficient) in made.iter_mut().zip(divisor) {
            let Ok(product) = times([*coefficient, factor]);

            *term ^= product;
        }
    }

    Ok(made)
}

fn corrected(data: &[u8], size: &Size) -> Result<Vec<u8>, Never> {
    let Ok(divisor) = divisor(size.correcting);
    let short = size.blocks.saturating_sub(size.total.wrapping_rem(size.blocks));
    let short_long = size.total.wrapping_div(size.blocks);
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut from = data.iter();

    for block in 0..size.blocks {
        let longer = match block < short {
            true => 0,
            false => 1,
        };
        let Ok(taking) = index(short_long.saturating_sub(size.correcting).saturating_add(longer));
        let mut made: Vec<u8> = from.by_ref().take(taking).copied().collect();
        let Ok(correcting) = remainder(&made, &divisor);

        match longer {
            0 => made.push(0),
            _longer => {},
        }

        made.extend(correcting);
        blocks.push(made);
    }

    let skipped = short_long.saturating_sub(size.correcting);
    let Ok(long) = fitted::<_, u32>(blocks.first().map_or(0, Vec::len));
    let mut made = Vec::new();

    for at in 0..long {
        let Ok(place) = index(at);

        for (which, block) in (0_u32..).zip(&blocks) {
            match (at == skipped && which < short, block.get(place)) {
                (false, Some(byte)) => made.push(*byte),
                (true, _) | (false, None) => {},
            }
        }
    }

    Ok(made)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Fixed,
    Data,
}

#[derive(Clone)]
struct Grid {
    side: u32,
    dark: Vec<Vec<Module>>,
    role: Vec<Vec<Role>>,
}

impl Grid {
    fn fixed(version: u32, aligned: &[u32]) -> Result<Grid, Never> {
        let Ok(side) = side_of(version);
        let Ok(many) = index(side);
        let mut grid = Grid {
            side,
            dark: vec![vec![Module::Light; many]; many],
            role: vec![vec![Role::Data; many]; many],
        };

        for at in 0..side {
            let module = match at.checked_rem(2) {
                Some(0) => Module::Dark,
                Some(_) | None => Module::Light,
            };
            let Ok(()) = grid.fix(Point { x: 6, y: at }, module);
            let Ok(()) = grid.fix(Point { x: at, y: 6 }, module);
        }

        let far = side.saturating_sub(7);

        for (across, down) in [(0, 0), (far, 0), (0, far)] {
            let Ok(()) = grid.finder(Point { x: across, y: down });
        }

        let Ok(last) = fitted::<_, u32>(aligned.len().saturating_sub(1));

        for (one, across) in (0_u32..).zip(aligned) {
            for (other, down) in (0_u32..).zip(aligned) {
                let corner = (one == 0 && other == 0) || (one == 0 && other == last) || (one == last && other == 0);

                match corner {
                    true => {},
                    false => {
                        let Ok(()) = grid.alignment(Point { x: *across, y: *down });
                    }
                }
            }
        }

        let Ok(()) = grid.formatted(Mask(0));
        let Ok(()) = grid.versioned(version);

        Ok(grid)
    }

    fn fix(&mut self, at: Point<u32>, module: Module) -> Result<(), Never> {
        let Ok(()) = put(&mut self.dark, at, module);

        put(&mut self.role, at, Role::Fixed)
    }

    fn finder(&mut self, corner: Point<u32>) -> Result<(), Never> {
        for down in -1_i32..=7 {
            for across in -1_i32..=7 {
                let far = across.abs_diff(3).max(down.abs_diff(3));
                let module = match far {
                    2 | 4 => Module::Light,
                    _ => Module::Dark,
                };
                let x = i64::from(corner.x).saturating_add(i64::from(across));
                let y = i64::from(corner.y).saturating_add(i64::from(down));

                match (u32::try_from(x), u32::try_from(y)) {
                    (Ok(x), Ok(y)) => match x < self.side && y < self.side {
                        true => {
                            let Ok(()) = self.fix(Point { x, y }, module);
                        }
                        false => {},
                    },
                    (Err(_), _) | (_, Err(_)) => {},
                }
            }
        }

        Ok(())
    }

    fn alignment(&mut self, middle: Point<u32>) -> Result<(), Never> {
        for down in 0_u32..5 {
            for across in 0_u32..5 {
                let module = match (across, down) {
                    (2, 2) | (0 | 4, _) | (_, 0 | 4) => Module::Dark,
                    _inside => Module::Light,
                };
                let x = middle.x.saturating_add(across).saturating_sub(2);
                let y = middle.y.saturating_add(down).saturating_sub(2);
                let Ok(()) = self.fix(Point { x, y }, module);
            }
        }

        Ok(())
    }

    fn formatted(&mut self, mask: Mask) -> Result<(), Never> {
        let Ok(bits) = format_bits(mask);
        let bit = |at: u32| match bits.wrapping_shr(at) & 1 {
            0 => Module::Light,
            _one => Module::Dark,
        };
        let side = self.side;

        for at in 0..=5 {
            let Ok(()) = self.fix(Point { x: 8, y: at }, bit(at));
        }

        let Ok(()) = self.fix(Point { x: 8, y: 7 }, bit(6));
        let Ok(()) = self.fix(Point { x: 8, y: 8 }, bit(7));
        let Ok(()) = self.fix(Point { x: 7, y: 8 }, bit(8));

        for at in 9..15 {
            let Ok(()) = self.fix(Point { x: 14_u32.saturating_sub(at), y: 8 }, bit(at));
        }

        for at in 0..8 {
            let Ok(()) = self.fix(Point { x: side.saturating_sub(1).saturating_sub(at), y: 8 }, bit(at));
        }

        for at in 8..15 {
            let Ok(()) = self.fix(Point { x: 8, y: side.saturating_sub(15).saturating_add(at) }, bit(at));
        }

        self.fix(Point { x: 8, y: side.saturating_sub(8) }, Module::Dark)
    }

    fn versioned(&mut self, version: u32) -> Result<(), Never> {
        match version < 7 {
            true => return Ok(()),
            false => {},
        }

        let mut left = version;

        for _ in 0..12 {
            left = left.wrapping_shl(1) ^ (left.wrapping_shr(11).wrapping_mul(0x1F25));
        }

        let bits = version.wrapping_shl(12) | left;
        let far = self.side.saturating_sub(11);

        for at in 0..18_u32 {
            let module = match bits.wrapping_shr(at) & 1 {
                0 => Module::Light,
                _one => Module::Dark,
            };
            let one = far.saturating_add(at.wrapping_rem(3));
            let other = at.wrapping_div(3);
            let Ok(()) = self.fix(Point { x: one, y: other }, module);
            let Ok(()) = self.fix(Point { x: other, y: one }, module);
        }

        Ok(())
    }

    fn role(&self, at: Point<u32>) -> Result<Role, Never> {
        let Ok(found) = cell(&self.role, at);

        Ok(match found {
            Some(role) => role,
            None => Role::Fixed,
        })
    }

    fn set(&mut self, at: Point<u32>, module: Module) -> Result<(), Never> {
        put(&mut self.dark, at, module)
    }

    fn filled(mut self, data: &[u8]) -> Result<Grid, Never> {
        let Ok(many) = fitted::<_, u32>(data.len().saturating_mul(8));
        let mut taken: u32 = 0;
        let mut right = self.side.saturating_sub(1);

        loop {
            match right {
                6 => right = 5,
                _other => {},
            }

            for vertical in 0..self.side {
                for step in 0..2_u32 {
                    let across = right.saturating_sub(step);
                    let upward = right.saturating_add(1) & 2 == 0;
                    let down = match upward {
                        true => self.side.saturating_sub(1).saturating_sub(vertical),
                        false => vertical,
                    };
                    let Ok(role) = self.role(Point { x: across, y: down });

                    match (role, taken < many) {
                        (Role::Data, true) => {
                            let Ok(byte) = index(taken.wrapping_shr(3));
                            let bit = 7_u32.saturating_sub(taken & 7);
                            let module = match data.get(byte).map(|byte| byte.wrapping_shr(bit) & 1) {
                                Some(1) => Module::Dark,
                                Some(_) | None => Module::Light,
                            };
                            let Ok(()) = self.set(Point { x: across, y: down }, module);

                            taken = taken.saturating_add(1);
                        }
                        (Role::Data, false) | (Role::Fixed, _) => {},
                    }
                }
            }

            match right.checked_sub(2) {
                Some(next) => right = next,
                None => break,
            }
        }

        Ok(self)
    }

    fn masked_by(&self, mask: Mask) -> Result<Grid, Never> {
        let mut made = self.clone();

        for down in 0..self.side {
            for across in 0..self.side {
                let at = Point { x: across, y: down };
                let Ok(role) = self.role(at);
                let Ok(flips) = flips(mask, at);

                match (role, flips) {
                    (Role::Data, Flip::Yes) => {
                        let Ok(was) = self.module(at);
                        let Ok(()) = made.set(at, match was {
                            Module::Dark => Module::Light,
                            Module::Light => Module::Dark,
                        });
                    }
                    (Role::Data, Flip::No) | (Role::Fixed, _) => {},
                }
            }
        }

        let Ok(()) = made.formatted(mask);

        Ok(made)
    }

    fn module(&self, at: Point<u32>) -> Result<Module, Never> {
        let Ok(found) = cell(&self.dark, at);

        Ok(match found {
            Some(module) => module,
            None => Module::Light,
        })
    }
}

fn cell<T: Copy>(rows: &[Vec<T>], at: Point<u32>) -> Result<Option<T>, Never> {
    let Ok(row) = index(at.y);
    let Ok(column) = index(at.x);

    Ok(rows.get(row).and_then(|row| row.get(column)).copied())
}

fn put<T>(rows: &mut [Vec<T>], at: Point<u32>, value: T) -> Result<(), Never> {
    let Ok(row) = index(at.y);
    let Ok(column) = index(at.x);

    match rows.get_mut(row).and_then(|row| row.get_mut(column)) {
        Some(held) => *held = value,
        None => {},
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mask(u32);

fn format_bits(Mask(mask): Mask) -> Result<u32, Never> {
    let data = LEVEL_M.wrapping_shl(3) | mask;
    let mut left = data;

    for _ in 0..10 {
        left = left.wrapping_shl(1) ^ (left.wrapping_shr(9).wrapping_mul(0x537));
    }

    Ok((data.wrapping_shl(10) | left) ^ 0x5412)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flip {
    Yes,
    No,
}

fn flips(Mask(mask): Mask, Point { x, y }: Point<u32>) -> Result<Flip, Never> {
    let sum = x.saturating_add(y);
    let product = x.saturating_mul(y);
    let rem = |value: u32, by: u32| value.wrapping_rem(by);

    let even = match mask {
        0 => rem(sum, 2),
        1 => rem(y, 2),
        2 => rem(x, 3),
        3 => rem(sum, 3),
        4 => rem(x.wrapping_div(3).saturating_add(y.wrapping_div(2)), 2),
        5 => rem(product, 2).saturating_add(rem(product, 3)),
        6 => rem(rem(product, 2).saturating_add(rem(product, 3)), 2),
        _seventh => rem(rem(sum, 2).saturating_add(rem(product, 3)), 2),
    };

    Ok(match even {
        0 => Flip::Yes,
        _ => Flip::No,
    })
}

fn masked(grid: &Grid) -> Result<Grid, Never> {
    let mut best: Option<(u32, Grid)> = None;

    for mask in (0..8).map(Mask) {
        let Ok(tried) = grid.masked_by(mask);
        let Ok(penalty) = penalty(&tried);

        best = match best {
            Some((held, kept)) => match penalty < held {
                true => Some((penalty, tried)),
                false => Some((held, kept)),
            },
            None => Some((penalty, tried)),
        };
    }

    Ok(match best {
        Some((_, grid)) => grid,
        None => grid.clone(),
    })
}

fn penalty(grid: &Grid) -> Result<u32, Never> {
    let mut score: u32 = 0;
    let mut dark: u32 = 0;

    for one in 0..grid.side {
        let mut across_run = (Module::Light, 0_u32);
        let mut down_run = (Module::Light, 0_u32);

        for other in 0..grid.side {
            let Ok(this_way) = grid.module(Point { x: other, y: one });
            let Ok(that_way) = grid.module(Point { x: one, y: other });
            let Ok(scored) = run(&mut across_run, this_way, other);
            let Ok(scored_too) = run(&mut down_run, that_way, other);

            score = score.saturating_add(scored).saturating_add(scored_too);

            match this_way {
                Module::Dark => dark = dark.saturating_add(1),
                Module::Light => {},
            }

            let Ok(square) = square_at(grid, Point { x: other, y: one });

            score = score.saturating_add(square);
        }
    }

    let whole = grid.side.saturating_mul(grid.side).max(1);
    let percent = dark.saturating_mul(100).wrapping_div(whole);
    let off = percent.abs_diff(50).wrapping_div(5);

    Ok(score.saturating_add(off.saturating_mul(10)))
}

fn run(held: &mut (Module, u32), module: Module, at: u32) -> Result<u32, Never> {
    let (was, long) = *held;

    match (at == 0, module == was) {
        (false, true) => {
            let long = long.saturating_add(1);

            *held = (module, long);

            Ok(match long {
                5 => 3,
                6.. => 1,
                _ => 0,
            })
        }
        (true, _) | (false, false) => {
            *held = (module, 1);

            Ok(0)
        }
    }
}

fn square_at(grid: &Grid, Point { x: across, y: down }: Point<u32>) -> Result<u32, Never> {
    match (across.checked_add(1), down.checked_add(1)) {
        (Some(right), Some(below)) => match right < grid.side && below < grid.side {
            true => {
                let Ok(one) = grid.module(Point { x: across, y: down });
                let Ok(two) = grid.module(Point { x: right, y: down });
                let Ok(three) = grid.module(Point { x: across, y: below });
                let Ok(four) = grid.module(Point { x: right, y: below });

                Ok(match one == two && two == three && three == four {
                    true => 3,
                    false => 0,
                })
            }
            false => Ok(0),
        },
        (None, _) | (_, None) => Ok(0),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network<'a> {
    pub name: &'a str,
    pub password: &'a str,
}

pub fn wifi(Network { name, password }: Network<'_>) -> Result<String, Never> {
    let escaped = |said: &str| {
        said.chars().fold(String::new(), |mut made, letter| {
            match letter {
                '\\' | ';' | ',' | ':' | '"' => made.push('\\'),
                _plain => {},
            }

            made.push(letter);

            made
        })
    };

    Ok(match password.is_empty() {
        true => format!("WIFI:T:nopass;S:{};;", escaped(name)),
        false => format!("WIFI:T:WPA;S:{};P:{};;", escaped(name), escaped(password)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drawn_code_has_its_quiet_border_and_each_square_is_the_scale_wide() {
        let code = match encoded(b"hello") {
            Ok(Some(code)) => code,
            Ok(None) => panic!("hello fits in the smallest code"),
        };
        let Ok(drawn) = code.drawn(2);
        let shade = |across: u32, down: u32| {
            let at = down.saturating_mul(drawn.side).saturating_add(across).saturating_mul(3);
            let Ok(at) = index(at);

            drawn.rgb.get(at).copied()
        };
        let corner = QUIET.saturating_mul(2);

        assert_eq!(drawn.side, code.size.saturating_add(8).saturating_mul(2));
        assert_eq!(fitted::<_, u32>(drawn.rgb.len()), Ok(drawn.side.saturating_mul(drawn.side).saturating_mul(3)));
        assert_eq!(shade(0, 0), Some(u8::MAX), "the quiet border is not light");
        assert_eq!(shade(corner, corner), Some(0), "the finder's corner is not dark");
        assert_eq!(shade(corner.saturating_add(1), corner.saturating_add(1)), Some(0), "a square is not two pixels wide");
    }

    #[test]
    fn the_remainder_is_the_one_the_standard_works_by_hand() {
        let data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
        let Ok(divisor) = divisor(10);
        let Ok(made) = remainder(&data, &divisor);

        assert_eq!(made, vec![196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
    }

    #[test]
    fn the_format_line_for_level_m_and_the_first_mask_is_the_standards() {
        assert_eq!(format_bits(Mask(0)), Ok(0b101010000010010));
    }

    #[test]
    fn the_size_grows_with_what_it_has_to_hold() {
        let small = encoded(b"hi");
        let large = encoded(&[b'x'; 150]);

        assert_eq!(small.map(|code| code.map(|code| code.size)), Ok(Some(21)));
        assert_eq!(large.map(|code| code.map(|code| code.size)), Ok(Some(49)));
        assert_eq!(encoded(&[b'x'; 300]), Ok(None), "more than the tenth size holds");
    }

    #[test]
    fn every_corner_but_one_holds_a_finder() {
        let code = match encoded(b"WIFI:T:WPA;S:home;P:secret;;") {
            Ok(Some(code)) => code,
            Ok(None) => panic!("no code"),
        };
        let far = code.size.saturating_sub(1);

        for (across, down) in [(0, 0), (far, 0), (0, far), (3, 3)] {
            assert_eq!(code.at(Point { x: across, y: down }), Ok(Module::Dark), "{across},{down}");
        }

        assert_eq!(code.at(Point { x: 7, y: 7 }), Ok(Module::Light), "the separator");
    }

    #[test]
    fn a_network_is_said_the_way_a_phone_camera_reads_it() {
        let said = |name, password| wifi(Network { name, password });

        assert_eq!(said("home", "secret"), Ok("WIFI:T:WPA;S:home;P:secret;;".to_string()));
        assert_eq!(said("a;b", "c:d"), Ok("WIFI:T:WPA;S:a\\;b;P:c\\:d;;".to_string()));
        assert_eq!(said("cafe", ""), Ok("WIFI:T:nopass;S:cafe;;".to_string()));
    }
}
