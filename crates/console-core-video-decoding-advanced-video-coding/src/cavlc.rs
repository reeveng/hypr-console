//! CAVLC residual block parsing (9.2).

use crate::cavlc_tables::*;
use crate::error::{Error, Result, ensure};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_video_decoding_bitstream::{BitReader, FlagBit};
use std::sync::OnceLock;

struct VlcTable {
  entries: Box<[[u16; 32]; 17]>,
}

impl VlcTable {
  fn empty() -> core::result::Result<Self, Never> {
    Ok(Self {
      entries: Box::new([[0; 32]; 17]),
    })
  }

  fn insert(&mut self, code: &str, value: u8) -> core::result::Result<(), Never> {
    let bytes = code.as_bytes();

    match bytes {
      [] => return Ok(()),
      [_, ..] => {}
    }

    let Ok(length) = wrapped::<_, u16>(bytes.len());
    let ::core::result::Result::Ok(zeros) = ::console_core_number_conversion::fixed_index(
      bytes.iter().take_while(|&&byte| byte == b'0').count(),
    );
    let Ok(prefix) = fixed_index(match ::console_core_number_conversion::index(zeros) {
      ::core::result::Result::Ok(native_position) => native_position,
    });
    let encoded = length.wrapping_shl(8) | u16::from(value);

    let Ok(row) = index(prefix);

    match bytes.get(row..) {
      Some([]) => {
        for entries in self.entries.iter_mut().skip(row) {
          entries.fill(encoded);
        }
      }
      Some([_, suffix @ ..]) => {
        let Ok(suffix_width) = wrapped::<_, u32>(suffix.len());
        let remaining = match 5_u32.checked_sub(suffix_width) {
          Some(bits) => bits,
          None => return Ok(()),
        };
        let raw = suffix.iter().fold(0_u32, |bits, &byte| {
          bits.wrapping_shl(1) | u32::from(byte == b'1')
        });
        let Ok(start) = index(raw.wrapping_shl(remaining));
        let Ok(count) = index(1_u32.wrapping_shl(remaining));

        match self.entries.get_mut(row) {
          Some(entries) => {
            for slot in entries.iter_mut().skip(start).take(count) {
              *slot = encoded;
            }
          }
          None => {}
        }
      }
      None => {}
    }

    Ok(())
  }

  #[inline]
  fn decode(&self, reader: &mut BitReader) -> Result<u8> {
    let Ok(bits) = reader.peek(32);
    let zeros = bits.leading_zeros().min(16);
    let suffix = match zeros {
      16 => 0,
      _ => bits.wrapping_shl(zeros).wrapping_shl(1).wrapping_shr(27),
    };
    let Ok(row) = index(zeros);
    let Ok(column) = index(suffix);
    let entry = self
      .entries
      .get(row)
      .and_then(|entries| entries.get(column));
    let encoded = entry.ok_or_else(|| {
      Error::Invalid(<::std::string::String as ::core::convert::From<
        &::core::primitive::str,
      >>::from("index or range outside the buffer"))
    })?;
    ensure!(*encoded != 0, "invalid VLC code");
    let [value, width] = encoded.to_le_bytes();
    reader.skip(u64::from(width))?;
    Ok(value)
  }
}

struct Tables {
  coeff_token: [VlcTable; 6],
  total_zeros_4x4: [VlcTable; 15],
  total_zeros_2x2: [VlcTable; 3],
  total_zeros_2x4: [VlcTable; 7],
  run_before: [VlcTable; 7],
}

fn build_ordered<const N: usize>(codes: [&str; N]) -> core::result::Result<VlcTable, Never> {
  let Ok(mut table) = VlcTable::empty();

  for (position, code) in codes.into_iter().enumerate() {
    let Ok(value) = wrapped::<_, u8>(position);
    let Ok(()) = table.insert(code, value);
  }

  Ok(table)
}

fn tables() -> core::result::Result<&'static Tables, Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    expect(
      explicit044_no_ambient_value,
      reason = "CAVLC lookup tables are derived entirely from fixed specification data and returned by immutable reference"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "initialization builds the immutable decode tables once from specification constants, and subsequent decoders only borrow them"
    )
  )]
  static T: OnceLock<Tables> = OnceLock::new();

  Ok(T.get_or_init(|| {
    let mut coeff_token = std::array::from_fn(|_| {
      let Ok(table) = VlcTable::empty();
      table
    });

    for &(trailing, total, codes) in &COEFF_TOKEN {
      let value = trailing.wrapping_shl(5) | total;

      for (table, code) in coeff_token.iter_mut().zip(codes) {
        let Ok(()) = table.insert(code, value);
      }
    }

    Tables {
      coeff_token,
      total_zeros_4x4: TOTAL_ZEROS_4X4.map(|codes| match build_ordered(codes) {
        Ok(table) => table,
      }),
      total_zeros_2x2: TOTAL_ZEROS_2X2.map(|codes| match build_ordered(codes) {
        Ok(table) => table,
      }),
      total_zeros_2x4: TOTAL_ZEROS_2X4.map(|codes| match build_ordered(codes) {
        Ok(table) => table,
      }),
      run_before: RUN_BEFORE.map(|codes| match build_ordered(codes) {
        Ok(table) => table,
      }),
    }
  }))
}

pub(crate) fn init_tables() -> core::result::Result<(), Never> {
  let Ok(_initialized) = tables();
  Ok(())
}

#[derive(Clone, Copy)]
struct CoeffToken {
  total: u8,
  trailing_ones: u8,
}

fn read_token(reader: &mut BitReader, tables: &Tables, context: i32) -> Result<CoeffToken> {
  let [small, medium, large, larger, dc420, dc422] = &tables.coeff_token;
  let table = match context {
    -1 => dc420,
    -2 => dc422,
    0..=1 => small,
    2..=3 => medium,
    4..=7 => large,
    _ => larger,
  };
  let value = table.decode(reader)?;
  Ok(CoeffToken {
    total: value & 31,
    trailing_ones: value.wrapping_shr(5),
  })
}

fn read_level_code(reader: &mut BitReader, suffix_length: u32) -> Result<i32> {
  let Ok(bits) = reader.peek(32);
  let prefix = bits.leading_zeros();
  ensure!(prefix <= 25, "level_prefix too large");
  reader.skip(u64::from(prefix).wrapping_add(1))?;
  let suffix_size = match (prefix, suffix_length) {
    (14, 0) => 4,
    (15.., _) => prefix.wrapping_sub(3),
    _ => suffix_length,
  };
  let suffix = match suffix_size {
    0 => 0,
    _ => {
      let bits_2 = reader.read_bits(suffix_size)?;
      let Ok(value) = wrapped::<_, i32>(bits_2);
      value
    }
  };
  let Ok(prefix_value) = wrapped::<_, i32>(prefix.min(15));
  let mut code = prefix_value
    .wrapping_shl(suffix_length)
    .wrapping_add(suffix);

  match (prefix, suffix_length) {
    (15.., 0) => code = code.wrapping_add(15),
    _ => {}
  }

  match prefix {
    16.. => {
      let additional = 1_i32
        .wrapping_shl(prefix.wrapping_sub(3))
        .wrapping_sub(4096);
      code = code.wrapping_add(additional);
    }
    _ => {}
  }

  Ok(code)
}

fn read_levels(reader: &mut BitReader, token: CoeffToken) -> Result<[i32; 16]> {
  let mut levels = [0_i32; 16];
  let mut suffix_length = match (token.total, token.trailing_ones) {
    (11.., 0..=2) => 1_u32,
    _ => 0,
  };

  for (ordinal, level) in (0..token.total).zip(&mut levels) {
    match ordinal.cmp(&token.trailing_ones) {
      core::cmp::Ordering::Less => {
        let sign = reader.read_flag()?;
        *level = match sign {
          FlagBit::One => -1,
          FlagBit::Zero => 1,
        };
        continue;
      }
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
    }

    let base_code = read_level_code(reader, suffix_length)?;
    let code = match (ordinal, token.trailing_ones) {
      (0..=2, count) => match ordinal.cmp(&count) {
        core::cmp::Ordering::Equal => base_code.wrapping_add(2),
        core::cmp::Ordering::Less | core::cmp::Ordering::Greater => base_code,
      },
      _ => base_code,
    };
    *level = match code.wrapping_rem(2) {
      0 => code.wrapping_add(2).wrapping_shr(1),
      _ => code.wrapping_neg().wrapping_sub(1).wrapping_shr(1),
    };
    suffix_length = suffix_length.max(1);
    let threshold = 3_u32.wrapping_shl(suffix_length.wrapping_sub(1));

    match (level.unsigned_abs().cmp(&threshold), suffix_length) {
      (core::cmp::Ordering::Greater, 0..=5) => suffix_length = suffix_length.wrapping_add(1),
      _ => {}
    }
  }

  Ok(levels)
}

fn read_total_zeros(
  reader: &mut BitReader,
  tables: &Tables,
  token: CoeffToken,
  capacity: u64,
) -> Result<u8> {
  match u64::from(token.total).cmp(&capacity) {
    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => return Ok(0),
    core::cmp::Ordering::Less => {}
  }

  let choices: &[VlcTable] = match capacity {
    4 => &tables.total_zeros_2x2,
    8 => &tables.total_zeros_2x4,
    _ => &tables.total_zeros_4x4,
  };
  let Ok(position) = index(token.total.wrapping_sub(1));
  let table = choices.get(position).ok_or_else(|| {
    Error::Invalid(<::std::string::String as ::core::convert::From<
      &::core::primitive::str,
    >>::from("index or range outside the buffer"))
  })?;
  let zeros = table.decode(reader)?;
  ensure!(
    u64::from(zeros).wrapping_add(u64::from(token.total)) <= capacity,
    "total_zeros out of range"
  );
  Ok(zeros)
}

fn write_levels(
  reader: &mut BitReader,
  tables: &Tables,
  levels: &[i32; 16],
  token: CoeffToken,
  mut zeros: u8,
  coefficients: &mut [i32],
) -> Result<()> {
  let mut position = token.total.wrapping_add(zeros).wrapping_sub(1);

  for (ordinal, &level) in (0..token.total).zip(levels) {
    let Ok(indexed) = index(position);
    let coefficient = coefficients.get_mut(indexed).ok_or_else(|| {
      Error::Invalid(<::std::string::String as ::core::convert::From<
        &::core::primitive::str,
      >>::from("index or range outside the buffer"))
    })?;

    *coefficient = level;
    match ::core::primitive::bool::then_some(ordinal.wrapping_add(1) == token.total, ()) {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let run = match zeros {
      0 => 0,
      _ => {
        let Ok(prior) = index(zeros.min(7).wrapping_sub(1));
        let table = tables.run_before.get(prior).ok_or_else(|| {
          Error::Invalid(<::std::string::String as ::core::convert::From<
            &::core::primitive::str,
          >>::from("index or range outside the buffer"))
        })?;
        let run = table.decode(reader)?;
        ensure!(run <= zeros, "run_before out of range");
        run
      }
    };
    zeros = zeros.wrapping_sub(run);
    position = position.wrapping_sub(run.wrapping_add(1));
  }

  Ok(())
}

pub(crate) fn residual_block(
  reader: &mut BitReader,
  coefficients: &mut [i32],
  context: i32,
) -> Result<u8> {
  let Ok(tables) = tables();
  let token = read_token(reader, tables, context)?;

  match token.total {
    0 => return Ok(0),
    _ => {}
  }

  let Ok(capacity) = fixed_index(coefficients.len());
  ensure!(
    u64::from(token.total) <= capacity,
    "TotalCoeff exceeds maxNumCoeff"
  );
  let levels = read_levels(reader, token)?;
  let zeros = read_total_zeros(reader, tables, token, capacity)?;
  write_levels(reader, tables, &levels, token, zeros, coefficients)?;
  Ok(token.total)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_video_decoding_bitstream::{BitWriter, Field};

  fn kraft(codes: &[&str]) -> core::result::Result<f64, console_core_never::Never> {
    Ok(
      codes
        .iter()
        .filter(|c| !c.is_empty())
        .map(|c| {
          0.5f64.powi({
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(c.len());
            numeric_value
          })
        })
        .sum(),
    )
  }

  #[derive(Debug, PartialEq, Eq)]
  enum PrefixTable {
    PrefixFree,
    ContainsPrefix,
  }

  #[must_use]
  fn prefix_table(codes: &[&str]) -> core::result::Result<PrefixTable, Never> {
    let c: Vec<&&str> = codes.iter().filter(|c| !c.is_empty()).collect();

    for (i, a) in c.iter().enumerate() {
      for (j, b) in c.iter().enumerate() {
        match ::core::primitive::bool::then_some(i != j && b.starts_with(**a), ()) {
          ::core::option::Option::Some(()) => {
            return Ok(PrefixTable::ContainsPrefix);
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(PrefixTable::PrefixFree)
  }

  #[test]
  fn coeff_token_tables_are_prefix_codes() {
    let mut columns = [const { Vec::new() }; 6];

    for &(_, _, codes) in &COEFF_TOKEN {
      for (column, code) in columns.iter_mut().zip(codes) {
        column.push(code);
      }
    }

    for (col, codes) in columns.iter().enumerate() {
      assert_eq!(
        prefix_table(codes),
        Ok(PrefixTable::PrefixFree),
        "column {col}"
      );
      let k = {
        let Ok(explicit_value) = kraft(codes);
        explicit_value
      };
      assert!(k <= 1.0 + 1e-12, "column {col} kraft {k}");
    }

    assert!(
      COEFF_TOKEN
        .iter()
        .map(|e| *match &e.2 {
          [_, _, _, element, ..] => element,
        })
        .all(|c| c.len() == 6)
    );
  }

  #[test]
  fn total_zeros_and_run_before_are_complete_prefix_codes() {
    for (i, row) in TOTAL_ZEROS_4X4.iter().enumerate() {
      assert_eq!(prefix_table(row), Ok(PrefixTable::PrefixFree));

      let k = {
        let Ok(explicit_value) = kraft(row);
        explicit_value
      };

      match ::core::primitive::bool::then_some(i == 0, ()) {
        ::core::option::Option::Some(()) => {
          assert!((k - (1.0 - 1.0 / 512.0)).abs() < 1e-12, "{row:?}");
        }
        ::core::option::Option::None => {
          assert!((k - 1.0).abs() < 1e-12, "{row:?}");
        }
      }
    }

    for row in TOTAL_ZEROS_2X2.iter() {
      assert_eq!(prefix_table(row), Ok(PrefixTable::PrefixFree));
      assert!(
        (({
          let Ok(explicit_value) = kraft(row);
          explicit_value
        }) - 1.0)
          .abs()
          < 1e-12
      );
    }

    for row in TOTAL_ZEROS_2X4.iter() {
      assert_eq!(prefix_table(row), Ok(PrefixTable::PrefixFree));
      assert!(
        (({
          let Ok(explicit_value) = kraft(row);
          explicit_value
        }) - 1.0)
          .abs()
          < 1e-12
      );
    }

    for row in RUN_BEFORE.iter() {
      assert_eq!(prefix_table(row), Ok(PrefixTable::PrefixFree));
    }
  }

  fn write_code(
    w: &mut BitWriter,
    code: &str,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      for b in code.bytes() {
        let Ok(()) = w.write_bits(Field {
          value: u32::from(b == b'1'),
          width: 1,
        });
      }
    };

    Ok(())
  }

  #[test]
  fn decodes_spec_style_example() -> ::core::result::Result<(), crate::error::Error> {
    let Ok(mut w) = BitWriter::new();
    ({
      let Ok(explicit_value) = write_code(&mut w, "0000100");
      explicit_value
    });
    ({
      let Ok(explicit_value) = write_code(&mut w, "0");
      explicit_value
    });
    ({
      let Ok(explicit_value) = write_code(&mut w, "0");
      explicit_value
    });
    ({
      let Ok(explicit_value) = write_code(&mut w, "1");
      explicit_value
    });

    ({
      let Ok(explicit_value) = write_code(&mut w, "01");
      explicit_value
    });

    ({
      let Ok(explicit_value) = write_code(&mut w, "0010");
      explicit_value
    });

    ({
      let Ok(explicit_value) = write_code(&mut w, "110");
      explicit_value
    });

    ({
      let Ok(explicit_value) = write_code(&mut w, "10");
      explicit_value
    });
    ({
      let Ok(explicit_value) = write_code(&mut w, "11");
      explicit_value
    });
    ({
      let Ok(explicit_value) = write_code(&mut w, "01");
      explicit_value
    });
    ({
      let Ok(explicit_value) = write_code(&mut w, "1");
      explicit_value
    });
    let Ok(()) = w.rbsp_trailing();
    let Ok(b) = w.finish();
    let Ok(mut r) = BitReader::new(&b);
    let mut c = [0i32; 16];
    let tc = residual_block(&mut r, &mut c, 0)?;
    assert_eq!(tc, 5);
    assert_eq!(
      {
        let [_explicit_part @ .., _, _, _, _, _, _] = &c;
        let _explicit_slice: &[_] = _explicit_part;
        _explicit_slice
      },
      &[0, 3, -1, 0, 0, -1, 1, 0, 1, 0]
    );
    ::core::result::Result::Ok(())
  }
}
