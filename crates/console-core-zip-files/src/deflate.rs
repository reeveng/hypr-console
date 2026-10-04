//! Deflate, done: bytes made into an RFC 1951 stream that [`crate::inflate`]
//! reads back.
//!
//! A PNG is written as one deflate stream, and a picture this desktop saves --
//! a thumbnail, a photograph turned the right way up -- is written here rather
//! than handed to ffmpeg to write. What is done is zlib's sixth level, which is
//! the one everything defaults to. Every place three bytes begin is filed
//! under a hash of them, each hash keeping a chain of the places before it in
//! the last thirty-two kilobytes, and the longest run that matches what comes
//! next is copied rather than written again: the chain is walked a hundred and
//! twenty-eight places back at most, and no further once a run of a hundred
//! and twenty-eight is found. A short run is not taken straight away. The
//! place after it is asked as well, and if a longer run begins there the byte
//! in between is written as itself and the longer run is taken instead, which
//! is zlib's lazy matching and is most of what its sixth level has over its
//! first. Nor is a run of three taken from more than four kilobytes back,
//! which zlib does not take either: saying that far costs more than the three
//! bytes would.
//!
//! A run is measured eight bytes at a time and only the last eight a byte at a
//! time, and a place is not measured at all unless the byte that would make
//! it longer than the best so far matches, which is zlib's own shortcut.
//!
//! What comes out is cut into blocks of sixteen thousand symbols, and each
//! block is written whichever of the format's three ways is shortest: as the
//! bytes it stands for, in the code the format fixes, or in a code made for it
//! from how often each symbol comes, which is sent ahead of it in a code of its
//! own. All three are counted before anything is written, so the choice is
//! exact rather than guessed.

use std::ops::ControlFlow;

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_prefix_codes::{BitWriter, Codebook, Field};

use crate::ZipError;
use crate::inflate::{
    BackReference, DISTANCE_BASE, DISTANCE_EXTRA, END_OF_BLOCK, FIRST_LENGTH, FIXED_DISTANCES, LENGTH_BASE,
    LENGTH_EXTRA, LENGTHS_IN_ORDER, fixed_lengths,
};

const WINDOW: u32 = 32_768;

const HASH_BITS: u32 = 15;

const HASH_SHIFT: u32 = 17;

const FIBONACCI: u32 = 0x9E37_79B1;

const SHORTEST: u32 = 3;

const LONGEST: u32 = 258;

const TRIED: u32 = 128;

const GOOD_ENOUGH: u32 = 128;

const LAZY_BELOW: u32 = 32;

const TOO_FAR: u32 = 4096;

const SYMBOLS_A_BLOCK: u32 = 16_384;

const STORED_AT_MOST: u32 = 65_535;

const STORED_AROUND: u64 = 3 + 7 + 32;

const LITERALS_AND_LENGTHS: u32 = 286;

const LONGEST_CODE: u8 = 15;

const LONGEST_LENGTH_CODE: u8 = 7;

const FEWEST_LITERALS_AND_LENGTHS: u32 = 257;

const FEWEST_LENGTH_CODES: u32 = 4;

const BLOCK_HEADER: u64 = 3;

const CODE_COUNTS: u64 = 5 + 5 + 4;

const LENGTH_CODE_WIDTH: u32 = 3;

const REPEAT_LAST: u8 = 16;

const REPEAT_ZERO: u8 = 17;

const REPEAT_ZERO_LONG: u8 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Literal(u8),
    Copy(BackReference),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Continues,
    Last,
}

struct Window {
    head: Vec<Option<u32>>,
    chain: Vec<Option<u32>>,
}

struct Books {
    lengths: Codebook,
    distances: Codebook,
}

struct Alphabet {
    length_codes: Vec<u8>,
    distance_codes: Vec<u8>,
    fixed: Books,
}

struct Coded {
    symbol: u16,
    extra: Field,
}

struct Counts {
    lengths: Vec<u32>,
    distances: Vec<u32>,
    extra: u64,
}

struct Header {
    books: Books,
    short: Codebook,
    said: Vec<(u8, u8)>,
    literals: u32,
    distances: u32,
    shorts: u32,
}

enum Kind {
    Stored,
    Fixed,
    Own(Header),
}

struct Deflating {
    at: u32,
    pending: Option<BackReference>,
    window: Window,
    tokens: Vec<Token>,
    from: u32,
    writer: BitWriter,
}

struct Chosen {
    token: Token,
    pending: Option<BackReference>,
    past: u32,
}

pub fn deflate(bytes: &[u8]) -> Result<Vec<u8>, ZipError> {
    let end = match u32::try_from(bytes.len()) {
        Ok(end) => end,
        Err(_more_than_four_gigabytes) => return Err(ZipError::TooLarge),
    };

    let alphabet = alphabet()?;
    let Ok(window) = Window::new();
    let Ok(writer) = BitWriter::new();
    let Ok(room) = index(SYMBOLS_A_BLOCK);
    let started = Deflating { at: 0, pending: None, window, tokens: Vec::with_capacity(room), from: 0, writer };

    let walked = iterate(started, |state| {
        Ok(match state.at < end {
            false => Step::Halt(Ok(state)),
            true => match advanced(bytes, state, &alphabet) {
                Ok(state) => Step::Again(state),
                Err(fault) => Step::Halt(Err(fault)),
            },
        })
    });

    let mut ended = match walked {
        Ok(walked) => walked?,
        Err(Endless) => return Err(ZipError::Corrupt),
    };

    let raw = span(bytes, (ended.from, end))?;

    block(&mut ended.writer, (&ended.tokens, raw), Block::Last, &alphabet)?;

    let Ok(written) = ended.writer.finished();

    Ok(written)
}

fn span(bytes: &[u8], range: (u32, u32)) -> Result<&[u8], ZipError> {
    let (from, to) = range;
    let Ok(from) = index(from);
    let Ok(to) = index(to);

    match bytes.get(from..to) {
        Some(span) => Ok(span),
        None => Err(ZipError::Corrupt),
    }
}

fn advanced(bytes: &[u8], state: Deflating, alphabet: &Alphabet) -> Result<Deflating, ZipError> {
    let Deflating { at, pending, mut window, mut tokens, mut from, mut writer } = state;

    let found = match pending {
        Some(found) => Some(found),
        None => {
            let Ok(found) = window.longest(bytes, at);
            let Ok(()) = window.inserted(bytes, at);

            found
        },
    };

    let chosen = chosen(bytes, &mut window, (at, found))?;
    let Ok(held) = fitted::<_, u32>(tokens.len());

    tokens.push(chosen.token);

    match held.saturating_add(1) >= SYMBOLS_A_BLOCK {
        true => {
            let raw = span(bytes, (from, chosen.past))?;

            block(&mut writer, (&tokens, raw), Block::Continues, alphabet)?;
            tokens.clear();
            from = chosen.past;
        },
        false => {},
    }

    Ok(Deflating { at: chosen.past, pending: chosen.pending, window, tokens, from, writer })
}

fn chosen(bytes: &[u8], window: &mut Window, here: (u32, Option<BackReference>)) -> Result<Chosen, ZipError> {
    let (at, found) = here;
    let Ok(byte) = index(at);

    let literal = match bytes.get(byte) {
        Some(byte) => Token::Literal(*byte),
        None => return Err(ZipError::Corrupt),
    };

    let next = at.saturating_add(1);

    let found = match found {
        Some(found) => found,
        None => return Ok(Chosen { token: literal, pending: None, past: next }),
    };

    let (ahead, unfiled) = match found.length < LAZY_BELOW {
        true => {
            let Ok(ahead) = window.longest(bytes, next);
            let Ok(()) = window.inserted(bytes, next);

            (ahead, next.saturating_add(1))
        },
        false => (None, next),
    };

    match ahead.map(|ahead| ahead.length > found.length) {
        Some(true) => return Ok(Chosen { token: literal, pending: ahead, past: next }),
        Some(false) | None => {},
    }

    let past = at.saturating_add(found.length);

    for skipped in unfiled..past {
        let Ok(()) = window.inserted(bytes, skipped);
    }

    Ok(Chosen { token: Token::Copy(found), pending: None, past })
}

#[inline(always)]
fn hashed(bytes: &[u8], at: u32) -> Result<Option<u32>, Never> {
    let Ok(from) = index(at);

    Ok(match bytes.get(from..).and_then(<[u8]>::first_chunk::<3>) {
        Some([first, second, third]) => {
            let joined = u32::from_le_bytes([*first, *second, *third, 0]);

            Some(joined.wrapping_mul(FIBONACCI).wrapping_shr(HASH_SHIFT))
        },
        None => None,
    })
}

#[inline(always)]
fn reach(pair: (&[u8], &[u8]), beat: u32) -> Result<Option<u32>, Never> {
    let (ahead, behind) = pair;
    let Ok(beyond) = index(beat);

    match (ahead.get(beyond), behind.get(beyond)) {
        (Some(one), Some(other)) => match one == other {
            true => {},
            false => return Ok(None),
        },
        (Some(_), None) | (None, _) => return Ok(None),
    }

    let (ahead_words, _) = ahead.as_chunks::<8>();
    let (behind_words, _) = behind.as_chunks::<8>();
    let Ok(words) = index(LONGEST.div_ceil(8));
    let Ok(same_words) = fitted::<_, u32>(ahead_words.iter().zip(behind_words).take(words).take_while(|(one, other)| one == other).count());
    let compared = same_words.saturating_mul(8);
    let Ok(from) = index(compared);
    let Ok(left) = index(LONGEST.saturating_sub(compared));

    let same = match (ahead.get(from..), behind.get(from..)) {
        (Some(ahead), Some(behind)) => {
            let Ok(same) = fitted::<_, u32>(ahead.iter().zip(behind).take(left).take_while(|(one, other)| one == other).count());

            same
        },
        (Some(_), None) | (None, _) => 0,
    };

    Ok(Some(compared.saturating_add(same).min(LONGEST)))
}

impl Window {
    fn new() -> Result<Window, Never> {
        let Ok(heads) = index(1u32.wrapping_shl(HASH_BITS));
        let Ok(places) = index(WINDOW);

        Ok(Window { head: vec![None; heads], chain: vec![None; places] })
    }

    #[inline(always)]
    fn inserted(&mut self, bytes: &[u8], at: u32) -> Result<(), Never> {
        let Ok(hash) = hashed(bytes, at);
        let Ok(link) = index(at & WINDOW.wrapping_sub(1));

        let hash = match hash {
            Some(hash) => hash,
            None => return Ok(()),
        };

        let Ok(slot) = index(hash);

        match (self.head.get_mut(slot), self.chain.get_mut(link)) {
            (Some(head), Some(link)) => {
                *link = *head;
                *head = Some(at);
            },
            (Some(_), None) | (None, _) => {},
        }

        Ok(())
    }

    #[inline(always)]
    fn earlier(&self, place: u32) -> Result<Option<u32>, Never> {
        let Ok(link) = index(place & WINDOW.wrapping_sub(1));

        Ok(match self.chain.get(link) {
            Some(Some(before)) => match *before < place {
                true => Some(*before),
                false => None,
            },
            Some(None) | None => None,
        })
    }

    fn longest(&self, bytes: &[u8], at: u32) -> Result<Option<BackReference>, Never> {
        let Ok(hash) = hashed(bytes, at);
        let Ok(from) = index(at);
        let Ok(tried) = index(TRIED);

        let first = match hash {
            Some(hash) => {
                let Ok(slot) = index(hash);

                self.head.get(slot).copied().flatten()
            },
            None => None,
        };

        let ahead = match bytes.get(from..) {
            Some(ahead) => ahead,
            None => return Ok(None),
        };

        let flow = std::iter::successors(first, |place| {
            let Ok(before) = self.earlier(*place);

            before
        })
        .take_while(|place| *place < at && at.saturating_sub(*place) <= WINDOW)
        .take(tried)
        .try_fold(None, |best: Option<BackReference>, place| {
            let Ok(better) = better(bytes, (ahead, at), (place, best));

            match better.map(|better| better.length >= GOOD_ENOUGH) {
                Some(true) => ControlFlow::Break(better),
                Some(false) | None => ControlFlow::Continue(better),
            }
        });

        let best = match flow {
            ControlFlow::Break(best) | ControlFlow::Continue(best) => best,
        };

        Ok(best.filter(|best| best.length > SHORTEST || best.distance <= TOO_FAR))
    }
}

#[inline(always)]
fn better(bytes: &[u8], here: (&[u8], u32), tried: (u32, Option<BackReference>)) -> Result<Option<BackReference>, Never> {
    let (ahead, at) = here;
    let (place, best) = tried;
    let Ok(from) = index(place);

    let beat = match best {
        Some(best) => best.length,
        None => SHORTEST.saturating_sub(1),
    };

    let behind = match bytes.get(from..) {
        Some(behind) => behind,
        None => return Ok(best),
    };

    let Ok(reached) = reach((ahead, behind), beat);

    Ok(match reached {
        Some(length) => match length > beat {
            true => Some(BackReference { distance: at.saturating_sub(place), length }),
            false => best,
        },
        None => best,
    })
}

fn alphabet() -> Result<Alphabet, ZipError> {
    let Ok(length_codes) = codes((&LENGTH_BASE, &LENGTH_EXTRA), LONGEST);
    let Ok(distance_codes) = codes((&DISTANCE_BASE, &DISTANCE_EXTRA), WINDOW);
    let Ok(lengths) = fixed_lengths();
    let lengths = book(Codebook::from_lengths(&lengths))?;
    let distances = book(Codebook::from_lengths(&FIXED_DISTANCES))?;

    Ok(Alphabet { length_codes, distance_codes, fixed: Books { lengths, distances } })
}

fn book<Fault>(made: Result<Codebook, Fault>) -> Result<Codebook, ZipError> {
    match made {
        Ok(book) => Ok(book),
        Err(_uncoded) => Err(ZipError::Uncoded),
    }
}

fn codes(table: (&[u16], &[u8]), most: u32) -> Result<Vec<u8>, Never> {
    let (bases, extras) = table;
    let Ok(room) = index(most.saturating_add(1));
    let mut codes = vec![0u8; room];

    for (code, (base, extra)) in (0u8..).zip(bases.iter().zip(extras)) {
        let Ok(from) = index(*base);
        let Ok(many) = index(1u32.wrapping_shl(u32::from(*extra)));

        for slot in codes.iter_mut().skip(from).take(many) {
            *slot = code;
        }
    }

    Ok(codes)
}

fn coded(codes: &[u8], table: (&[u16], &[u8]), value: u32) -> Result<Coded, ZipError> {
    let (bases, extras) = table;
    let Ok(at) = index(value);

    let code = match codes.get(at) {
        Some(code) => *code,
        None => return Err(ZipError::Uncoded),
    };

    let Ok(slot) = index(code);

    match (bases.get(slot), extras.get(slot)) {
        (Some(base), Some(extra)) => Ok(Coded {
            symbol: u16::from(code),
            extra: Field { value: value.saturating_sub(u32::from(*base)), width: u32::from(*extra) },
        }),
        (None, _) | (_, None) => Err(ZipError::Uncoded),
    }
}

fn copy_coded(alphabet: &Alphabet, reference: BackReference) -> Result<[Coded; 2], ZipError> {
    let mut length = coded(&alphabet.length_codes, (&LENGTH_BASE, &LENGTH_EXTRA), reference.length)?;
    let distance = coded(&alphabet.distance_codes, (&DISTANCE_BASE, &DISTANCE_EXTRA), reference.distance)?;

    length.symbol = length.symbol.saturating_add(FIRST_LENGTH);

    Ok([length, distance])
}

fn tallied(counts: &mut [u32], symbol: u16) -> Result<(), ZipError> {
    let Ok(at) = index(symbol);

    match counts.get_mut(at) {
        Some(count) => {
            *count = count.saturating_add(1);

            Ok(())
        },
        None => Err(ZipError::Uncoded),
    }
}

fn counted(tokens: &[Token], alphabet: &Alphabet) -> Result<Counts, ZipError> {
    let Ok(lengths) = index(LITERALS_AND_LENGTHS);
    let mut counts = Counts { lengths: vec![0; lengths], distances: vec![0; DISTANCE_BASE.len()], extra: 0 };

    for token in tokens {
        match token {
            Token::Literal(byte) => tallied(&mut counts.lengths, u16::from(*byte))?,
            Token::Copy(reference) => {
                let [length, distance] = copy_coded(alphabet, *reference)?;

                tallied(&mut counts.lengths, length.symbol)?;
                tallied(&mut counts.distances, distance.symbol)?;
                counts.extra = counts.extra.saturating_add(u64::from(length.extra.width)).saturating_add(u64::from(distance.extra.width));
            },
        }
    }

    tallied(&mut counts.lengths, END_OF_BLOCK)?;

    Ok(counts)
}

fn put(book: &Codebook, writer: &mut BitWriter, symbol: u16) -> Result<(), ZipError> {
    match book.put(writer, symbol) {
        Ok(()) => Ok(()),
        Err(_uncoded) => Err(ZipError::Uncoded),
    }
}

fn tokens_put(writer: &mut BitWriter, tokens: &[Token], books: &Books, alphabet: &Alphabet) -> Result<(), ZipError> {
    for token in tokens {
        match token {
            Token::Literal(byte) => put(&books.lengths, writer, u16::from(*byte))?,
            Token::Copy(reference) => {
                let [length, distance] = copy_coded(alphabet, *reference)?;

                put(&books.lengths, writer, length.symbol)?;

                let Ok(()) = writer.put(length.extra);

                put(&books.distances, writer, distance.symbol)?;

                let Ok(()) = writer.put(distance.extra);
            },
        }
    }

    put(&books.lengths, writer, END_OF_BLOCK)
}

fn used(lengths: &[u8], fewest: u32) -> Result<u32, Never> {
    let after_the_last = (1u32..).zip(lengths).fold(0u32, |after, (count, length)| match length {
        0 => after,
        1.. => count,
    });

    Ok(after_the_last.max(fewest))
}

fn header(counts: &Counts) -> Result<Header, ZipError> {
    let lengths = book(Codebook::from_counts(&counts.lengths, LONGEST_CODE))?;
    let mut distance_counts = counts.distances.clone();

    match distance_counts.iter().all(|count| *count == 0) {
        true => tallied(&mut distance_counts, 0)?,
        false => {},
    }

    let distances = book(Codebook::from_counts(&distance_counts, LONGEST_CODE))?;
    let Ok(literal_lengths) = lengths.lengths();
    let Ok(distance_lengths) = distances.lengths();
    let Ok(literals) = used(literal_lengths, FEWEST_LITERALS_AND_LENGTHS);
    let Ok(far) = used(distance_lengths, 1);
    let Ok(literals_sent) = index(literals);
    let Ok(far_sent) = index(far);
    let sent: Vec<u8> = literal_lengths.iter().take(literals_sent).chain(distance_lengths.iter().take(far_sent)).copied().collect();
    let Ok(said) = runs(&sent);
    let mut short_counts = vec![0u32; LENGTHS_IN_ORDER.len()];

    for (symbol, _) in &said {
        tallied(&mut short_counts, u16::from(*symbol))?;
    }

    let short = book(Codebook::from_counts(&short_counts, LONGEST_LENGTH_CODE))?;
    let Ok(short_lengths) = short.lengths();
    let in_order: Vec<u8> = LENGTHS_IN_ORDER.iter().map(|symbol| {
        let Ok(at) = index(*symbol);

        match short_lengths.get(at) {
            Some(length) => *length,
            None => 0,
        }
    }).collect();
    let Ok(shorts) = used(&in_order, FEWEST_LENGTH_CODES);

    Ok(Header { books: Books { lengths, distances }, short, said, literals, distances: far, shorts })
}

fn runs(lengths: &[u8]) -> Result<Vec<(u8, u8)>, Never> {
    let mut said = Vec::new();

    for run in lengths.chunk_by(|one, other| one == other) {
        let Ok(long) = fitted::<_, u32>(run.len());

        match run.first() {
            Some(0) => {
                let Ok(()) = zeros(&mut said, long);
            },
            Some(length) => {
                let Ok(()) = repeated(&mut said, (*length, long));
            },
            None => {},
        }
    }

    Ok(said)
}

fn zeros(said: &mut Vec<(u8, u8)>, long: u32) -> Result<(), Never> {
    let Ok(full) = index(long.div_euclid(138));

    said.extend(std::iter::repeat_n((REPEAT_ZERO_LONG, 127), full));

    let rest = long.rem_euclid(138);
    let Ok(left) = index(rest);
    let Ok(rest) = fitted::<u32, u8>(rest);

    match rest {
        0..=2 => said.extend(std::iter::repeat_n((0, 0), left)),
        3..=10 => said.push((REPEAT_ZERO, rest.saturating_sub(3))),
        11.. => said.push((REPEAT_ZERO_LONG, rest.saturating_sub(11))),
    }

    Ok(())
}

fn repeated(said: &mut Vec<(u8, u8)>, run: (u8, u32)) -> Result<(), Never> {
    let (length, long) = run;
    let more = long.saturating_sub(1);
    let Ok(full) = index(more.div_euclid(6));

    said.push((length, 0));
    said.extend(std::iter::repeat_n((REPEAT_LAST, 3), full));

    let rest = more.rem_euclid(6);
    let Ok(left) = index(rest);
    let Ok(rest) = fitted::<u32, u8>(rest);

    match rest {
        0..=2 => said.extend(std::iter::repeat_n((length, 0), left)),
        3.. => said.push((REPEAT_LAST, rest.saturating_sub(3))),
    }

    Ok(())
}

fn extra_width(symbol: u8) -> Result<u32, Never> {
    Ok(match symbol {
        REPEAT_LAST => 2,
        REPEAT_ZERO => 3,
        REPEAT_ZERO_LONG => 7,
        _ => 0,
    })
}

fn header_cost(header: &Header) -> Result<u64, Never> {
    let Ok(lengths) = header.short.lengths();

    let said = header.said.iter().fold(0u64, |bits, (symbol, _)| {
        let Ok(at) = index(*symbol);
        let Ok(extra) = extra_width(*symbol);

        let length = match lengths.get(at) {
            Some(length) => *length,
            None => 0,
        };

        bits.saturating_add(u64::from(length)).saturating_add(u64::from(extra))
    });

    Ok(CODE_COUNTS.saturating_add(u64::from(header.shorts).saturating_mul(u64::from(LENGTH_CODE_WIDTH))).saturating_add(said))
}

fn header_put(writer: &mut BitWriter, header: &Header) -> Result<(), ZipError> {
    let Ok(lengths) = header.short.lengths();
    let Ok(shorts) = index(header.shorts);

    for value in [
        (header.literals.saturating_sub(FEWEST_LITERALS_AND_LENGTHS), 5),
        (header.distances.saturating_sub(1), 5),
        (header.shorts.saturating_sub(FEWEST_LENGTH_CODES), 4),
    ] {
        let (value, width) = value;
        let Ok(()) = writer.put(Field { value, width });
    }

    for symbol in LENGTHS_IN_ORDER.iter().take(shorts) {
        let Ok(at) = index(*symbol);

        let length = match lengths.get(at) {
            Some(length) => *length,
            None => 0,
        };

        let Ok(()) = writer.put(Field { value: u32::from(length), width: LENGTH_CODE_WIDTH });
    }

    for (symbol, extra) in &header.said {
        put(&header.short, writer, u16::from(*symbol))?;

        let Ok(width) = extra_width(*symbol);
        let Ok(()) = writer.put(Field { value: u32::from(*extra), width });
    }

    Ok(())
}

fn stored_cost(raw: &[u8]) -> Result<Option<u64>, Never> {
    let Ok(long) = fitted::<_, u64>(raw.len());
    let pieces = long.div_ceil(u64::from(STORED_AT_MOST));

    Ok(match pieces {
        0 => None,
        1.. => Some(pieces.saturating_mul(STORED_AROUND).saturating_add(long.saturating_mul(8))),
    })
}

fn kind(counts: &Counts, raw: &[u8], alphabet: &Alphabet) -> Result<Kind, ZipError> {
    let header = header(counts)?;
    let Ok(header_bits) = header_cost(&header);
    let Ok(own_lengths) = header.books.lengths.cost(&counts.lengths);
    let Ok(own_distances) = header.books.distances.cost(&counts.distances);
    let Ok(fixed_lengths) = alphabet.fixed.lengths.cost(&counts.lengths);
    let Ok(fixed_distances) = alphabet.fixed.distances.cost(&counts.distances);
    let Ok(stored) = stored_cost(raw);
    let own = header_bits.saturating_add(own_lengths).saturating_add(own_distances);
    let fixed = fixed_lengths.saturating_add(fixed_distances);
    let coded = own.min(fixed).saturating_add(counts.extra).saturating_add(BLOCK_HEADER);

    Ok(match (stored.map(|stored| stored <= coded), own <= fixed) {
        (Some(true), _) => Kind::Stored,
        (Some(false) | None, true) => Kind::Own(header),
        (Some(false) | None, false) => Kind::Fixed,
    })
}

fn opened(writer: &mut BitWriter, last: Block, kind: u32) -> Result<(), Never> {
    let closing = match last {
        Block::Continues => 0,
        Block::Last => 1,
    };

    let Ok(()) = writer.put(Field { value: closing, width: 1 });

    writer.put(Field { value: kind, width: 2 })
}

fn block(writer: &mut BitWriter, pending: (&[Token], &[u8]), last: Block, alphabet: &Alphabet) -> Result<(), ZipError> {
    let (tokens, raw) = pending;
    let counts = counted(tokens, alphabet)?;
    let kind = kind(&counts, raw, alphabet)?;

    match kind {
        Kind::Stored => {
            let Ok(()) = stored(writer, raw, last);
        },
        Kind::Fixed => {
            let Ok(()) = opened(writer, last, 1);

            tokens_put(writer, tokens, &alphabet.fixed, alphabet)?;
        },
        Kind::Own(header) => {
            let Ok(()) = opened(writer, last, 2);

            header_put(writer, &header)?;
            tokens_put(writer, tokens, &header.books, alphabet)?;
        },
    }

    Ok(())
}

fn stored(writer: &mut BitWriter, raw: &[u8], last: Block) -> Result<(), Never> {
    let Ok(most) = index(STORED_AT_MOST);
    let pieces = raw.chunks(most);
    let Ok(count) = fitted::<_, u32>(pieces.len());

    for (at, piece) in (1u32..).zip(pieces) {
        let closing = match (last, at == count) {
            (Block::Last, true) => Block::Last,
            (Block::Last, false) | (Block::Continues, _) => Block::Continues,
        };

        let Ok(long) = fitted::<_, u16>(piece.len());
        let [low, high] = long.to_le_bytes();
        let [not_low, not_high] = (!long).to_le_bytes();
        let Ok(()) = opened(writer, closing, 0);
        let Ok(()) = writer.copied(&[low, high, not_low, not_high]);
        let Ok(()) = writer.copied(piece);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate::inflate;
    use std::error::Error;

    fn round_trip(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let packed = deflate(bytes)?;
        let size = u32::try_from(bytes.len())?;
        let unpacked = inflate(&packed, size)?;

        assert!(unpacked.as_slice() == bytes, "{} bytes came back different", bytes.len());

        Ok(packed)
    }

    fn noise(long: u32) -> Result<Vec<u8>, Never> {
        Ok((0..long)
            .scan(0x2545_F491u32, |state, _| {
                *state ^= state.wrapping_shl(13);
                *state ^= state.wrapping_shr(17);
                *state ^= state.wrapping_shl(5);

                let [byte, ..] = state.to_le_bytes();

                Some(byte)
            })
            .collect())
    }

    #[test]
    fn nothing_is_the_shortest_block_there_is() -> Result<(), Box<dyn Error>> {
        let packed = round_trip(b"")?;

        assert_eq!(packed, vec![0x03, 0x00]);

        Ok(())
    }

    #[test]
    fn words_come_back_and_come_out_smaller() -> Result<(), Box<dyn Error>> {
        let words = include_bytes!("../tests/words.txt");
        let packed = round_trip(words)?;

        assert!(packed.len().saturating_mul(2) < words.len(), "{} bytes from {}", packed.len(), words.len());

        Ok(())
    }

    const ZLIB_AT_ITS_SIXTH_LEVEL: u32 = 20_541;

    #[test]
    fn what_zlib_packs_at_the_level_everything_defaults_to_is_packed_no_larger() -> Result<(), Box<dyn Error>> {
        let packed = include_bytes!("../tests/mixed.deflate");
        let unpacked = inflate(packed, 39_330)?;
        let again = round_trip(&unpacked)?;
        let Ok(long) = fitted::<_, u32>(again.len());

        assert!(long <= ZLIB_AT_ITS_SIXTH_LEVEL, "{long} bytes where zlib makes {ZLIB_AT_ITS_SIXTH_LEVEL}");

        Ok(())
    }

    #[test]
    fn noise_is_stored_rather_than_made_larger() -> Result<(), Box<dyn Error>> {
        let Ok(noise) = noise(200_000);
        let packed = round_trip(&noise)?;

        assert!(packed.len() <= noise.len().saturating_add(100), "{} bytes from {}", packed.len(), noise.len());

        Ok(())
    }

    #[test]
    fn a_long_run_is_copies_of_itself_over_and_over() -> Result<(), Box<dyn Error>> {
        let zeros = vec![0u8; 1_000_000];
        let packed = round_trip(&zeros)?;
        let pattern: Vec<u8> = b"abc".iter().copied().cycle().take(100_000).collect();
        let _ = round_trip(&pattern)?;

        assert!(packed.len() < 2_000, "{} bytes", packed.len());

        Ok(())
    }

    #[test]
    fn a_stored_block_longer_than_one_piece_is_cut_into_pieces_inflate_reads() -> Result<(), Box<dyn Error>> {
        let Ok(noise) = noise(150_000);
        let Ok(mut writer) = BitWriter::new();
        let Ok(()) = stored(&mut writer, &noise, Block::Last);
        let Ok(packed) = writer.finished();
        let size = u32::try_from(noise.len())?;

        let unpacked = inflate(&packed, size)?;

        assert!(unpacked == noise);

        Ok(())
    }

    #[test]
    fn noise_with_words_in_it_takes_whichever_block_is_shorter_each_time() -> Result<(), Box<dyn Error>> {
        let Ok(noise) = noise(40_000);
        let words = include_bytes!("../tests/words.txt");
        let mixed: Vec<u8> = noise.iter().chain(words.iter()).chain(noise.iter()).copied().collect();
        let _ = round_trip(&mixed)?;

        Ok(())
    }
}
