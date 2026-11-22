//! `cargo kani -p console-core-state-machine --lib --harness schema`.

use super::*;

fn read_byte_by_byte(decoder: &mut Decoder<'_>) -> Result<Vec<u8>, ParseError> {
  let count = u64::decode(decoder)?;
  let mut bytes = Vec::new();

  for _ in 0..count {
    bytes.push(decoder.byte()?);
  }

  Ok(bytes)
}

#[kani::proof]
#[kani::unwind(15)]
fn a_checked_slice_preserves_the_result_and_the_decoder_position() {
  let bytes: [u8; 12] = kani::any();
  let length = usize::from(kani::any::<u8>());
  kani::assume(length <= bytes.len());
  let input = &bytes[..length];
  let Ok(mut previous) = Decoder::new(input);
  let Ok(mut current) = Decoder::new(input);

  let expected = read_byte_by_byte(&mut previous);
  assert_eq!(current.bytes(), expected);
  assert_eq!(current.rest, previous.rest);
  kani::cover!(expected.as_ref().is_ok_and(|bytes| !bytes.is_empty()));
  kani::cover!(length < 8);
  kani::cover!(length == 12 && bytes[..8] == u64::MAX.to_be_bytes());
}
