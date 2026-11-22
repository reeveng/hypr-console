use crate::{Clamper, Interpolation, PdfSample, TupleVec, Values, interpolate};
use console_core_number_conversion::Float;
use console_core_portable_document_syntax::bit_reader::BitReader;
use console_core_portable_document_syntax::object::Array;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::{
  BITS_PER_SAMPLE, DECODE, ENCODE, SIZE,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::num::NonZeroUsize;

#[derive(Debug)]
pub(crate) struct Type0 {
  sizes: IntVec,
  table: HashMap<Key, IntVec>,
  clamper: Clamper,
  bits_per_sample: u8,
  encode: TupleVec,
  decode: TupleVec,
}

impl Type0 {
  pub(crate) fn new(
    stream: &Stream<'_>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(dict) = stream.dict();
    let Ok(sample_bits) = dict.get::<u8>(BITS_PER_SAMPLE);
    let bits_per_sample = match sample_bits {
      Some(value) => value,
      None => return Ok(None),
    };

    match bits_per_sample {
      1 | 2 | 4 | 8 | 16 | 24 | 32 => {}
      _ => {
        #[cfg(feature = "logging")]
        console_core_logging::error!("invalid bits per sample: {bits_per_sample}");

        return Ok(None);
      }
    }

    let Ok(limits) = Clamper::new(dict);
    let clamper = match limits {
      Some(value) => value,
      None => return Ok(None),
    };
    let range = match clamper.range.clone() {
      Some(value) => value,
      None => return Ok(None),
    };

    match range.as_slice() {
      [] => {
        #[cfg(feature = "logging")]
        console_core_logging::warn!("encountered Type0 function with invalid range length 0.");

        return Ok(None);
      }
      [_, ..] => {}
    }

    let Ok(dimensions) = dict.get::<Array<'_>>(SIZE);
    let dimensions_array = match dimensions {
      Some(array) => array,
      None => return Ok(None),
    };
    let Ok(iter) = dimensions_array.iter::<u32>();
    let sizes = iter.collect::<IntVec>();

    match (sizes.as_slice(), sizes.iter().find(|&&size| size == 0)) {
      ([], _) | (_, Some(_)) => return Ok(None),
      ([_, ..], None) => {}
    }

    let Ok(encoding) = dict.get::<TupleVec>(ENCODE);
    let encode = match encoding {
      Some(encode) => encode,
      None => sizes
        .iter()
        .map(|s| {
          let Ok(last) = sample_f32(s.saturating_sub(1));

          (0.0, last)
        })
        .collect(),
    };

    let Ok(decoding) = dict.get::<TupleVec>(DECODE);
    let decode = match decoding {
      Some(decode) => decode,
      None => range.clone(),
    };

    let Ok(channels) = console_core_number_conversion::fitted::<_, u32>(range.len());
    let num_expected_entries = match sizes.iter().try_fold(u64::from(channels), |count, size| {
      count.checked_mul(u64::from(*size))
    }) {
      Some(value) => value,
      None => return Ok(None),
    };

    let decoded = match stream.decoded() {
      Ok(decoded) => decoded,
      Err(_decode_failure) => return Ok(None),
    };
    let Ok(mut reader) = BitReader::new(&decoded);
    let mut data = Vec::new();

    for _ in 0..num_expected_entries {
      let Ok(bits) = reader.read(bits_per_sample);
      let sample = match bits {
        Some(value) => value,
        None => return Ok(None),
      };
      data.push(sample);
    }

    let Ok(samples) = build_table(&data, &sizes, channels);
    let table = match samples {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some(Self {
      sizes,
      clamper,
      bits_per_sample,
      table,
      encode,
      decode,
    }))
  }

  pub(crate) fn eval(
    &self,
    mut input: Values,
  ) -> core::result::Result<Option<Values>, console_core_never::Never> {
    match input.len().cmp(&self.sizes.len()) {
      Ordering::Less | Ordering::Greater => {
        #[cfg(feature = "logging")]
        console_core_logging::warn!("wrong number of arguments for sampled function");

        return Ok(None);
      }
      Ordering::Equal => {}
    }

    let Ok(()) = self.clamper.clamp_input(&mut input);

    let mut key = input;

    for (((x, domain), encode), size) in key
      .iter_mut()
      .zip(self.clamper.domain.iter())
      .zip(self.encode.iter())
      .zip(self.sizes.iter())
    {
      *x = {
        let Ok(explicit_value) = interpolate(
          PdfSample(*x),
          Interpolation {
            domain: [domain.0, domain.1],
            range: [encode.0, encode.1],
          },
        );
        explicit_value
      };
      let Ok(extent) = sample_f32(*size);

      *x = x.max(0.0).min(extent - 1.0);
    }

    let in_prev = key
      .iter()
      .map(|v| {
        let Ok(floor) = console_core_number_conversion::toward_zero_u32(f64::from(v.floor()));

        floor
      })
      .collect::<IntVec>();
    let in_next = key
      .iter()
      .map(|v| {
        let Ok(ceil) = console_core_number_conversion::toward_zero_u32(f64::from(v.ceil()));

        ceil
      })
      .collect::<IntVec>();

    let interpolator = Interpolator {
      input: key.clone().to_vec(),
      in_prev,
      in_next,
      sizes: self.sizes.clone(),
    };

    let Ok(explicit_value) = interpolator.interpolate(&self.table);
    let interpolated = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(largest) = largest_sample(self.bits_per_sample);
    let mut out = interpolated
      .iter()
      .zip(self.decode.iter())
      .map(|(x, decode)| {
        let Ok(decoded) = interpolate(
          PdfSample(*x),
          Interpolation {
            domain: [0.0, largest],
            range: [decode.0, decode.1],
          },
        );

        decoded
      })
      .collect::<Vec<_>>();

    let Ok(()) = self.clamper.clamp_output(&mut out);

    Ok(Some(out))
  }
}

type FloatVec = Vec<f32>;
type IntVec = Vec<u32>;

struct Interpolator {
  input: FloatVec,
  sizes: IntVec,
  in_prev: IntVec,
  in_next: IntVec,
}

impl Interpolator {
  fn interpolate(
    &self,
    table: &HashMap<Key, IntVec>,
  ) -> core::result::Result<Option<FloatVec>, console_core_never::Never> {
    let Ok(corners) = self.corners(table);
    let mut values = match corners {
      Some(value) => value,
      None => return Ok(None),
    };

    for ((x, prev), next) in self
      .input
      .iter()
      .zip(self.in_prev.iter())
      .zip(self.in_next.iter())
      .rev()
    {
      match prev.cmp(next) {
        Ordering::Equal => {}
        Ordering::Less | Ordering::Greater => {
          let Ok(below) = sample_f32(*prev);
          let Ok(above) = sample_f32(*next);
          let (pairs, _unpaired) = values.as_chunks::<2>();

          values = pairs
            .iter()
            .map(|[low, high]| {
              low
                .iter()
                .zip(high.iter())
                .map(|(from, to)| {
                  let Ok(between) = interpolate(
                    PdfSample(*x),
                    Interpolation {
                      domain: [below, above],
                      range: [*from, *to],
                    },
                  );

                  between
                })
                .collect()
            })
            .collect();
        }
      }
    }

    Ok(values.into_iter().next())
  }

  fn corners(
    &self,
    table: &HashMap<Key, IntVec>,
  ) -> core::result::Result<Option<Vec<FloatVec>>, console_core_never::Never> {
    Ok(
      self
        .in_prev
        .iter()
        .zip(self.in_next.iter())
        .fold(vec![IntVec::new()], |corners, (prev, next)| {
          let choices: IntVec = match prev.cmp(next) {
            Ordering::Equal => vec![*prev],
            Ordering::Less | Ordering::Greater => vec![*prev, *next],
          };

          corners
            .into_iter()
            .flat_map(|corner| {
              choices.clone().into_iter().map(move |choice| {
                let mut extended = corner.clone();

                extended.push(choice);

                extended
              })
            })
            .collect()
        })
        .into_iter()
        .map(|corner| {
          let Ok(key) = Key::from_raw(&self.sizes, &corner);

          table.get(&key).map(|sample| {
            sample
              .iter()
              .map(|level| {
                let Ok(value) = sample_f32(*level);

                value
              })
              .collect()
          })
        })
        .collect(),
    )
  }
}

fn sample_f32(sample: u32) -> core::result::Result<f32, console_core_never::Never> {
  console_core_number_conversion::narrow_f32(f64::from(sample))
}

fn largest_sample(bits_per_sample: u8) -> core::result::Result<f32, console_core_never::Never> {
  let Ok(levels) = u64::wrapping_sub(u64::wrapping_shl(1, u32::from(bits_per_sample)), 1).float();

  console_core_number_conversion::narrow_f32(levels)
}

fn build_table(
  data: &[u32],
  sizes: &[u32],
  channels: u32,
) -> core::result::Result<Option<HashMap<Key, IntVec>>, console_core_never::Never> {
  let Ok(mut key) = Key::new(sizes);
  let mut table = HashMap::new();

  let Ok(width) = console_core_number_conversion::index(channels);
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the fixed-width channel count is converted to the slice API's native index, and NonZeroUsize proves chunks_exact cannot divide by zero"
    )
  )]
  let stride = match NonZeroUsize::new(width) {
    Some(stride) => stride,
    None => return Ok(None),
  };

  for (row, b) in data.chunks_exact(stride.get()).enumerate() {
    match row {
      0 => {}
      1.. => {
        let Ok(explicit_value) = key.increment();

        match explicit_value {
          Some(value) => value,
          None => return Ok(None),
        };
      }
    }

    table.insert(key.clone(), b.to_vec());
  }

  Ok(Some(table))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
  sizes: Vec<u32>,
  parts: Vec<u32>,
}

impl Key {
  fn new(sizes: &[u32]) -> core::result::Result<Self, console_core_never::Never> {
    let parts = vec![0; sizes.len()];

    Ok(Self {
      sizes: sizes.to_vec(),
      parts,
    })
  }

  fn from_raw(
    sizes: &[u32],
    parts: &[u32],
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      sizes: sizes.to_vec(),
      parts: parts.to_vec(),
    })
  }

  fn increment(&mut self) -> core::result::Result<Option<()>, console_core_never::Never> {
    for (size, val) in self.sizes.iter().zip(self.parts.iter_mut()) {
      let last = match size.checked_sub(1) {
        Some(value) => value,
        None => return Ok(None),
      };

      match (*val).cmp(&last) {
        Ordering::Equal | Ordering::Greater => {
          *val = 0;
        }
        Ordering::Less => {
          *val = match val.checked_add(1) {
            Some(value) => value,
            None => return Ok(None),
          };
          return Ok(Some(()));
        }
      }
    }

    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use super::Type0;
  use console_core_portable_document_syntax::object::{FromBytes, Stream};

  #[test]
  fn sampled_function_interpolates_between_samples() -> Result<(), crate::TestFailure> {
    let stream = Stream::from_bytes(
            b"<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Length 2 >>\nstream\n\x00\xff\nendstream",
        );
    let found = stream.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Type0::new(&found);
    let function = made.ok_or(crate::TestFailure::MissingValue)?;
    assert_eq!(function.eval(vec![0.5]), Ok(Some(vec![0.5])));

    Ok(())
  }

  #[test]
  fn two_inputs_interpolate_the_last_input_first() -> Result<(), crate::TestFailure> {
    let stream = Stream::from_bytes(
            b"<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1] /Size [2 2] /BitsPerSample 8 /Length 4 >>\nstream\n\x00\xff\x33\x66\nendstream",
        );
    let found = stream.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Type0::new(&found);
    let function = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(
      function.eval(vec![0.5, 0.25]),
      Ok(Some(vec![114.75 / 255.0]))
    );
    assert_eq!(function.eval(vec![1.0, 0.0]), Ok(Some(vec![1.0])));
    assert_eq!(function.eval(vec![0.0, 1.0]), Ok(Some(vec![51.0 / 255.0])));

    Ok(())
  }

  #[test]
  fn thirty_two_bit_samples_span_the_whole_range() -> Result<(), crate::TestFailure> {
    let stream = Stream::from_bytes(
            b"<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 32 /Length 8 >>\nstream\n\x00\x00\x00\x00\xff\xff\xff\xff\nendstream",
        );
    let found = stream.ok_or(crate::TestFailure::MissingValue)?;
    let Ok(made) = Type0::new(&found);
    let function = made.ok_or(crate::TestFailure::MissingValue)?;

    assert_eq!(function.eval(vec![0.0]), Ok(Some(vec![0.0])));
    assert_eq!(function.eval(vec![1.0]), Ok(Some(vec![1.0])));

    Ok(())
  }

  #[test]
  fn zero_sample_extent_is_rejected() -> Result<(), crate::TestFailure> {
    let stream = Stream::from_bytes(
            b"<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [0] /BitsPerSample 8 /Length 1 >>\nstream\n\x00\nendstream",
        );
    let found = stream.ok_or(crate::TestFailure::MissingValue)?;
    assert!(matches!(Type0::new(&found), Ok(None)));

    Ok(())
  }

  #[test]
  fn overflowing_sample_count_is_rejected() -> Result<(), crate::TestFailure> {
    let stream = Stream::from_bytes(
            b"<< /FunctionType 0 /Domain [0 1 0 1 0 1] /Range [0 1] /Size [4294967295 4294967295 4294967295] /BitsPerSample 8 /Length 1 >>\nstream\n\x00\nendstream",
        );
    let found = stream.ok_or(crate::TestFailure::MissingValue)?;
    assert!(matches!(Type0::new(&found), Ok(None)));

    Ok(())
  }

  #[test]
  fn truncated_sample_table_is_rejected() -> Result<(), crate::TestFailure> {
    let stream = Stream::from_bytes(
            b"<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Length 1 >>\nstream\n\x00\nendstream",
        );
    let found = stream.ok_or(crate::TestFailure::MissingValue)?;
    assert!(matches!(Type0::new(&found), Ok(None)));

    Ok(())
  }
}
