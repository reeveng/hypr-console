//! A saved machine: its state as a snapshot, and a snapshot as a state again.
//!
//! A snapshot is a mark, the layout's version, the state, and a CRC-32 over
//! every byte before it. The sum is checked first, so a file cut short by a
//! power cut or flipped by a bad write is met as `Corrupt` before any field is
//! read, and a field that will not decode is a layout question rather than a
//! disk one.
//!
//! A layout changes when a state gains or loses a field, and the version is
//! how a snapshot says which layout wrote it. A machine that changed its layout
//! answers the old one in `migrate`; one that does not answer it is told the
//! version it cannot read, and is initialized as if nothing had been saved --
//! losing a page somebody was on is a smaller fault than refusing to start.

use console_core_checksums::crc32;
use console_core_never::Never;

use crate::Machine;
use crate::schema::{Decoder, Encoder, ParseError, Serializable};

const MARK: [u8; 4] = *b"CSM2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version(pub u8);

pub trait SerializableMachine: Machine<State: Serializable> {
    const VERSION: Version;

    fn migrate(from: Version, decoder: &mut Decoder<'_>) -> Result<Self::State, ParseError> {
        let _unread = decoder;

        Err(ParseError::Version(from))
    }
}

pub fn snapshot<M: SerializableMachine>(state: &M::State) -> Result<Vec<u8>, Never> {
    let Ok(mut encoder) = Encoder::new();
    let Ok(()) = encoder.bytes(&MARK);
    let Ok(()) = encoder.byte(M::VERSION.0);
    let Ok(()) = state.encode(&mut encoder);
    let Ok(mut bytes) = encoder.finish();
    let Ok(sum) = crc32::of(&bytes);

    bytes.extend_from_slice(&sum.to_be_bytes());

    Ok(bytes)
}

pub fn restore<M: SerializableMachine>(bytes: &[u8]) -> Result<M::State, ParseError> {
    let (held, sum) = match bytes.split_last_chunk::<4>() {
        Some(split) => split,
        None => return Err(ParseError::Truncated),
    };
    let Ok(computed) = crc32::of(held);

    match computed == u32::from_be_bytes(*sum) {
        true => {}
        false => return Err(ParseError::Corrupt),
    }

    let Ok(mut decoder) = Decoder::new(held);
    let mark = u32::decode(&mut decoder)?;

    match mark == u32::from_be_bytes(MARK) {
        true => {}
        false => return Err(ParseError::NotASnapshot),
    }

    let written = decoder.byte()?;
    let version = Version(written);
    let state = match version == M::VERSION {
        true => M::State::decode(&mut decoder),
        false => M::migrate(version, &mut decoder),
    }?;

    decoder.finish()?;

    Ok(state)
}
