mod common;

use std::num::TryFromIntError;

use common::{Failure, describe, length, samples};
use console_core_audio_decoding_wave_container::WavReader;
use console_core_never::Never;

const RATE: u32 = 8000;
const PCM: u16 = 1;
const ADPCM: u16 = 2;
const FLOAT: u16 = 3;
const ALAW: u16 = 6;
const MULAW: u16 = 7;
const IMA: u16 = 0x11;
const EXTENSIBLE: u16 = 0xfffe;

#[derive(Clone, Copy)]
enum Kind {
  Pcm,
  Float,
  Alaw,
  Mulaw,
  Adpcm,
  AmbisonicPcm,
  AmbisonicFloat,
}

struct Ext {
  channels: u16,
  align: u16,
  coded: u16,
  valid: u16,
  mask: u32,
  kind: Kind,
}

struct Fmt<'a> {
  format: u16,
  channels: u16,
  align: u16,
  bits: u16,
  extra: &'a [u8],
}

enum Piece {
  Format(Fmt<'static>),
  Extensible(Ext),
  Data {
    declared: Option<u32>,
    count: u32,
  },
  Chunk {
    tag: [u8; 4],
    body: &'static [u8],
  },
  List {
    form: [u8; 4],
    children: &'static [([u8; 4], &'static [u8])],
  },
}

enum Tweak {
  Keep,
  Truncated,
  Marker([u8; 4]),
  Form([u8; 4]),
  RiffLength(u32),
  Replace(&'static [u8]),
}

struct Case {
  name: &'static str,
  pieces: &'static [Piece],
  tweak: Tweak,
}

fn chunk(tag: [u8; 4], declared: u32, body: &[u8]) -> Result<Vec<u8>, Never> {
  let mut bytes = tag.to_vec();

  bytes.extend(declared.to_le_bytes());
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

fn guid(kind: Kind) -> Result<[u8; 16], Never> {
  let standard = |first: u8| {
    [
      first, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
    ]
  };
  let ambisonic = |first: u8| {
    [
      first, 0, 0, 0, 0x21, 0x07, 0xd3, 0x11, 0x86, 0x44, 0xc8, 0xc1, 0xca, 0, 0, 0,
    ]
  };

  Ok(match kind {
    Kind::Pcm => standard(1),
    Kind::Adpcm => standard(2),
    Kind::Float => standard(3),
    Kind::Mulaw => standard(4),
    Kind::Alaw => standard(6),
    Kind::AmbisonicPcm => ambisonic(1),
    Kind::AmbisonicFloat => ambisonic(3),
  })
}

fn format_body(fmt: &Fmt<'_>) -> Result<Vec<u8>, Never> {
  let mut body = Vec::new();

  body.extend(fmt.format.to_le_bytes());
  body.extend(fmt.channels.to_le_bytes());
  body.extend(RATE.to_le_bytes());
  body.extend(RATE.saturating_mul(u32::from(fmt.align)).to_le_bytes());
  body.extend(fmt.align.to_le_bytes());
  body.extend(fmt.bits.to_le_bytes());
  body.extend(fmt.extra);

  Ok(body)
}

fn extensible_body(ext: &Ext) -> Result<Vec<u8>, Never> {
  let Ok(identifier) = guid(ext.kind);
  let mut extra = 22u16.to_le_bytes().to_vec();

  extra.extend(ext.valid.to_le_bytes());
  extra.extend(ext.mask.to_le_bytes());
  extra.extend(identifier);

  format_body(&Fmt {
    format: EXTENSIBLE,
    channels: ext.channels,
    align: ext.align,
    bits: ext.coded,
    extra: &extra,
  })
}

fn list_body(form: [u8; 4], children: &[([u8; 4], &[u8])]) -> Result<Vec<u8>, TryFromIntError> {
  let mut body = form.to_vec();

  for (tag, value) in children {
    let child = honest(*tag, value)?;

    body.extend(child);
  }

  honest(*b"LIST", &body)
}

fn piece_bytes(piece: &Piece) -> Result<Vec<u8>, TryFromIntError> {
  match piece {
    Piece::Format(fmt) => {
      let Ok(body) = format_body(fmt);

      honest(*b"fmt ", &body)
    }
    Piece::Extensible(ext) => {
      let Ok(body) = extensible_body(ext);

      honest(*b"fmt ", &body)
    }
    Piece::Data {
      declared: None,
      count,
    } => {
      let Ok(sound) = samples(*count);

      honest(*b"data", &sound)
    }
    Piece::Data {
      declared: Some(declared),
      count,
    } => {
      let Ok(sound) = samples(*count);
      let Ok(bytes) = chunk(*b"data", *declared, &sound);

      Ok(bytes)
    }
    Piece::Chunk { tag, body } => honest(*tag, body),
    Piece::List { form, children } => list_body(*form, children),
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
  let mut file = b"RIFF".to_vec();

  file.extend(declared.to_le_bytes());
  file.extend(b"WAVE");
  file.extend(body);

  match &case.tweak {
    Tweak::Keep => (),
    Tweak::Truncated => file.truncate(2000),
    Tweak::Marker(marker) => file.splice(0..4, *marker).for_each(drop),
    Tweak::Form(form) => file.splice(8..12, *form).for_each(drop),
    Tweak::RiffLength(size) => file.splice(4..8, size.to_le_bytes()).for_each(drop),
    Tweak::Replace(bytes) => file = bytes.to_vec(),
  }

  Ok(file)
}

mod fixtures {
  use super::{
    ADPCM, ALAW, Case, EXTENSIBLE, Ext, FLOAT, Fmt, IMA, Kind, MULAW, PCM, Piece, Tweak,
  };

  pub(super) const CASES: &[&[Case]] = &[
    PCM_CASES,
    FLOAT_CASES,
    ADPCM_CASES,
    EXTENSIBLE_CASES,
    BROKEN_CASES,
  ];

  const PCM_CASES: &[Case] = &[
    Case {
      name: "pcm 16 stereo",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm 8 mono",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 1,
          align: 1,
          bits: 8,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 3001,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm 24 with empty extension",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 6,
          bits: 24,
          extra: &[0; 2],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm 32 with extension block",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 8,
          bits: 32,
          extra: &[0; 24],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm 12 bits",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 12,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm no channels",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 0,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm too many channels",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 33,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm odd fmt length",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[0],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm fact",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Chunk {
          tag: *b"fact",
          body: &[220, 5, 0, 0],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm bad fact",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Chunk {
          tag: *b"fact",
          body: &[1, 2, 3, 4, 5, 6],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm info list",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::List {
          form: *b"INFO",
          children: &[
            (*b"INAM", b"Title\0"),
            (*b"IART", b"Artist\0"),
            (*b"ICMT", b"odd"),
          ],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm other list",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::List {
          form: *b"adtl",
          children: &[(*b"labl", b"abcd")],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm empty list",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::List {
          form: *b"INFO",
          children: &[],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm short list",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Chunk {
          tag: *b"LIST",
          body: &[73, 78],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm unknown chunks",
      pieces: &[
        Piece::Chunk {
          tag: *b"junk",
          body: &[1, 2, 3],
        },
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Chunk {
          tag: *b"cue ",
          body: &[0; 10],
        },
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm no data",
      pieces: &[Piece::Format(Fmt {
        format: PCM,
        channels: 2,
        align: 4,
        bits: 16,
        extra: &[],
      })],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm data before format",
      pieces: &[
        Piece::Data {
          declared: None,
          count: 100,
        },
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm no format",
      pieces: &[Piece::Data {
        declared: None,
        count: 100,
      }],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm zero block",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 0,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm empty data",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 0,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm short fmt",
      pieces: &[
        Piece::Chunk {
          tag: *b"fmt ",
          body: &[1, 0, 2, 0],
        },
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm truncated data",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 6000,
        },
      ],
      tweak: Tweak::Truncated,
    },
    Case {
      name: "pcm chunk past parent",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: Some(90000),
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "pcm streamed",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: Some(u32::MAX),
          count: 6000,
        },
      ],
      tweak: Tweak::RiffLength(u32::MAX),
    },
  ];

  const FLOAT_CASES: &[Case] = &[
    Case {
      name: "float 32",
      pieces: &[
        Piece::Format(Fmt {
          format: FLOAT,
          channels: 2,
          align: 8,
          bits: 32,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "float 64 with empty extension",
      pieces: &[
        Piece::Format(Fmt {
          format: FLOAT,
          channels: 2,
          align: 16,
          bits: 64,
          extra: &[0; 2],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "float 32 with extension block",
      pieces: &[
        Piece::Format(Fmt {
          format: FLOAT,
          channels: 2,
          align: 8,
          bits: 32,
          extra: &[0; 24],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "float with extra data",
      pieces: &[
        Piece::Format(Fmt {
          format: FLOAT,
          channels: 2,
          align: 8,
          bits: 32,
          extra: &[1, 0],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "float 16 bits",
      pieces: &[
        Piece::Format(Fmt {
          format: FLOAT,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "float odd length",
      pieces: &[
        Piece::Format(Fmt {
          format: FLOAT,
          channels: 2,
          align: 8,
          bits: 32,
          extra: &[0],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "alaw",
      pieces: &[
        Piece::Format(Fmt {
          format: ALAW,
          channels: 2,
          align: 2,
          bits: 8,
          extra: &[0; 2],
        }),
        Piece::Data {
          declared: None,
          count: 3000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "alaw with extra",
      pieces: &[
        Piece::Format(Fmt {
          format: ALAW,
          channels: 2,
          align: 2,
          bits: 8,
          extra: &[2, 0, 9, 9],
        }),
        Piece::Data {
          declared: None,
          count: 3000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "alaw without extension",
      pieces: &[
        Piece::Format(Fmt {
          format: ALAW,
          channels: 2,
          align: 2,
          bits: 8,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 3000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "mulaw",
      pieces: &[
        Piece::Format(Fmt {
          format: MULAW,
          channels: 1,
          align: 1,
          bits: 8,
          extra: &[0; 2],
        }),
        Piece::Data {
          declared: None,
          count: 3000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "mulaw without extension",
      pieces: &[
        Piece::Format(Fmt {
          format: MULAW,
          channels: 1,
          align: 1,
          bits: 8,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 3000,
        },
      ],
      tweak: Tweak::Keep,
    },
  ];

  const ADPCM_CASES: &[Case] = &[
    Case {
      name: "ms adpcm",
      pieces: &[
        Piece::Format(Fmt {
          format: ADPCM,
          channels: 2,
          align: 2048,
          bits: 4,
          extra: &[
            32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
          ],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ms adpcm mono",
      pieces: &[
        Piece::Format(Fmt {
          format: ADPCM,
          channels: 1,
          align: 512,
          bits: 4,
          extra: &[
            32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
          ],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ms adpcm short extra",
      pieces: &[
        Piece::Format(Fmt {
          format: ADPCM,
          channels: 2,
          align: 2048,
          bits: 4,
          extra: &[
            30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
          ],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ms adpcm wrong bits",
      pieces: &[
        Piece::Format(Fmt {
          format: ADPCM,
          channels: 2,
          align: 2048,
          bits: 8,
          extra: &[
            32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
          ],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ms adpcm no extension",
      pieces: &[
        Piece::Format(Fmt {
          format: ADPCM,
          channels: 2,
          align: 2048,
          bits: 4,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ima adpcm",
      pieces: &[
        Piece::Format(Fmt {
          format: IMA,
          channels: 2,
          align: 2048,
          bits: 4,
          extra: &[2, 0, 0, 0],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ima adpcm wrong extra",
      pieces: &[
        Piece::Format(Fmt {
          format: IMA,
          channels: 2,
          align: 2048,
          bits: 4,
          extra: &[3, 0, 0, 0, 0],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "ima adpcm mono",
      pieces: &[
        Piece::Format(Fmt {
          format: IMA,
          channels: 1,
          align: 256,
          bits: 4,
          extra: &[2, 0, 0, 0],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
  ];

  const EXTENSIBLE_CASES: &[Case] = &[
    Case {
      name: "extensible pcm 24 of 20",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 6,
          coded: 24,
          valid: 20,
          mask: 3,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible pcm 16",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 4,
          coded: 16,
          valid: 16,
          mask: 3,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible pcm 8",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 2,
          coded: 8,
          valid: 8,
          mask: 3,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible pcm 32",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 6,
          align: 24,
          coded: 32,
          valid: 32,
          mask: 0x3f,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible pcm 40",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 10,
          coded: 40,
          valid: 40,
          mask: 3,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible pcm 12",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 4,
          coded: 12,
          valid: 12,
          mask: 3,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible pcm valid above coded",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 4,
          coded: 16,
          valid: 24,
          mask: 3,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible ambisonic pcm",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 4,
          align: 8,
          coded: 16,
          valid: 16,
          mask: 0xf,
          kind: Kind::AmbisonicPcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible mask too few",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 6,
          align: 24,
          coded: 32,
          valid: 32,
          mask: 0b101,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible mask too many",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 4,
          coded: 16,
          valid: 16,
          mask: 0xff,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible mask empty",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 3,
          align: 6,
          coded: 16,
          valid: 16,
          mask: 0,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible no channels",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 0,
          align: 0,
          coded: 16,
          valid: 16,
          mask: 0xff,
          kind: Kind::Pcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible float 32",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 8,
          coded: 32,
          valid: 32,
          mask: 3,
          kind: Kind::Float,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible float 64",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 16,
          coded: 64,
          valid: 64,
          mask: 3,
          kind: Kind::Float,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible float 16",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 4,
          coded: 16,
          valid: 16,
          mask: 3,
          kind: Kind::Float,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible float truncated",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 8,
          coded: 32,
          valid: 24,
          mask: 3,
          kind: Kind::Float,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible ambisonic float",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 4,
          align: 16,
          coded: 32,
          valid: 32,
          mask: 0xf,
          kind: Kind::AmbisonicFloat,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible alaw",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 2,
          coded: 8,
          valid: 8,
          mask: 3,
          kind: Kind::Alaw,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible mulaw",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 2,
          coded: 8,
          valid: 8,
          mask: 3,
          kind: Kind::Mulaw,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible adpcm",
      pieces: &[
        Piece::Extensible(Ext {
          channels: 2,
          align: 2,
          coded: 8,
          valid: 8,
          mask: 3,
          kind: Kind::Adpcm,
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "extensible wrong extra size",
      pieces: &[
        Piece::Format(Fmt {
          format: EXTENSIBLE,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[
            21, 0, 16, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113, 0,
          ],
        }),
        Piece::Data {
          declared: None,
          count: 9000,
        },
      ],
      tweak: Tweak::Keep,
    },
  ];

  const BROKEN_CASES: &[Case] = &[
    Case {
      name: "extensible short",
      pieces: &[
        Piece::Format(Fmt {
          format: EXTENSIBLE,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[22, 0],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "unsupported format",
      pieces: &[
        Piece::Format(Fmt {
          format: 0x55,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Keep,
    },
    Case {
      name: "not riff",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Marker(*b"RIFX"),
    },
    Case {
      name: "not wave",
      pieces: &[
        Piece::Format(Fmt {
          format: PCM,
          channels: 2,
          align: 4,
          bits: 16,
          extra: &[],
        }),
        Piece::Data {
          declared: None,
          count: 100,
        },
      ],
      tweak: Tweak::Form(*b"AVI "),
    },
    Case {
      name: "empty",
      pieces: &[],
      tweak: Tweak::Replace(&[]),
    },
    Case {
      name: "marker only",
      pieces: &[],
      tweak: Tweak::Replace(b"RIFF"),
    },
    Case {
      name: "length",
      pieces: &[],
      tweak: Tweak::Replace(&[3, 0, 0, 0]),
    },
  ];
}

#[test]
fn every_wave_reads_as_it_always_did() -> Result<(), Failure> {
  let mut dump = String::new();

  for case in fixtures::CASES.iter().copied().flatten() {
    let bytes = file_bytes(case)?;
    let Ok(text) = describe::<WavReader>(bytes);

    dump.push_str(&format!("## {}\n{text}\n", case.name));
  }

  assert_eq!(dump, include_str!("goldens/waves.txt"));

  Ok(())
}
