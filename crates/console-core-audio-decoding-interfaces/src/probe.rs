//! Finding which reader a stream wants, from what it starts with.
//!
//! Every demuxer and metadata reader declares the byte strings its stream can
//! begin with, a marker of two to sixteen bytes. A probe keeps a small bloom
//! filter of the first two bytes of every marker, so a byte stream is
//! scanned a byte at a time against one hash instead of against every marker,
//! and only a window the filter might know is read out and compared with the
//! real ones. A metadata reader found this way is run, and the scan goes on
//! behind it, because a stream may carry tags in front of the format.

use std::cmp::Ordering;
use std::ops::ControlFlow;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::errors::{Error, Result, unsupported_error};
use crate::formats::{FormatOptions, FormatReader};
use crate::io::{MediaSourceStream, ReadBytes, SeekBuffered};
use crate::meta::{Metadata, MetadataLog, MetadataOptions, MetadataReader};

type Infallible<T> = std::result::Result<T, Never>;

const PROBE_SEARCH_LIMIT: u64 = 1024 * 1024;
const FILTER_BITS: u32 = 2 * 1024 * 8;

enum Membership {
  Absent,
  Possible,
}

impl From<bool> for Membership {
  fn from(present: bool) -> Self {
    match bool::then_some(present, ()) {
      Some(()) => Self::Possible,
      None => Self::Absent,
    }
  }
}

struct BloomFilter {
  filter: Vec<u64>,
}

impl Default for BloomFilter {
  fn default() -> Self {
    BloomFilter {
      filter: vec![0; 256],
    }
  }
}

fn slots(key: [u8; 2]) -> Infallible<[u32; 3]> {
  let hash = key.iter().fold(0x811c_9dc5_u32, |state, byte| {
    (state ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
  });
  let [high_a, high_b, low_a, low_b] = hash.to_be_bytes();
  let h0 = u16::from_be_bytes([high_a, high_b]);
  let h1 = u16::from_be_bytes([low_a, low_b]);
  let mask = FILTER_BITS.wrapping_sub(1);

  Ok([
    u32::from(h0) & mask,
    u32::from(h0.wrapping_add(h1.wrapping_mul(1))) & mask,
    u32::from(h0.wrapping_add(h1.wrapping_mul(2))) & mask,
  ])
}

impl BloomFilter {
  fn insert(&mut self, key: [u8; 2]) -> Infallible<()> {
    let Ok(chosen) = slots(key);

    for slot in chosen {
      let Ok(at) = index(slot.wrapping_shr(6));

      match self.filter.get_mut(at) {
        Some(word) => *word |= 1u64.wrapping_shl(slot & 63),
        None => {}
      }
    }

    Ok(())
  }

  fn may_contain(&self, key: [u8; 2]) -> Infallible<Membership> {
    let Ok(chosen) = slots(key);

    let present = chosen.iter().all(|slot| {
      let Ok(at) = index(slot.wrapping_shr(6));

      self
        .filter
        .get(at)
        .is_some_and(|word| word & 1u64.wrapping_shl(slot & 63) != 0)
    });

    Ok(Membership::from(present))
  }
}

#[derive(Copy, Clone)]
pub enum Instantiate {
  Format(fn(MediaSourceStream, &FormatOptions) -> Result<Box<dyn FormatReader>>),
  Metadata(fn(&MetadataOptions) -> Infallible<Box<dyn MetadataReader>>),
}

#[derive(Copy, Clone)]
pub struct Descriptor {
  pub short_name: &'static str,
  pub long_name: &'static str,
  pub extensions: &'static [&'static str],
  pub mime_types: &'static [&'static str],
  pub markers: &'static [&'static [u8]],
  pub score: fn(&[u8]) -> Infallible<u8>,
  pub inst: Instantiate,
}

pub trait QueryDescriptor {
  fn query() -> std::result::Result<&'static [Descriptor], console_core_never::Never>;

  fn score(context: &[u8]) -> std::result::Result<u8, console_core_never::Never>;
}

#[derive(Clone, Debug, Default)]
pub struct Hint {
  pub extension: Option<String>,
  pub mime_type: Option<String>,
}

impl Hint {
  #[must_use]
  pub fn new() -> std::result::Result<Self, console_core_never::Never> {
    Ok(Hint::default())
  }

  pub fn with_extension(
    &mut self,
    extension: &str,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.extension = Some(extension.to_owned());
    Ok(self)
  }

  pub fn mime_type(
    &mut self,
    mime_type: &str,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.mime_type = Some(mime_type.to_owned());
    Ok(self)
  }
}

pub struct ProbedMetadata {
  metadata: Option<MetadataLog>,
}

impl ProbedMetadata {
  pub fn get(&mut self) -> std::result::Result<Option<Metadata<'_>>, console_core_never::Never> {
    Ok(self.metadata.as_mut().map(|metadata| {
      let Ok(current) = metadata.metadata();

      current
    }))
  }

  #[must_use]
  pub fn into_inner(self) -> std::result::Result<Option<MetadataLog>, console_core_never::Never> {
    Ok(self.metadata)
  }
}

pub struct ProbeResult {
  pub format: Box<dyn FormatReader>,
  pub metadata: ProbedMetadata,
}

#[derive(Default)]
pub struct Probe {
  filter: BloomFilter,
  registered: Vec<Descriptor>,
}

enum Scanned {
  Found(Instantiate),
  Failed(Error),
  Exhausted,
}

fn matched(registered: &[Descriptor], context: &[u8; 16]) -> Infallible<Option<Instantiate>> {
  Ok(
    registered
      .iter()
      .flat_map(|descriptor| {
        descriptor
          .markers
          .iter()
          .map(|marker| (*marker, descriptor.inst))
      })
      .find(|(marker, _)| context.starts_with(marker))
      .map(|(_, inst)| inst),
  )
}

type Scan<'a> = ControlFlow<Scanned, (&'a mut MediaSourceStream, u16)>;

impl Probe {
  pub fn register_all<Q: QueryDescriptor>(
    &mut self,
  ) -> std::result::Result<(), console_core_never::Never> {
    let Ok(descriptors) = Q::query();

    for descriptor in descriptors {
      let Ok(()) = self.register(descriptor);
    }

    Ok(())
  }

  pub fn register(
    &mut self,
    descriptor: &Descriptor,
  ) -> std::result::Result<(), console_core_never::Never> {
    let usable = descriptor
      .markers
      .iter()
      .filter(|marker| marker.len() <= 16);

    for marker in usable {
      match marker.first_chunk::<2>() {
        Some(prefix) => {
          let Ok(()) = self.filter.insert(*prefix);
        }
        None => {}
      }
    }

    self.registered.push(*descriptor);

    Ok(())
  }

  fn scan<'a>(
    &self,
    mss: &'a mut MediaSourceStream,
    count: u64,
    window: u16,
  ) -> Infallible<Scan<'a>> {
    let byte = match mss.read_byte() {
      Ok(byte) => byte,
      Err(_ended) => return Ok(ControlFlow::Break(Scanned::Exhausted)),
    };
    let window_2 = window.wrapping_shl(8) | u16::from(byte);

    match count.cmp(&PROBE_SEARCH_LIMIT) {
      Ordering::Greater => return Ok(ControlFlow::Break(Scanned::Exhausted)),
      Ordering::Less | Ordering::Equal => {}
    }

    let [first, second] = window_2.to_be_bytes();
    let Ok(membership) = self.filter.may_contain([first, second]);

    match membership {
      Membership::Absent => Ok(ControlFlow::Continue((mss, window_2))),
      Membership::Possible => {
        let mut context = [0u8; 16];
        let [head, tail, rest @ ..] = &mut context;

        *head = first;
        *tail = second;

        match mss.read_buf_exact(rest) {
          Ok(()) => {}
          Err(fault) => return Ok(ControlFlow::Break(Scanned::Failed(Error::from(fault)))),
        }

        let Ok(found) = matched(&self.registered, &context);

        Ok(match found {
          Some(inst) => {
            let Ok(()) = mss.seek_buffered_rev(16);

            ControlFlow::Break(Scanned::Found(inst))
          }
          None => {
            let Ok(()) = mss.seek_buffered_rev(14);

            ControlFlow::Continue((mss, window_2))
          }
        })
      }
    }
  }

  pub fn next(&self, mss: &mut MediaSourceStream) -> Result<Instantiate> {
    let flow = (1..=PROBE_SEARCH_LIMIT.saturating_add(1)).try_fold(
      (mss, 0u16),
      |(mss_2, window), count| {
        let Ok(step) = self.scan(mss_2, count, window);

        step
      },
    );

    match flow {
      ControlFlow::Break(Scanned::Found(inst)) => Ok(inst),
      ControlFlow::Break(Scanned::Failed(fault)) => Err(fault),
      ControlFlow::Break(Scanned::Exhausted) | ControlFlow::Continue(_) => {
        unsupported_error("core (probe): no suitable format reader found")
      }
    }
  }

  pub fn format(
    &self,
    _hint: &Hint,
    mss: MediaSourceStream,
    format_opts: &FormatOptions,
    metadata_opts: &MetadataOptions,
  ) -> Result<ProbeResult> {
    let stepped = iterate(
      (mss, MetadataLog::default()),
      |(mut mss_2, mut metadata)| {
        Ok(match self.next(&mut mss_2) {
          Err(fault) => Step::Halt(Err(fault)),
          Ok(Instantiate::Format(fmt)) => match fmt(mss_2, format_opts) {
            Ok(format) => {
              let Ok(current) = metadata.metadata();
              let Ok(revision) = current.current();
              let metadata_2 = match revision {
                Some(_revision) => Some(metadata),
                None => None,
              };

              Step::Halt(Ok(ProbeResult {
                format,
                metadata: ProbedMetadata {
                  metadata: metadata_2,
                },
              }))
            }
            Err(fault) => Step::Halt(Err(fault)),
          },
          Ok(Instantiate::Metadata(meta)) => {
            let Ok(mut reader) = meta(metadata_opts);

            match reader.read_all(&mut mss_2) {
              Ok(revision) => {
                let Ok(()) = metadata.push(revision);

                Step::Again((mss_2, metadata))
              }
              Err(fault) => Step::Halt(Err(fault)),
            }
          }
        })
      },
    );

    match stepped {
      Ok(outcome) => outcome,
      Err(_endless) => unsupported_error("core (probe): no suitable format reader found"),
    }
  }
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "every demuxer in the family declares what it can read with this one definition, which builds its descriptor from its own `try_new` and `score`"
  )
)]
macro_rules! support_format {
  ($short_name:expr, $long_name:expr, $exts:expr, $mimes:expr, $markers:expr) => {
    Descriptor {
      short_name: $short_name,
      long_name: $long_name,
      extensions: $exts,
      mime_types: $mimes,
      markers: $markers,
      score: Self::score,
      inst: Instantiate::Format(|source, opt| Ok(Box::new(Self::try_new(source, &opt)?))),
    }
  };
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "every metadata reader in the family declares what it can read with this one definition, which builds its descriptor from its own `new` and `score`"
  )
)]
macro_rules! support_metadata {
  ($short_name:expr, $long_name:expr, $exts:expr, $mimes:expr, $markers:expr) => {
    Descriptor {
      short_name: $short_name,
      long_name: $long_name,
      extensions: $exts,
      mime_types: $mimes,
      markers: $markers,
      score: Self::score,
      inst: Instantiate::Metadata(|opt| {
        let Ok(reader) = Self::new(opt);

        Ok(Box::new(reader))
      }),
    }
  };
}
