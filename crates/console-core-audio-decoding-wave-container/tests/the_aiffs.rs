mod common;

use std::num::TryFromIntError;

use common::{Failure, describe, length, samples};
use console_core_audio_decoding_wave_container::AiffReader;
use console_core_never::Never;

enum Rate {
  Whole(u32),
  Raw {
    sign: u8,
    exponent: u16,
    mantissa: u64,
  },
}

enum Tail {
  None,
  Aifc {
    compression: [u8; 4],
    name: &'static [u8],
  },
  Raw(&'static [u8]),
}

enum Piece {
  Comm {
    channels: i16,
    bits: i16,
    rate: Rate,
    tail: Tail,
  },
  Sound {
    offset: u32,
    block: u32,
    count: u32,
  },
  Chunk {
    tag: [u8; 4],
    body: &'static [u8],
  },
  Declared {
    tag: [u8; 4],
    declared: u32,
    body: &'static [u8],
  },
}

enum Tweak {
  Keep,
  Truncated,
  Marker([u8; 4]),
  Replace(&'static [u8]),
}

struct Case {
  name: &'static str,
  form: [u8; 4],
  pieces: &'static [Piece],
  tweak: Tweak,
}

fn chunk(tag: [u8; 4], declared: u32, body: &[u8]) -> Result<Vec<u8>, Never> {
  let mut bytes = tag.to_vec();

  bytes.extend(declared.to_be_bytes());
  bytes.extend(body);

  match body.len() & 1 {
    1 => bytes.push(0),
    _ => (),
  }

  Ok(bytes)
}

fn honest(tag: [u8; 4], body: &[u8]) -> Result<Vec<u8>, TryFromIntError> {
  let declared = length(body)?;
  let Ok(bytes) = chunk(tag, declared, body);

  Ok(bytes)
}

fn extended(sign: u8, exponent: u16, mantissa: u64) -> Result<Vec<u8>, Never> {
  let head = exponent | u16::from(sign).wrapping_shl(15);
  let mut bytes = head.to_be_bytes().to_vec();

  bytes.extend(mantissa.to_be_bytes());

  Ok(bytes)
}

fn rate_bytes(rate: &Rate) -> Result<Vec<u8>, TryFromIntError> {
  let (sign, exponent, mantissa) = match rate {
    Rate::Raw {
      sign,
      exponent,
      mantissa,
    } => (*sign, *exponent, *mantissa),
    Rate::Whole(0) => (0, 0, 0),
    Rate::Whole(value) => {
      let shift = value.leading_zeros();
      let exponent = u16::try_from(16414u32.saturating_sub(shift))?;

      (
        0,
        exponent,
        u64::from(*value).wrapping_shl(32u32.saturating_add(shift)),
      )
    }
  };
  let Ok(bytes) = extended(sign, exponent, mantissa);

  Ok(bytes)
}

fn pascal(name: &[u8]) -> Result<Vec<u8>, TryFromIntError> {
  let size = u8::try_from(name.len())?;
  let mut text = vec![size];

  text.extend(name);

  match text.len() & 1 {
    1 => text.push(0),
    _ => (),
  }

  Ok(text)
}

fn tail_bytes(tail: &Tail) -> Result<Vec<u8>, TryFromIntError> {
  match tail {
    Tail::None => Ok(Vec::new()),
    Tail::Raw(bytes) => Ok(bytes.to_vec()),
    Tail::Aifc { compression, name } => {
      let mut bytes = compression.to_vec();

      let text = pascal(name)?;

      bytes.extend(text);

      Ok(bytes)
    }
  }
}

fn piece_bytes(piece: &Piece) -> Result<Vec<u8>, TryFromIntError> {
  match piece {
    Piece::Comm {
      channels,
      bits,
      rate,
      tail,
    } => {
      let written = rate_bytes(rate)?;
      let extra = tail_bytes(tail)?;
      let mut body = channels.to_be_bytes().to_vec();

      body.extend(1000u32.to_be_bytes());
      body.extend(bits.to_be_bytes());
      body.extend(written);
      body.extend(extra);

      honest(*b"COMM", &body)
    }
    Piece::Sound {
      offset,
      block,
      count,
    } => {
      let mut body = offset.to_be_bytes().to_vec();
      let Ok(sound) = samples(*count);

      body.extend(block.to_be_bytes());
      body.extend(sound);

      honest(*b"SSND", &body)
    }
    Piece::Chunk { tag, body } => honest(*tag, body),
    Piece::Declared {
      tag,
      declared,
      body,
    } => {
      let Ok(bytes) = chunk(*tag, *declared, body);

      Ok(bytes)
    }
  }
}

fn file_bytes(case: &Case) -> Result<Vec<u8>, TryFromIntError> {
  let mut body = Vec::new();

  for piece in case.pieces {
    let bytes = piece_bytes(piece)?;

    body.extend(bytes);
  }

  let body_len = length(&body)?;
  let declared = body_len.saturating_add(4);
  let mut file = b"FORM".to_vec();

  file.extend(declared.to_be_bytes());
  file.extend(case.form);
  file.extend(body);

  match &case.tweak {
    Tweak::Keep => (),
    Tweak::Truncated => file.truncate(2000),
    Tweak::Marker(marker) => file.splice(0..4, *marker).for_each(drop),
    Tweak::Replace(bytes) => file = bytes.to_vec(),
  }

  Ok(file)
}

mod fixtures {
  use super::{Case, Piece, Rate, Tail, Tweak};

  pub(super) const CASES: &[&[Case]] = &[INTEGER_CASES, CHUNK_CASES, AIFC_CASES];

  const INTEGER_CASES: &[Case] = &[
    Case {
      name: "aiff 16 stereo",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff 8 mono",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 1,
          bits: 8,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 3001,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff 24 stereo",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 24,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff 32 stereo",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 32,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff 12 bits",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 12,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff no channels",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 0,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff negative channels",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: -2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff negative bits",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: -16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff rate 8000",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(8000),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff rate 1",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(1),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff rate max",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(u32::MAX),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff rate past max",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Raw {
            sign: 0,
            exponent: 16423,
            mantissa: 0x8000_0000_0000_0000,
          },
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff rate fraction",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Raw {
            sign: 0,
            exponent: 16397,
            mantissa: 0xac44_8000_0000_0000,
          },
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff offset",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 4,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff block size",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 512,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff unknown chunks",
      form: *b"AIFF",
      pieces: &[
        Piece::Chunk {
          tag: *b"NAME",
          body: b"song",
        },
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Chunk {
          tag: *b"ANNO",
          body: b"abc",
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
  ];

  const CHUNK_CASES: &[Case] = &[
    Case {
      name: "aiff no sound",
      form: *b"AIFF",
      pieces: &[Piece::Comm {
        channels: 2,
        bits: 16,
        rate: Rate::Whole(44100),
        tail: Tail::None,
      }],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff no common",
      form: *b"AIFF",
      pieces: &[Piece::Sound {
        offset: 0,
        block: 0,
        count: 6000,
      }],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff empty sound",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 0,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff short common",
      form: *b"AIFF",
      pieces: &[
        Piece::Chunk {
          tag: *b"COMM",
          body: &[0, 2, 0, 0],
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aiff truncated",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Truncated,
    },
    Case {
      name: "aiff chunk past parent",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Declared {
          tag: *b"SSND",
          declared: 90000,
          body: &[0; 8],
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc none",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"NONE",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc none lowercase 24",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 24,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"none",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc none 12 bits",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 12,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"NONE",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc sowt",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"sowt",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc sowt uppercase",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"SOWT",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc sowt 8 bits",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 8,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"sowt",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
  ];

  const AIFC_CASES: &[Case] = &[
    Case {
      name: "aifc twos",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"twos",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc twos 24 bits",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 24,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"twos",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc fl32",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 32,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"fl32",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc fl64",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 64,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"fl64",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc fl32 16 bits",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"fl32",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc alaw",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"alaw",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc alaw uppercase",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 1,
          bits: 8,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"ALAW",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc ulaw",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"ulaw",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc ulaw uppercase",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 1,
          bits: 8,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"ULAW",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc unsupported",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"ima4",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc even name",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"NONE",
            name: b"odd",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc empty name",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"NONE",
            name: &[],
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc long name",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"NONE",
            name: &[b'x'; 200],
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc name past chunk",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Raw(&[b'N', b'O', b'N', b'E', 253]),
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc no name",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Raw(b"NONE"),
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "aifc no channels",
      form: *b"AIFC",
      pieces: &[
        Piece::Comm {
          channels: 0,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::Aifc {
            compression: *b"NONE",
            name: b"not compressed",
          },
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "form unknown",
      form: *b"8SVX",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "not form",
      form: *b"AIFF",
      pieces: &[
        Piece::Comm {
          channels: 2,
          bits: 16,
          rate: Rate::Whole(44100),
          tail: Tail::None,
        },
        Piece::Sound {
          offset: 0,
          block: 0,
          count: 100,
        },
      ],
      tweak: Tweak::Marker(*b"RIFF"),
    },
    Case {
      name: "empty",
      form: *b"AIFF",
      pieces: &[],
      tweak: Tweak::Replace(&[]),
    },
    Case {
      name: "marker only",
      form: *b"AIFF",
      pieces: &[],
      tweak: Tweak::Replace(b"FORM"),
    },
  ];
}

#[test]
fn every_aiff_reads_as_it_always_did() -> Result<(), Failure> {
  let mut dump = String::new();

  for case in fixtures::CASES.iter().copied().flatten() {
    let bytes = file_bytes(case)?;
    let Ok(text) = describe::<AiffReader>(bytes);

    dump.push_str(&format!("## {}\n{text}\n", case.name));
  }

  assert_eq!(dump, include_str!("goldens/aiffs.txt"));

  Ok(())
}
