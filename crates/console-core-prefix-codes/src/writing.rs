//! The other way round: bits written from the lowest of each byte up, and the
//! canonical prefix code a count of each symbol deserves.
//!
//! A [`BitWriter`] gathers fields into a word and hands the bytes on four at a
//! time, low end first, which is the order [`crate::Bits`] takes them back in.
//!
//! A [`Codebook`] is the code each symbol is written with. It is made from the
//! lengths of the codes, which is all a canonical code is, or from how often
//! each symbol is going to be written. That second way is Huffman's: the two
//! lightest are joined until one tree is left, and how deep each symbol ends up
//! is its length. The leaves are sorted once and what is joined comes out no
//! lighter than what was joined before it, so the lightest of either is always
//! at the front of its own list and nothing is ever searched for.
//!
//! Huffman's tree has no ceiling and a format does -- fifteen bits in deflate,
//! seven for the code its lengths are sent in -- so a tree that goes deeper is
//! cut back the way miniz cuts it: every code past the ceiling is brought up to
//! it, which asks for more codes than there are, and then one at a time a code
//! at the ceiling is given up and the deepest code above it is split in two,
//! until the lengths make exactly the strings of bits there are. The symbols
//! then take the lengths in order of how often they come, so the commonest are
//! the shortest. That is not the shortest code under the ceiling in every case,
//! which package-merge would find; it is within a fraction of a percent of it
//! on anything a picture makes, and it is a page rather than a chapter.
//!
//! A code is written low bit first like everything else in the stream but is
//! read from its high bit, so each is kept reversed and written as it is kept.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::CodeError;

const WORD: u32 = 32;

const DEEPEST: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub value: u32,
    pub width: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitWriter {
    bytes: Vec<u8>,
    word: u64,
    count: u32,
}

impl BitWriter {
    pub fn new() -> Result<BitWriter, Never> {
        Ok(BitWriter { bytes: Vec::new(), word: 0, count: 0 })
    }

    #[inline(always)]
    pub fn put(&mut self, field: Field) -> Result<(), Never> {
        let width = field.width.min(WORD);
        let kept = u64::from(field.value) & 1u64.wrapping_shl(width).wrapping_sub(1);

        self.word |= kept.wrapping_shl(self.count);
        self.count = self.count.saturating_add(width);

        match self.count >= WORD {
            true => {
                let Ok(low) = fitted::<u64, u32>(self.word & 0xFFFF_FFFF);

                self.bytes.extend_from_slice(&low.to_le_bytes());
                self.word = self.word.wrapping_shr(WORD);
                self.count = self.count.saturating_sub(WORD);
            },
            false => {},
        }

        Ok(())
    }

    pub fn aligned(&mut self) -> Result<(), Never> {
        let Ok(()) = self.put(Field { value: 0, width: self.count.wrapping_neg() & 7 });
        let Ok(held) = index(self.count.wrapping_shr(3));

        self.bytes.extend(self.word.to_le_bytes().iter().take(held));
        self.word = 0;
        self.count = 0;

        Ok(())
    }

    pub fn copied(&mut self, bytes: &[u8]) -> Result<(), Never> {
        let Ok(()) = self.aligned();

        self.bytes.extend_from_slice(bytes);

        Ok(())
    }

    pub fn finished(mut self) -> Result<Vec<u8>, Never> {
        let Ok(()) = self.aligned();

        Ok(self.bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codebook {
    lengths: Vec<u8>,
    codes: Vec<u16>,
}

impl Codebook {
    pub fn from_lengths(lengths: &[u8]) -> Result<Codebook, CodeError> {
        let count = crate::counted(lengths)?;
        let _strings_no_code_begins = crate::unused(&count)?;
        let mut next = [0u32; 16];
        let mut code = 0u32;
        let shorter = std::iter::once(0u16).chain(count.iter().skip(1).copied());

        for (slot, counted) in next.iter_mut().skip(1).zip(shorter) {
            code = code.saturating_add(u32::from(counted)).wrapping_shl(1);
            *slot = code;
        }

        let mut codes = Vec::with_capacity(lengths.len());

        for length in lengths {
            let Ok(at) = index(*length);

            let assigned = match next.get_mut(at) {
                Some(assigned) => assigned,
                None => return Err(CodeError::Oversubscribed),
            };

            let reversed = assigned.reverse_bits().wrapping_shr(WORD.saturating_sub(u32::from(*length)));
            let Ok(reversed) = fitted::<u32, u16>(reversed);

            codes.push(reversed);
            *assigned = assigned.saturating_add(1);
        }

        Ok(Codebook { lengths: lengths.to_vec(), codes })
    }

    pub fn from_counts(counts: &[u32], longest: u8) -> Result<Codebook, CodeError> {
        let Ok(lengths) = lengths(counts, longest);

        Codebook::from_lengths(&lengths)
    }

    pub fn lengths(&self) -> Result<&[u8], Never> {
        Ok(&self.lengths)
    }

    #[inline(always)]
    pub fn put(&self, writer: &mut BitWriter, symbol: u16) -> Result<(), CodeError> {
        let Ok(at) = index(symbol);

        match (self.codes.get(at), self.lengths.get(at)) {
            (Some(code), Some(length @ 1..)) => {
                let Ok(()) = writer.put(Field { value: u32::from(*code), width: u32::from(*length) });

                Ok(())
            },
            (Some(_), Some(0)) | (None, _) | (_, None) => Err(CodeError::Uncoded),
        }
    }

    pub fn cost(&self, counts: &[u32]) -> Result<u64, Never> {
        Ok(counts
            .iter()
            .zip(&self.lengths)
            .fold(0u64, |bits, (count, length)| bits.saturating_add(u64::from(*count).saturating_mul(u64::from(*length)))))
    }
}

fn lengths(counts: &[u32], longest: u8) -> Result<Vec<u8>, Never> {
    let mut used: Vec<(u32, u16)> = (0u16..).zip(counts).filter(|(_, count)| **count > 0).map(|(symbol, count)| (*count, symbol)).collect();

    used.sort_unstable();

    let weights: Vec<u64> = used.iter().map(|(count, _)| u64::from(*count)).collect();

    let given = match weights.as_slice() {
        [] => Vec::new(),
        [_] => vec![1u8],
        [_, _, ..] => {
            let Ok(depths) = depths(&weights);
            let Ok(limited) = limited(&depths, longest);

            limited
        },
    };

    let mut lengths = vec![0u8; counts.len()];

    for ((_, symbol), length) in used.iter().zip(given) {
        let Ok(at) = index(*symbol);

        match lengths.get_mut(at) {
            Some(slot) => *slot = length,
            None => {},
        }
    }

    Ok(lengths)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Node {
    Leaf(u32),
    Joined(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fronts {
    leaf: u32,
    joined: u32,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Taken {
    node: Node,
    weight: u64,
    fronts: Fronts,
}

fn lightest(leaves: &[u64], joined: &[(u64, [Node; 2])], fronts: Fronts) -> Result<Option<Taken>, Never> {
    let Ok(leaf) = index(fronts.leaf);
    let Ok(made) = index(fronts.joined);
    let past_the_leaf = Fronts { leaf: fronts.leaf.saturating_add(1), joined: fronts.joined };
    let past_the_joined = Fronts { leaf: fronts.leaf, joined: fronts.joined.saturating_add(1) };
    let a_leaf = |weight: u64| Taken { node: Node::Leaf(fronts.leaf), weight, fronts: past_the_leaf };
    let a_join = |weight: u64| Taken { node: Node::Joined(fronts.joined), weight, fronts: past_the_joined };

    Ok(match (leaves.get(leaf), joined.get(made)) {
        (Some(one), Some((other, _))) => match one <= other {
            true => Some(a_leaf(*one)),
            false => Some(a_join(*other)),
        },
        (Some(one), None) => Some(a_leaf(*one)),
        (None, Some((other, _))) => Some(a_join(*other)),
        (None, None) => None,
    })
}

fn two_lightest(leaves: &[u64], joined: &[(u64, [Node; 2])], fronts: Fronts) -> Result<Option<[Taken; 2]>, Never> {
    let Ok(first) = lightest(leaves, joined, fronts);

    let first = match first {
        Some(first) => first,
        None => return Ok(None),
    };

    let Ok(second) = lightest(leaves, joined, first.fronts);

    Ok(second.map(|second| [first, second]))
}

fn joined(leaves: &[u64]) -> Result<Vec<(u64, [Node; 2])>, Never> {
    let mut joined: Vec<(u64, [Node; 2])> = Vec::with_capacity(leaves.len());
    let mut fronts = Fronts { leaf: 0, joined: 0 };

    for _join in leaves.iter().skip(1) {
        let Ok(two) = two_lightest(leaves, &joined, fronts);

        match two {
            Some([first, second]) => {
                joined.push((first.weight.saturating_add(second.weight), [first.node, second.node]));
                fronts = second.fronts;
            },
            None => {},
        }
    }

    Ok(joined)
}

fn depths(leaves: &[u64]) -> Result<Vec<u32>, Never> {
    let Ok(joined) = joined(leaves);
    let Ok(count) = fitted::<_, u32>(joined.len());
    let mut leaf_depths = vec![0u32; leaves.len()];
    let mut joined_depths = vec![0u32; joined.len()];

    for (made, (_, children)) in (0..count).rev().zip(joined.iter().rev()) {
        let Ok(at) = index(made);

        let below = match joined_depths.get(at) {
            Some(here) => here.saturating_add(1),
            None => 0,
        };

        for child in children {
            let slot = match *child {
                Node::Leaf(leaf) => {
                    let Ok(at) = index(leaf);

                    leaf_depths.get_mut(at)
                },
                Node::Joined(made) => {
                    let Ok(at) = index(made);

                    joined_depths.get_mut(at)
                },
            };

            match slot {
                Some(depth) => *depth = below,
                None => {},
            }
        }
    }

    Ok(leaf_depths)
}

fn limited(depths: &[u32], longest: u8) -> Result<Vec<u8>, Never> {
    let ceiling = u32::from(longest).clamp(1, DEEPEST);
    let mut count = [0u32; 16];

    for depth in depths {
        let Ok(at) = index((*depth).min(ceiling));

        match count.get_mut(at) {
            Some(counted) => *counted = counted.saturating_add(1),
            None => {},
        }
    }

    let total = (0u32..).zip(count).fold(0u64, |total, (length, counted)| {
        total.saturating_add(u64::from(counted).wrapping_shl(ceiling.saturating_sub(length)))
    });

    for _over in 0..total.saturating_sub(1u64.wrapping_shl(ceiling)) {
        let Ok(()) = moved_up(&mut count, ceiling);
    }

    let mut lengths = Vec::with_capacity(depths.len());

    for (length, counted) in (0u8..16).zip(count).rev() {
        let Ok(times) = index(counted);

        lengths.extend(std::iter::repeat_n(length, times));
    }

    Ok(lengths)
}

fn moved_up(count: &mut [u32; 16], ceiling: u32) -> Result<(), Never> {
    let deepest_above = (1..ceiling).rev().find(|length| {
        let Ok(at) = index(*length);

        matches!(count.get(at), Some(1..))
    });

    let split = match deepest_above {
        Some(split) => split,
        None => return Ok(()),
    };

    for (length, change) in [(ceiling, Change::Less), (split, Change::Less), (split.saturating_add(1), Change::Two)] {
        let Ok(at) = index(length);

        match (count.get_mut(at), change) {
            (Some(counted), Change::Less) => *counted = counted.saturating_sub(1),
            (Some(counted), Change::Two) => *counted = counted.saturating_add(2),
            (None, Change::Less | Change::Two) => {},
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Less,
    Two,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bits, Code};

    fn strings_used(lengths: &[u8]) -> Result<u64, Never> {
        Ok(lengths.iter().filter(|length| **length > 0).fold(0u64, |used, length| {
            used.saturating_add(1u64.wrapping_shl(DEEPEST.saturating_sub(u32::from(*length))))
        }))
    }

    #[test]
    fn fields_go_in_from_the_low_end_and_come_back_out_the_same_way() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(mut writer) = BitWriter::new();
        let fields = [(1, 1), (0b10, 2), (0x5A5A, 16), (0x1234_5678, 32), (0b101, 3)];

        for (value, width) in fields {
            let Ok(()) = writer.put(Field { value, width });
        }

        let Ok(written) = writer.finished();
        let mut bits = Bits::new(&written)?;

        for (value, width) in fields {
            assert_eq!(bits.take(width), Ok(value));
        }

        Ok(())
    }

    #[test]
    fn aligning_pads_with_zeros_to_the_next_byte() {
        let Ok(mut writer) = BitWriter::new();
        let Ok(()) = writer.put(Field { value: 0b111, width: 3 });
        let Ok(()) = writer.copied(&[0xAB, 0xCD]);

        assert_eq!(writer.finished(), Ok(vec![0b111, 0xAB, 0xCD]));
    }

    #[test]
    fn the_commonest_symbol_gets_the_shortest_code() {
        assert_eq!(lengths(&[45, 13, 12, 16, 9, 5], 15), Ok(vec![1, 3, 3, 3, 4, 4]));
    }

    #[test]
    fn what_is_never_written_has_no_code_and_one_symbol_alone_has_one_bit() {
        assert_eq!(lengths(&[0, 0, 0], 15), Ok(vec![0, 0, 0]));
        assert_eq!(lengths(&[0, 7, 0], 15), Ok(vec![0, 1, 0]));
    }

    #[test]
    fn a_tree_deeper_than_the_ceiling_is_cut_back_to_a_complete_code_under_it() {
        let fibonacci: Vec<u32> = (0..30)
            .scan((1u32, 1u32), |(a, b), _| {
                let next = *a;

                (*a, *b) = (*b, a.saturating_add(*b));

                Some(next)
            })
            .collect();

        for longest in [7u8, 9, 15] {
            let Ok(cut) = lengths(&fibonacci, longest);

            assert!(cut.iter().all(|length| (1..=longest).contains(length)), "{longest}: {cut:?}");
            assert_eq!(strings_used(&cut), Ok(1u64.wrapping_shl(DEEPEST)), "{longest}: {cut:?}");
        }
    }

    #[test]
    fn what_a_codebook_writes_the_reading_code_reads_back() -> Result<(), Box<dyn std::error::Error>> {
        let message: Vec<u16> = (0u32..2000)
            .map(|at| {
                let [low, ..] = at.wrapping_mul(at).wrapping_shr(3).to_le_bytes();

                u16::from(low)
            })
            .collect();
        let mut counts = vec![0u32; 256];

        for symbol in &message {
            let Ok(at) = index(*symbol);

            match counts.get_mut(at) {
                Some(count) => *count = count.saturating_add(1),
                None => {},
            }
        }

        let book = Codebook::from_counts(&counts, 15)?;
        let Ok(mut writer) = BitWriter::new();

        for symbol in &message {
            book.put(&mut writer, *symbol)?;
        }

        let Ok(written) = writer.finished();
        let Ok(lengths) = book.lengths();
        let code = Code::partial(lengths)?;
        let mut bits = Bits::new(&written)?;

        for symbol in &message {
            let Ok(()) = bits.fill();

            assert_eq!(code.decoded(&mut bits), Ok(*symbol));
        }

        let Ok(cost) = book.cost(&counts);
        let Ok(long) = fitted::<_, u64>(written.len());

        assert_eq!(cost.div_ceil(8), long);

        Ok(())
    }

    #[test]
    fn a_symbol_with_no_code_is_refused_rather_than_written_as_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let book = Codebook::from_counts(&[3, 0, 5], 15)?;
        let Ok(mut writer) = BitWriter::new();

        assert_eq!(book.put(&mut writer, 1), Err(CodeError::Uncoded));
        assert_eq!(book.put(&mut writer, 9), Err(CodeError::Uncoded));

        Ok(())
    }
}
