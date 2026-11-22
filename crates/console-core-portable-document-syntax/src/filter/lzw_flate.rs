use crate::bit_reader::{BitChunk, BitChunks, BitReader, BitWriter, bit_mask};
use crate::object::Dict;
use crate::object::dict::keys::{BITS_PER_COMPONENT, COLORS, COLUMNS, EARLY_CHANGE, PREDICTOR};
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

pub(crate) mod flate {
  use super::*;
  use crate::filter::lzw_flate::{PredictorParams, apply_predictor};
  use crate::object::Dict;

  const MAX_DECODED_BYTES: u64 = 256 * 1024 * 1024;

  #[cfg(feature = "unsafe")]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Predictor fields can resolve indirect references through the bounded object reader"
    )
  )]
  pub(crate) fn decode(
    data: &[u8],
    params: &Dict<'_>,
  ) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
    use flate2::read::{DeflateDecoder, ZlibDecoder};
    use std::io::Read;

    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "Both callers pass an in-memory decoder, and Take bounds the decompressed output"
      )
    )]
    fn read_stream(decoder: impl Read) -> Result<Option<Vec<u8>>, console_core_never::Never> {
      let mut bounded = decoder.take(MAX_DECODED_BYTES.saturating_add(1));
      let mut output = Vec::new();

      Ok(match bounded.read_to_end(&mut output) {
        Ok(count) => {
          let Ok(length) = fixed_index(count);

          match length {
            0..=MAX_DECODED_BYTES => Some(output),
            _over_output_limit => None,
          }
        }
        Err(_invalid_flate_stream) => None,
      })
    }

    let decoded = match ({
      let Ok(explicit_value) = read_stream(ZlibDecoder::new(data));
      explicit_value
    })
    .or_else(|| {
      let Ok(explicit_value) = read_stream(DeflateDecoder::new(data));
      explicit_value
    })
    .or_else(|| {
      warn!("flate stream is broken, decoding with fallback");

      let Ok(decode) = fallback::decode(data);

      decode
    }) {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(params_2) = PredictorParams::from_params(params);

    Ok({
      let Ok(explicit_value) = apply_predictor(decoded, &params_2);
      explicit_value
    })
  }

  #[cfg(not(feature = "unsafe"))]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Predictor fields can resolve indirect references through the bounded object reader"
    )
  )]
  pub(crate) fn decode(
    data: &[u8],
    params: &Dict<'_>,
  ) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
    let Ok(explicit_value) = fallback::decode(data);
    let decoded = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(params_2) = PredictorParams::from_params(params);

    Ok({
      let Ok(explicit_value_2) = apply_predictor(decoded, &params_2);
      explicit_value_2
    })
  }

  mod fallback {
    use super::{MAX_DECODED_BYTES, Never, fixed_index, index};
    use alloc::vec::Vec;

    pub(crate) fn decode(data: &[u8]) -> Result<Option<Vec<u8>>, Never> {
      let input = match data {
        [cmf, flags, body @ ..] => {
          let header = u16::from_be_bytes([*cmf, *flags]);
          let zlib = cmf & 0x0f == 8 && header.is_multiple_of(31) && flags & 0x20 == 0;

          match zlib.then_some(()) {
            Some(()) => body,
            None => data,
          }
        }
        [] | [_] => data,
      };
      let Ok(mut stream) = FlateStream::new(input);

      stream.decode()
    }

    enum FlateStatus {
      Reading,
      Finished,
      OutputLimitExceeded,
    }

    enum OutputRoom {
      Available,
      LimitExceeded,
    }

    enum HuffmanTables {
      Fixed,
      Dynamic,
    }

    struct FlateStream<'a> {
      data: &'a [u8],
      input_size: u64,
      pos: u64,
      code_buf: u32,
      code_size: u8,
      output: Vec<u8>,
      status: FlateStatus,
    }

    impl<'a> FlateStream<'a> {
      fn new(data: &'a [u8]) -> Result<Self, Never> {
        let Ok(input_size) = fixed_index(data.len());

        Ok(Self {
          data,
          input_size,
          pos: 0,
          code_buf: 0,
          code_size: 0,
          output: Vec::new(),
          status: FlateStatus::Reading,
        })
      }

      fn decode(&mut self) -> Result<Option<Vec<u8>>, Never> {
        for _block_bit in 0..self.input_size.saturating_mul(8) {
          match (&self.status, self.pos.cmp(&self.input_size)) {
            (FlateStatus::Reading, core::cmp::Ordering::Less) => {
              let Ok(()) = self.read_block();
            }
            _input_or_output_finished => break,
          }
        }

        Ok(match self.status {
          FlateStatus::OutputLimitExceeded => None,
          FlateStatus::Reading | FlateStatus::Finished => Some(core::mem::take(&mut self.output)),
        })
      }

      fn room_for(&mut self, bytes: u64) -> Result<OutputRoom, Never> {
        let Ok(length) = fixed_index(self.output.len());

        Ok(match bytes.cmp(&MAX_DECODED_BYTES.saturating_sub(length)) {
          core::cmp::Ordering::Less | core::cmp::Ordering::Equal => OutputRoom::Available,
          core::cmp::Ordering::Greater => {
            self.status = FlateStatus::OutputLimitExceeded;
            OutputRoom::LimitExceeded
          }
        })
      }

      fn get_byte(&mut self) -> Result<Option<u8>, Never> {
        let Ok(position) = index(self.pos);

        Ok(match self.data.get(position) {
          Some(byte) => {
            self.pos = self.pos.saturating_add(1);
            Some(*byte)
          }
          None => None,
        })
      }

      fn get_bytes(&mut self, bytes: u64) -> Result<&'a [u8], Never> {
        let end = self.pos.saturating_add(bytes).min(self.input_size);
        let Ok(start_position) = index(self.pos);
        let Ok(end_position) = index(end);
        self.pos = end;

        Ok(match self.data.get(start_position..end_position) {
          Some(bytes_2) => bytes_2,
          None => &[],
        })
      }

      fn fill_code(&mut self, bits: u8) -> Result<(), Never> {
        for _byte in 0..bits.div_ceil(8) {
          match self.code_size.cmp(&bits) {
            core::cmp::Ordering::Less => {}
            core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
          }

          let Ok(next) = self.get_byte();
          let byte = match next {
            Some(byte) => byte,
            None => break,
          };
          self.code_buf |= u32::from(byte).wrapping_shl(u32::from(self.code_size));
          self.code_size = self.code_size.saturating_add(8);
        }

        Ok(())
      }

      fn get_bits(&mut self, bits: u8) -> Result<Option<u32>, Never> {
        let Ok(()) = self.fill_code(bits);

        match self.code_size.cmp(&bits) {
          core::cmp::Ordering::Less => return Ok(None),
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
        }

        let value = self.code_buf & 1_u32.wrapping_shl(u32::from(bits)).wrapping_sub(1);
        let Ok(()) = self.advance_code(bits);

        Ok(Some(value))
      }

      fn get_code(&mut self, table: &HuffmanTable) -> Result<Option<u16>, Never> {
        let Ok(()) = self.fill_code(table.max_len);
        let mask = 1_u32.wrapping_shl(u32::from(table.max_len)).wrapping_sub(1);
        let Ok(position) = index(self.code_buf & mask);
        let code = match table.codes.get(position) {
          Some(code) => *code,
          None => return Ok(None),
        };
        let Ok(length) = console_core_number_conversion::wrapped::<_, u8>(code.wrapping_shr(16));

        match (length, self.code_size.cmp(&length)) {
          (0, _) | (_, core::cmp::Ordering::Less) => return Ok(None),
          (1.., core::cmp::Ordering::Equal | core::cmp::Ordering::Greater) => {}
        }

        let Ok(()) = self.advance_code(length);
        console_core_number_conversion::wrapped::<_, u16>(code & 0xffff).map(Some)
      }

      fn advance_code(&mut self, bits: u8) -> Result<(), Never> {
        self.code_buf = self.code_buf.wrapping_shr(u32::from(bits));
        self.code_size = self.code_size.saturating_sub(bits);

        Ok(())
      }

      fn read_block(&mut self) -> Result<(), Never> {
        let Ok(header) = self.get_bits(3);
        let bits = match header {
          Some(bits) => bits,
          None => {
            warn!("bad block header in flate stream");
            self.status = FlateStatus::Finished;
            return Ok(());
          }
        };

        match bits & 1 {
          0 => {}
          _last_block => self.status = FlateStatus::Finished,
        }

        match bits.wrapping_shr(1) {
          0 => self.read_uncompressed_block(),
          1 => self.read_compressed_block(HuffmanTables::Fixed),
          2 => self.read_compressed_block(HuffmanTables::Dynamic),
          _unknown_block => {
            warn!("unknown block type in flate stream");
            self.status = FlateStatus::Finished;
            Ok(())
          }
        }
      }

      fn read_uncompressed_block(&mut self) -> Result<(), Never> {
        self.code_buf = 0;
        self.code_size = 0;
        let Ok(header) = self.get_bytes(4);
        let (block_len, check) = match header {
          [len_low, len_high, check_low, check_high] => (
            u16::from_le_bytes([*len_low, *len_high]),
            u16::from_le_bytes([*check_low, *check_high]),
          ),
          _incomplete_header => {
            warn!("bad block header in flate stream");
            self.status = FlateStatus::Finished;
            return Ok(());
          }
        };

        match (check != !block_len && (block_len != 0 || check != 0)).then_some(()) {
          Some(()) => warn!("bad uncompressed block length in flate stream"),
          None => {}
        }

        let Ok(block) = self.get_bytes(u64::from(block_len));
        let Ok(read) = fixed_index(block.len());
        let Ok(room) = self.room_for(read);

        match room {
          OutputRoom::Available => self.output.extend_from_slice(block),
          OutputRoom::LimitExceeded => return Ok(()),
        }

        match (
          read.cmp(&u64::from(block_len)),
          self.pos.cmp(&self.input_size),
          block_len,
        ) {
          (core::cmp::Ordering::Less, _, _)
          | (_, core::cmp::Ordering::Equal | core::cmp::Ordering::Greater, 0) => {
            self.status = FlateStatus::Finished;
          }
          _continues => {}
        }

        Ok(())
      }

      fn read_compressed_block(&mut self, table_kind: HuffmanTables) -> Result<(), Never> {
        let (literals, distances) = match table_kind {
          HuffmanTables::Fixed => {
            let Ok(literals) = get_fixed_lit_table();
            let Ok(distances) = get_fixed_dist_table();
            (literals, distances)
          }
          HuffmanTables::Dynamic => {
            let Ok(parsed) = self.read_dynamic_tables();

            match parsed {
              Some(tables) => tables,
              None => {
                self.status = FlateStatus::Finished;
                return Ok(());
              }
            }
          }
        };

        for _symbol_bit in 0..self.input_size.saturating_mul(8) {
          let Ok(symbol) = self.get_code(&literals);

          match symbol {
            Some(literal @ 0..=255) => {
              let Ok(room) = self.room_for(1);

              match room {
                OutputRoom::Available => {
                  let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(literal);
                  self.output.push(byte);
                }
                OutputRoom::LimitExceeded => return Ok(()),
              }
            }
            Some(256) => return Ok(()),
            Some(code) => {
              let Ok(copied) = self.copy_reference(code, &distances);

              match copied {
                ReferenceCopy::Continues => {}
                ReferenceCopy::Stopped => return Ok(()),
              }
            }
            None => {
              self.status = FlateStatus::Finished;
              return Ok(());
            }
          }
        }

        self.status = FlateStatus::Finished;
        Ok(())
      }

      fn copy_reference(
        &mut self,
        symbol: u16,
        distances: &HuffmanTable,
      ) -> Result<ReferenceCopy, Never> {
        let Ok(length_slot) = index(symbol.wrapping_sub(257));
        let length_code = match LENGTH_DECODE.get(length_slot) {
          Some(code) => *code,
          None => 0,
        };
        let Ok(extent) = self.read_extent(length_code);
        let length = match extent {
          Some(length) => length,
          None => return self.end_reference(),
        };
        let Ok(next_distance) = self.get_code(distances);
        let distance_code = match next_distance {
          Some(code) => code,
          None => return self.end_reference(),
        };
        let Ok(distance_slot) = index(distance_code);
        let descriptor = match DIST_DECODE.get(distance_slot) {
          Some(code) => *code,
          None => {
            warn!("invalid distance code {} in flate stream", distance_code);
            return self.end_reference();
          }
        };
        let Ok(extent_2) = self.read_extent(descriptor);
        let distance = match extent_2 {
          Some(distance) => distance,
          None => return self.end_reference(),
        };
        let Ok(room) = self.room_for(u64::from(length));

        match room {
          OutputRoom::Available => {}
          OutputRoom::LimitExceeded => return Ok(ReferenceCopy::Stopped),
        }

        let Ok(back) = index(distance);

        for _ in 0..length {
          let byte = match self
            .output
            .len()
            .checked_sub(back)
            .and_then(|position| self.output.get(position))
          {
            Some(byte) => *byte,
            None => break,
          };
          self.output.push(byte);
        }

        Ok(ReferenceCopy::Continues)
      }

      fn read_extent(&mut self, descriptor: u32) -> Result<Option<u32>, Never> {
        let Ok(width) =
          console_core_number_conversion::wrapped::<_, u8>(descriptor.wrapping_shr(16));
        let Ok(extra) = self.get_bits(width);

        Ok(extra.map(|extra_2| (descriptor & 0xffff).wrapping_add(extra_2)))
      }

      fn end_reference(&mut self) -> Result<ReferenceCopy, Never> {
        self.status = FlateStatus::Finished;
        Ok(ReferenceCopy::Stopped)
      }

      fn read_dynamic_tables(&mut self) -> Result<Option<(HuffmanTable, HuffmanTable)>, Never> {
        let Ok(counts) = self.read_dynamic_counts();
        let (literals, distances, length_symbols) = match counts {
          Some(counts_2) => counts_2,
          None => return Ok(None),
        };
        let mut code_length_code_lengths = [0_u8; 19];
        let Ok(read_limit) = index(length_symbols);

        for &symbol in CODE_LEN_CODE_MAP.iter().take(read_limit) {
          let Ok(bits) = self.get_bits(3);
          let length = match bits {
            Some(length) => length,
            None => return Ok(None),
          };
          let Ok(symbol_index) = index(symbol);
          let slot = match code_length_code_lengths.get_mut(symbol_index) {
            Some(slot) => slot,
            None => return Ok(None),
          };
          let Ok(width) = console_core_number_conversion::wrapped::<_, u8>(length);
          *slot = width;
        }

        let Ok(length_table) = generate_huffman_table(&code_length_code_lengths);
        let Ok(parsed) =
          self.read_code_lengths(&length_table, literals.saturating_add(distances));
        let lengths = match parsed {
          Some(lengths) => lengths,
          None => return Ok(None),
        };
        let Ok(first_table_size) = index(literals);
        let (literal_lengths, distance_lengths) = match lengths.split_at_checked(first_table_size)
        {
          Some(tables) => tables,
          None => return Ok(None),
        };
        let Ok(literal_table) = generate_huffman_table(literal_lengths);
        let Ok(distance_table) = generate_huffman_table(distance_lengths);

        Ok(Some((literal_table, distance_table)))
      }

      fn read_dynamic_counts(&mut self) -> Result<Option<(u32, u32, u32)>, Never> {
        let Ok(literals) = self.get_bits(5);

        match literals {
          Some(literal_count) => {
            let Ok(distances) = self.get_bits(5);

            match distances {
              Some(distance_count) => {
                let Ok(length_symbols) = self.get_bits(4);

                Ok(length_symbols.map(|symbols| {
                  (
                    literal_count.saturating_add(257),
                    distance_count.saturating_add(1),
                    symbols.saturating_add(4),
                  )
                }))
              }
              None => Ok(None),
            }
          }
          None => Ok(None),
        }
      }

      fn read_code_lengths(
        &mut self,
        table: &HuffmanTable,
        count: u32,
      ) -> Result<Option<Vec<u8>>, Never> {
        let mut lengths = Vec::new();
        let mut remaining = count;

        for _ in 0..count {
          match remaining {
            0 => break,
            1.. => {}
          }

          let Ok(code) = self.get_code(table);
          let code_3 = match code {
            Some(code_2) => code_2,
            None => return Ok(None),
          };
          let previous = match lengths.last() {
            Some(length) => *length,
            None => 0,
          };
          let Ok(repetition) = self.read_length_run(code_3, previous);
          let (width, repeat) = match repetition {
            Some(run) => run,
            None => return Ok(None),
          };
          let run = remaining.min(repeat);
          lengths.extend((0..run).map(|_| width));
          remaining = remaining.saturating_sub(run);
        }

        Ok(Some(lengths))
      }

      fn read_length_run(&mut self, code: u16, previous: u8) -> Result<Option<(u8, u32)>, Never> {
        let (sample, width, base) = match code {
          0..=15 => {
            let Ok(length) = console_core_number_conversion::wrapped::<_, u8>(code);
            return Ok(Some((length, 1)));
          }
          16 => (previous, 2, 3_u32),
          17 => (0, 3, 3),
          18 => (0, 7, 11),
          _unexpected_symbol => return Ok(None),
        };
        let Ok(extra) = self.get_bits(width);

        Ok(extra.map(|extra_2| (sample, base.saturating_add(extra_2))))
      }
    }

    enum ReferenceCopy {
      Continues,
      Stopped,
    }

    struct HuffmanTable {
      codes: Vec<u32>,
      max_len: u8,
    }

    fn generate_huffman_table(lengths: &[u8]) -> Result<HuffmanTable, Never> {
      let max_len = match lengths.iter().copied().max() {
        Some(used @ 1..=15) => used,
        Some(_) | None => {
          return Ok(HuffmanTable {
            codes: alloc::vec![0],
            max_len: 1,
          });
        }
      };
      let size = 1_u32.wrapping_shl(u32::from(max_len));
      let mut codes = (0..size).map(|_| 0_u32).collect::<Vec<_>>();
      let mut code = 0_u32;

      for depth in 1..=max_len {
        for (symbol, &length) in lengths.iter().enumerate() {
          match length.cmp(&depth) {
            core::cmp::Ordering::Equal => {
              let Ok(()) = insert_huffman_code(
                &mut codes,
                SymbolCode {
                  code,
                  depth,
                  symbol: {
                    let Ok(number) = console_core_number_conversion::wrapped::<_, u32>(symbol);
                    number
                  },
                },
                size,
              );
              code = code.wrapping_add(1);
            }
            core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {}
          }
        }

        code = code.wrapping_shl(1);
      }

      Ok(HuffmanTable { codes, max_len })
    }

    struct SymbolCode {
      code: u32,
      depth: u8,
      symbol: u32,
    }

    fn insert_huffman_code(
      codes: &mut [u32],
      code: SymbolCode,
      table_size: u32,
    ) -> Result<(), Never> {
      let depth = u32::from(code.depth);
      let reversed = code
        .code
        .reverse_bits()
        .wrapping_shr(32_u32.saturating_sub(depth));
      let stride = 1_u32.wrapping_shl(depth);
      let packed = depth.wrapping_shl(16) | code.symbol;

      for entry in 0..table_size.wrapping_div(stride) {
        let position = reversed.saturating_add(entry.saturating_mul(stride));
        let Ok(slot) = index(position);

        match codes.get_mut(slot) {
          Some(target) => *target = packed,
          None => break,
        }
      }

      Ok(())
    }

    fn get_fixed_lit_table() -> core::result::Result<HuffmanTable, console_core_never::Never> {
      Ok(HuffmanTable {
        codes: FIXED_LIT_CODE_TAB.to_vec(),
        max_len: 9,
      })
    }

    fn get_fixed_dist_table() -> core::result::Result<HuffmanTable, console_core_never::Never> {
      Ok(HuffmanTable {
        codes: FIXED_DIST_CODE_TAB.to_vec(),
        max_len: 5,
      })
    }

    const CODE_LEN_CODE_MAP: [u8; 19] = [
      16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    const LENGTH_DECODE: [u32; 29] = [
      0x00003, 0x00004, 0x00005, 0x00006, 0x00007, 0x00008, 0x00009, 0x0000a, 0x1000b, 0x1000d,
      0x1000f, 0x10011, 0x20013, 0x20017, 0x2001b, 0x2001f, 0x30023, 0x3002b, 0x30033, 0x3003b,
      0x40043, 0x40053, 0x40063, 0x40073, 0x50083, 0x500a3, 0x500c3, 0x500e3, 0x00102,
    ];

    const DIST_DECODE: [u32; 30] = [
      0x00001, 0x00002, 0x00003, 0x00004, 0x10005, 0x10007, 0x20009, 0x2000d, 0x30011, 0x30019,
      0x40021, 0x40031, 0x50041, 0x50061, 0x60081, 0x600c1, 0x70101, 0x70181, 0x80201, 0x80301,
      0x90401, 0x90601, 0xa0801, 0xa0c01, 0xb1001, 0xb1801, 0xc2001, 0xc3001, 0xd4001, 0xd6001,
    ];

    const FIXED_LIT_CODE_TAB: [u32; 512] = [
      0x70100, 0x80050, 0x80010, 0x80118, 0x70110, 0x80070, 0x80030, 0x900c0, 0x70108, 0x80060,
      0x80020, 0x900a0, 0x80000, 0x80080, 0x80040, 0x900e0, 0x70104, 0x80058, 0x80018, 0x90090,
      0x70114, 0x80078, 0x80038, 0x900d0, 0x7010c, 0x80068, 0x80028, 0x900b0, 0x80008, 0x80088,
      0x80048, 0x900f0, 0x70102, 0x80054, 0x80014, 0x8011c, 0x70112, 0x80074, 0x80034, 0x900c8,
      0x7010a, 0x80064, 0x80024, 0x900a8, 0x80004, 0x80084, 0x80044, 0x900e8, 0x70106, 0x8005c,
      0x8001c, 0x90098, 0x70116, 0x8007c, 0x8003c, 0x900d8, 0x7010e, 0x8006c, 0x8002c, 0x900b8,
      0x8000c, 0x8008c, 0x8004c, 0x900f8, 0x70101, 0x80052, 0x80012, 0x8011a, 0x70111, 0x80072,
      0x80032, 0x900c4, 0x70109, 0x80062, 0x80022, 0x900a4, 0x80002, 0x80082, 0x80042, 0x900e4,
      0x70105, 0x8005a, 0x8001a, 0x90094, 0x70115, 0x8007a, 0x8003a, 0x900d4, 0x7010d, 0x8006a,
      0x8002a, 0x900b4, 0x8000a, 0x8008a, 0x8004a, 0x900f4, 0x70103, 0x80056, 0x80016, 0x8011e,
      0x70113, 0x80076, 0x80036, 0x900cc, 0x7010b, 0x80066, 0x80026, 0x900ac, 0x80006, 0x80086,
      0x80046, 0x900ec, 0x70107, 0x8005e, 0x8001e, 0x9009c, 0x70117, 0x8007e, 0x8003e, 0x900dc,
      0x7010f, 0x8006e, 0x8002e, 0x900bc, 0x8000e, 0x8008e, 0x8004e, 0x900fc, 0x70100, 0x80051,
      0x80011, 0x80119, 0x70110, 0x80071, 0x80031, 0x900c2, 0x70108, 0x80061, 0x80021, 0x900a2,
      0x80001, 0x80081, 0x80041, 0x900e2, 0x70104, 0x80059, 0x80019, 0x90092, 0x70114, 0x80079,
      0x80039, 0x900d2, 0x7010c, 0x80069, 0x80029, 0x900b2, 0x80009, 0x80089, 0x80049, 0x900f2,
      0x70102, 0x80055, 0x80015, 0x8011d, 0x70112, 0x80075, 0x80035, 0x900ca, 0x7010a, 0x80065,
      0x80025, 0x900aa, 0x80005, 0x80085, 0x80045, 0x900ea, 0x70106, 0x8005d, 0x8001d, 0x9009a,
      0x70116, 0x8007d, 0x8003d, 0x900da, 0x7010e, 0x8006d, 0x8002d, 0x900ba, 0x8000d, 0x8008d,
      0x8004d, 0x900fa, 0x70101, 0x80053, 0x80013, 0x8011b, 0x70111, 0x80073, 0x80033, 0x900c6,
      0x70109, 0x80063, 0x80023, 0x900a6, 0x80003, 0x80083, 0x80043, 0x900e6, 0x70105, 0x8005b,
      0x8001b, 0x90096, 0x70115, 0x8007b, 0x8003b, 0x900d6, 0x7010d, 0x8006b, 0x8002b, 0x900b6,
      0x8000b, 0x8008b, 0x8004b, 0x900f6, 0x70103, 0x80057, 0x80017, 0x8011f, 0x70113, 0x80077,
      0x80037, 0x900ce, 0x7010b, 0x80067, 0x80027, 0x900ae, 0x80007, 0x80087, 0x80047, 0x900ee,
      0x70107, 0x8005f, 0x8001f, 0x9009e, 0x70117, 0x8007f, 0x8003f, 0x900de, 0x7010f, 0x8006f,
      0x8002f, 0x900be, 0x8000f, 0x8008f, 0x8004f, 0x900fe, 0x70100, 0x80050, 0x80010, 0x80118,
      0x70110, 0x80070, 0x80030, 0x900c1, 0x70108, 0x80060, 0x80020, 0x900a1, 0x80000, 0x80080,
      0x80040, 0x900e1, 0x70104, 0x80058, 0x80018, 0x90091, 0x70114, 0x80078, 0x80038, 0x900d1,
      0x7010c, 0x80068, 0x80028, 0x900b1, 0x80008, 0x80088, 0x80048, 0x900f1, 0x70102, 0x80054,
      0x80014, 0x8011c, 0x70112, 0x80074, 0x80034, 0x900c9, 0x7010a, 0x80064, 0x80024, 0x900a9,
      0x80004, 0x80084, 0x80044, 0x900e9, 0x70106, 0x8005c, 0x8001c, 0x90099, 0x70116, 0x8007c,
      0x8003c, 0x900d9, 0x7010e, 0x8006c, 0x8002c, 0x900b9, 0x8000c, 0x8008c, 0x8004c, 0x900f9,
      0x70101, 0x80052, 0x80012, 0x8011a, 0x70111, 0x80072, 0x80032, 0x900c5, 0x70109, 0x80062,
      0x80022, 0x900a5, 0x80002, 0x80082, 0x80042, 0x900e5, 0x70105, 0x8005a, 0x8001a, 0x90095,
      0x70115, 0x8007a, 0x8003a, 0x900d5, 0x7010d, 0x8006a, 0x8002a, 0x900b5, 0x8000a, 0x8008a,
      0x8004a, 0x900f5, 0x70103, 0x80056, 0x80016, 0x8011e, 0x70113, 0x80076, 0x80036, 0x900cd,
      0x7010b, 0x80066, 0x80026, 0x900ad, 0x80006, 0x80086, 0x80046, 0x900ed, 0x70107, 0x8005e,
      0x8001e, 0x9009d, 0x70117, 0x8007e, 0x8003e, 0x900dd, 0x7010f, 0x8006e, 0x8002e, 0x900bd,
      0x8000e, 0x8008e, 0x8004e, 0x900fd, 0x70100, 0x80051, 0x80011, 0x80119, 0x70110, 0x80071,
      0x80031, 0x900c3, 0x70108, 0x80061, 0x80021, 0x900a3, 0x80001, 0x80081, 0x80041, 0x900e3,
      0x70104, 0x80059, 0x80019, 0x90093, 0x70114, 0x80079, 0x80039, 0x900d3, 0x7010c, 0x80069,
      0x80029, 0x900b3, 0x80009, 0x80089, 0x80049, 0x900f3, 0x70102, 0x80055, 0x80015, 0x8011d,
      0x70112, 0x80075, 0x80035, 0x900cb, 0x7010a, 0x80065, 0x80025, 0x900ab, 0x80005, 0x80085,
      0x80045, 0x900eb, 0x70106, 0x8005d, 0x8001d, 0x9009b, 0x70116, 0x8007d, 0x8003d, 0x900db,
      0x7010e, 0x8006d, 0x8002d, 0x900bb, 0x8000d, 0x8008d, 0x8004d, 0x900fb, 0x70101, 0x80053,
      0x80013, 0x8011b, 0x70111, 0x80073, 0x80033, 0x900c7, 0x70109, 0x80063, 0x80023, 0x900a7,
      0x80003, 0x80083, 0x80043, 0x900e7, 0x70105, 0x8005b, 0x8001b, 0x90097, 0x70115, 0x8007b,
      0x8003b, 0x900d7, 0x7010d, 0x8006b, 0x8002b, 0x900b7, 0x8000b, 0x8008b, 0x8004b, 0x900f7,
      0x70103, 0x80057, 0x80017, 0x8011f, 0x70113, 0x80077, 0x80037, 0x900cf, 0x7010b, 0x80067,
      0x80027, 0x900af, 0x80007, 0x80087, 0x80047, 0x900ef, 0x70107, 0x8005f, 0x8001f, 0x9009f,
      0x70117, 0x8007f, 0x8003f, 0x900df, 0x7010f, 0x8006f, 0x8002f, 0x900bf, 0x8000f, 0x8008f,
      0x8004f, 0x900ff,
    ];

    const FIXED_DIST_CODE_TAB: [u32; 32] = [
      0x50000, 0x50010, 0x50008, 0x50018, 0x50004, 0x50014, 0x5000c, 0x5001c, 0x50002, 0x50012,
      0x5000a, 0x5001a, 0x50006, 0x50016, 0x5000e, 0x00000, 0x50001, 0x50011, 0x50009, 0x50019,
      0x50005, 0x50015, 0x5000d, 0x5001d, 0x50003, 0x50013, 0x5000b, 0x5001b, 0x50007, 0x50017,
      0x5000f, 0x00000,
    ];
  }
}

pub(crate) mod lzw {
  use alloc::vec::Vec;
  use console_core_lzw::{BitOrder, End, Parameters};
  use console_core_never::Never;

  use crate::filter::lzw_flate::{PredictorParams, apply_predictor};
  use crate::object::Dict;

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "LZW parameters use indirect resolution with a fixed ancestry limit"
    )
  )]
  pub(crate) fn decode(data: &[u8], params: &Dict<'_>) -> Result<Option<Vec<u8>>, Never> {
    let Ok(predictor) = PredictorParams::from_params(params);
    let parameters = Parameters {
      bit_order: BitOrder::MostSignificantFirst,
      literal_width: 8,
      width_growth: predictor.width_growth,
      output_limit: None,
    };
    let decoded = match console_core_lzw::decode(data, parameters) {
      Ok(decoded) => decoded,
      Err(fault) => {
        warn!("invalid code in LZW stream: {:?}", fault);

        return Ok(None);
      }
    };

    match decoded.end {
      End::Input => warn!("premature EOF in LZW stream, EOD code missing"),
      End::Code | End::OutputLimit => {}
    }

    apply_predictor(decoded.bytes, &predictor)
  }
}

struct PredictorParams {
  predictor: u8,
  colors: u8,
  bits_per_component: u8,
  columns: u64,
  width_growth: console_core_lzw::WidthGrowth,
}

impl Default for PredictorParams {
  fn default() -> Self {
    Self {
      predictor: 1,
      colors: 1,
      bits_per_component: 8,
      columns: 1,
      width_growth: console_core_lzw::WidthGrowth::OneCodeEarly,
    }
  }
}

impl PredictorParams {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Indirect dictionary values resolve through the reader context's visited objects"
    )
  )]
  fn from_params(dict: &Dict<'_>) -> Result<Self, Never> {
    let defaults = Self::default();
    let Ok(predictor) = dict.get::<u8>(PREDICTOR);
    let Ok(colors) = dict.get::<u8>(COLORS);
    let Ok(bits) = dict.get::<u8>(BITS_PER_COMPONENT);
    let Ok(columns) = dict.get::<u64>(COLUMNS);
    let Ok(early_change) = dict.get::<u8>(EARLY_CHANGE);

    Ok(Self {
      predictor: match predictor {
        Some(value) => value,
        None => defaults.predictor,
      },
      colors: match colors {
        Some(value) => value,
        None => defaults.colors,
      },
      bits_per_component: match bits {
        Some(value) => value,
        None => defaults.bits_per_component,
      },
      columns: match columns {
        Some(value) => value,
        None => defaults.columns,
      },
      width_growth: match early_change {
        Some(0) => console_core_lzw::WidthGrowth::AtCapacity,
        Some(_) | None => defaults.width_growth,
      },
    })
  }
}

#[derive(Clone, Copy)]
enum PredictorKind {
  Tiff,
  Png,
  Unrecognized,
}

#[derive(Clone, Copy)]
struct ChunkLayout {
  bits: u8,
  samples: u16,
}

fn apply_predictor(data: Vec<u8>, params: &PredictorParams) -> Result<Option<Vec<u8>>, Never> {
  let kind = match params.predictor {
    1 => return Ok(Some(data)),
    2 => PredictorKind::Tiff,
    10..=u8::MAX => PredictorKind::Png,
    _unsupported => PredictorKind::Unrecognized,
  };
  let pixel_bits = u16::from(params.colors).saturating_mul(u16::from(params.bits_per_component));
  let stride = params
    .columns
    .saturating_mul(u64::from(pixel_bits))
    .div_ceil(8);
  let row_len = match core::num::NonZeroU64::new(stride) {
    Some(bytes) => bytes.get(),
    None => return Ok(None),
  };
  let input_row_len = match kind {
    PredictorKind::Png => row_len.saturating_add(1),
    PredictorKind::Tiff | PredictorKind::Unrecognized => row_len,
  };
  let Ok(input_len) = fixed_index(data.len());
  let rows = match input_len.checked_div(input_row_len) {
    Some(rows) => rows,
    None => return Ok(None),
  };

  match rows {
    0 => return Ok(Some(Vec::new())),
    1.. => {}
  }

  match params.bits_per_component {
    1 | 2 | 4 | 8 | 16 => {}
    _invalid_depth => {
      warn!("invalid bits per component {}", params.bits_per_component);
      return Ok(None);
    }
  }

  let layout = match kind {
    PredictorKind::Png => ChunkLayout {
      bits: 8,
      samples: pixel_bits.div_ceil(8),
    },
    PredictorKind::Tiff => ChunkLayout {
      bits: params.bits_per_component,
      samples: u16::from(params.colors),
    },
    PredictorKind::Unrecognized => {
      warn!("unknown predictor {}", params.predictor);
      return Ok(None);
    }
  };

  decode_predicted_rows(
    &data,
    kind,
    layout,
    RowLengths {
      row_len,
      input_row_len,
      rows,
    },
  )
}

struct RowLengths {
  row_len: u64,
  input_row_len: u64,
  rows: u64,
}

fn decode_predicted_rows(
  data: &[u8],
  kind: PredictorKind,
  layout: ChunkLayout,
  lengths: RowLengths,
) -> Result<Option<Vec<u8>>, Never> {
  let zero_row = (0..lengths.row_len).map(|_| 0).collect::<Vec<_>>();
  let Ok(samples) = index(layout.samples);
  let Ok(previous) = BitChunks::new(&zero_row, layout.bits, samples);
  let mut prev_row = match previous {
    Some(row) => row,
    None => return Ok(None),
  };
  let Ok(zero_col) = BitChunk::new(0, samples);
  let decoded_size = lengths.rows.saturating_mul(lengths.row_len);
  let mut output = (0..decoded_size).map(|_| 0).collect::<Vec<_>>();
  let Ok(writer_result) = BitWriter::new(&mut output, layout.bits);
  let mut writer = match writer_result {
    Some(writer) => writer,
    None => return Ok(None),
  };
  let Ok(stride) = index(lengths.input_row_len);
  let mut remaining = data;

  for _ in 0..lengths.rows {
    let (row, next) = match remaining.split_at_checked(stride) {
      Some(row) => row,
      None => return Ok(None),
    };
    let Ok(status) = decode_prediction_row(row, kind, layout, prev_row, &zero_col, &mut writer);

    match status {
      Some(()) => {}
      None => return Ok(None),
    }

    let Ok((decoded, next_writer)) = writer.split_off();
    writer = next_writer;
    let Ok(previous_2) = BitChunks::new(decoded, layout.bits, samples);
    prev_row = match previous_2 {
      Some(row_2) => row_2,
      None => return Ok(None),
    };
    remaining = next;
  }

  Ok(Some(output))
}

fn decode_prediction_row<'a>(
  input: &'a [u8],
  kind: PredictorKind,
  layout: ChunkLayout,
  previous: BitChunks<'a>,
  zero: &BitChunk,
  writer: &mut BitWriter<'a>,
) -> Result<Option<()>, Never> {
  let (bytes, filter) = match kind {
    PredictorKind::Png => match input {
      [tag, pixels @ ..] => match tag {
        1 => (pixels, RowFilter::Left),
        2 => (pixels, RowFilter::Above),
        3 => (pixels, RowFilter::Average),
        4 => (pixels, RowFilter::Paeth),
        _copy => {
          for &sample in pixels {
            let Ok(_written) = writer.write(u32::from(sample));
          }

          return Ok(Some(()));
        }
      },
      [] => return Ok(None),
    },
    PredictorKind::Tiff => (input, RowFilter::Left),
    PredictorKind::Unrecognized => return Ok(None),
  };
  let Ok(samples) = index(layout.samples);
  let Ok(chunked) = BitChunks::new(bytes, layout.bits, samples);
  let row = match chunked {
    Some(row) => row,
    None => return Ok(None),
  };
  let Ok(result) = filter_row(filter, layout, row, previous, zero, writer);

  Ok(match kind {
    PredictorKind::Tiff => Some(()),
    PredictorKind::Png | PredictorKind::Unrecognized => result,
  })
}

fn filter_row<'a>(
  filter: RowFilter,
  layout: ChunkLayout,
  input: BitChunks<'a>,
  previous: BitChunks<'a>,
  zero: &BitChunk,
  writer: &mut BitWriter<'a>,
) -> Result<Option<()>, Never> {
  let mut left = zero.clone();
  let mut upper_left = zero.clone();
  let Ok(samples) = index(layout.samples);
  let Ok(mask) = bit_mask(layout.bits);

  for (current, above) in input.zip(previous) {
    let Ok(position) = fixed_index(match writer.cur_pos() {
      Ok(position) => position,
    });
    let Ok(current_values) = current.iter();
    let Ok(above_values) = above.iter();
    let Ok(left_values) = left.iter();
    let Ok(upper_left_values) = upper_left.iter();

    for (((input_2, above_2), left_2), upper_left_2) in current_values
      .zip(above_values)
      .zip(left_values)
      .zip(upper_left_values)
    {
      let Ok(predicted) = filter.predict(PredictionSample {
        input: input_2,
        above: above_2,
        left: left_2,
        upper_left: upper_left_2,
      });
      let Ok(written) = writer.write(u32::from(predicted) & mask);

      match written {
        Some(()) => {}
        None => return Ok(None),
      }
    }

    let Ok(output) = writer.get_data();
    let Ok(offset) = index(position);
    let Ok(mut reader) = BitReader::new_with(output, offset);
    let Ok(written) = BitChunk::from_reader(&mut reader, layout.bits, samples);
    left = match written {
      Some(chunk) => chunk,
      None => return Ok(None),
    };
    upper_left = above;
  }

  Ok(Some(()))
}

enum RowFilter {
  Left,
  Above,
  Average,
  Paeth,
}

struct PredictionSample {
  input: u16,
  left: u16,
  above: u16,
  upper_left: u16,
}

impl RowFilter {
  fn predict(&self, sample: PredictionSample) -> Result<u16, Never> {
    let offset = match self {
      Self::Left => sample.left,
      Self::Above => sample.above,
      Self::Average => {
        let total = u32::from(sample.left).saturating_add(u32::from(sample.above));
        let Ok(average) =
          console_core_number_conversion::wrapped::<_, u16>(total.wrapping_div(2));
        average
      }
      Self::Paeth => {
        let Ok(approximation) = paeth_neighbor(&sample);
        approximation
      }
    };

    Ok(sample.input.wrapping_add(offset))
  }
}

fn paeth_neighbor(sample: &PredictionSample) -> Result<u16, Never> {
  let left = i32::from(sample.left);
  let above = i32::from(sample.above);
  let upper_left = i32::from(sample.upper_left);
  let estimate = left.wrapping_add(above).wrapping_sub(upper_left);
  let left_distance = estimate.wrapping_sub(left).abs();
  let above_distance = estimate.wrapping_sub(above).abs();
  let corner_distance = estimate.wrapping_sub(upper_left).abs();

  Ok(
    match (
      left_distance <= above_distance && left_distance <= corner_distance,
      above_distance <= corner_distance,
    ) {
      (true, _) => sample.left,
      (false, true) => sample.above,
      (false, false) => sample.upper_left,
    },
  )
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use crate::filter::lzw_flate::{PredictorParams, apply_predictor, flate, lzw};
    use crate::object::Dict;

    #[test]
    fn decode_lzw() {
        let input = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        let decoded = lzw::decode(&input, &Dict::default());

        assert_eq!(decoded, Ok(Some(alloc::vec![45, 45, 45, 45, 45, 65, 45, 45, 45, 66])));
    }

    #[test]
    fn decode_flate_zlib() {
        let input = [
            0x78, 0x9c, 0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x7, 0x0, 0x5, 0x8c, 0x1, 0xf5,
        ];

        let decoded = flate::decode(&input, &Dict::default());
        assert_eq!(decoded, Ok(Some(b"Hello".to_vec())));
    }

    #[test]
    fn decode_flate() {
        let input = [0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x7, 0x0];

        let decoded = flate::decode(&input, &Dict::default());
        assert_eq!(decoded, Ok(Some(b"Hello".to_vec())));
    }

    fn predictor_expected() -> Result<alloc::vec::Vec<u8>, console_core_never::Never> {
        Ok(alloc::vec![

            127, 127, 127, 125, 129, 127, 123, 130, 128,

            128, 129, 126, 126, 132, 124, 121, 127, 126,

            131, 130, 122, 133, 129, 128, 127, 100, 126,
        ])
    }

    fn predictor_test(predictor: u8, input: &[u8]) -> Result<(), console_core_never::Never> {
        let params = PredictorParams {
            predictor,
            colors: 3,
            bits_per_component: 8,
            columns: 3,
            width_growth: console_core_lzw::WidthGrowth::AtCapacity,
        };

        let Ok(expected) = predictor_expected();
        let Ok(out) = apply_predictor(input.to_vec(), &params);

        assert_eq!(Some(expected), out);

        Ok(())
    }

    #[test]
    fn predictor_sub() {
        let Ok(()) = predictor_test(
            11,
            &[

                1, 127, 127, 127, 254, 2, 0, 254, 1, 1,

                1, 128, 129, 126, 254, 3, 254, 251, 251, 2,

                1, 131, 130, 122, 2, 255, 6, 250, 227, 254,
            ],
        );
    }

    #[test]
    fn predictor_up() {
        let Ok(()) = predictor_test(
            12,
            &[

                2, 127, 127, 127, 125, 129, 127, 123, 130, 128,

                2, 1, 2, 255, 1, 3, 253, 254, 253, 254,

                2, 3, 1, 252, 7, 253, 4, 6, 229, 0,
            ],
        );
    }

    #[test]
    fn predictor_avg() {
        let Ok(()) = predictor_test(
            13,
            &[

                3, 127, 127, 127, 62, 66, 64, 61, 66, 65,

                3, 65, 66, 63, 0, 3, 254, 253, 252, 0,

                3, 67, 66, 59, 5, 254, 5, 0, 228, 255,
            ],
        );
    }

    #[test]
    fn predictor_paeth() {
        let Ok(()) = predictor_test(
            14,
            &[

                4, 127, 127, 127, 254, 2, 0, 254, 1, 1,

                4, 1, 2, 255, 1, 3, 254, 254, 251, 2,

                4, 3, 1, 252, 5, 253, 6, 1, 229, 254,
            ],
        );
    }
}
