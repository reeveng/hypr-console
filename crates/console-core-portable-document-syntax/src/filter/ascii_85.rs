use alloc::vec::Vec;
use console_core_postscript_tokens::decoding;

pub(crate) fn decode(
  data: &[u8],
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let mut decoded = Vec::with_capacity(data.len().wrapping_mul(4).wrapping_div(5));

  match decoding::ascii85(data, &mut decoded) {
    Ok(()) => Ok(Some(decoded)),
    Err(error) => {
      warn!("invalid ASCII85 stream {:?}", error);
      Ok(None)
    }
  }
}
