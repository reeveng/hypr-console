//! The first Segment, walked one level-1 element at a time.
//!
//! The walk reads Info, Tracks, the SeekHead, Cues, Chapters, Tags and
//! Attachments where they stand, and scans every Cluster's block headers into
//! the tracks' sample tables -- or, without an index asked for, stops at the
//! first Cluster. The SeekHead is followed twice besides: at the first Cluster,
//! for the Info and Tracks a cluster's blocks need, and after the walk, for
//! whatever a muxer put behind the clusters. An element is read once however
//! many routes reach it, by its position.
//!
//! A metadata element is read whole and handed to `meta`, unless it is larger
//! than `MAX_META`, which is a damaged size rather than a real one and is
//! skipped with a warning. Attachments are the exception: they are listed,
//! and their payload is located rather than read.

use std::collections::BTreeSet;

use console_core_byte_sources::ByteSource;
use console_core_extensible_binary_meta_language::{Size, string, uint};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::clusters::{add_blocks, numbers, scan_cluster};
use crate::demux::{MkvFile, OpenOptions};
use crate::ebml::{CrcMatch, verify_crc};
use crate::error::{Error, Warning};
use crate::ids::*;
use crate::meta::{
  Attachment, parse_chapters, parse_cues, parse_info, parse_seekhead, parse_tags, parse_tracks,
};
use crate::states::ElementBounds;
use crate::window::{Header, Span, Window};

const MAX_META: u64 = 256 << 20;

const SMALL_CHILD: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
  Headers,
  Everything,
  Late,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Follow {
  Read,
  Skip,
}

impl Pass {
  fn follows(self, id: u32) -> Result<Follow, Never> {
    Ok(match (self, id) {
      (Pass::Headers, INFO | TRACKS)
      | (Pass::Everything, INFO | TRACKS | CUES | CHAPTERS | TAGS | ATTACHMENTS | SEEK_HEAD)
      | (Pass::Late, CUES | CHAPTERS | TAGS | ATTACHMENTS) => Follow::Read,
      (Pass::Headers | Pass::Everything | Pass::Late, _) => Follow::Skip,
    })
  }
}

pub(crate) struct Walking<'w, 'a, S: ByteSource + ?Sized> {
  pub(crate) window: &'w mut Window<'a, S>,
  pub(crate) file: &'w mut MkvFile,
  pub(crate) options: &'w OpenOptions,
  pub(crate) parsed: BTreeSet<u64>,
  pub(crate) at: u64,
}

fn check_crc(file: &mut MkvFile, at: u64, data: &[u8]) -> Result<(), Never> {
  let Ok(verified) = verify_crc(data);

  match verified {
    Some(verdict) => {
      file.crc_checked = file.crc_checked.saturating_add(1);

      match verdict {
        CrcMatch::Matches => {}
        CrcMatch::Mismatch => {
          file.crc_errors.push(at);
          file.warnings.push(Warning::CrcMismatch { at });
        }
      }
    }
    None => {}
  }

  Ok(())
}

fn read_level1(file: &mut MkvFile, id: u32, data: &[u8]) -> Result<(), Never> {
  match id {
    SEEK_HEAD => {
      let Ok(entries) = parse_seekhead(data);

      file.seek_head.extend(entries);
    }
    INFO => {
      let Ok(info) = parse_info(data);

      file.info = info;
    }
    TRACKS => match ::core::primitive::bool::then_some(file.tracks.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(tracks) = parse_tracks(data, file.info.timestamp_scale);

        file.tracks = tracks;
      }
      ::core::option::Option::None => {}
    },
    CUES => {
      let Ok(()) = parse_cues(data, &mut file.cues);
    }
    CHAPTERS => {
      let Ok(()) = parse_chapters(data, &mut file.chapters);
    }
    TAGS => {
      let Ok(()) = parse_tags(data, &mut file.tags);
    }
    _ => {}
  }

  Ok(())
}

fn read_attached<S: ByteSource + ?Sized>(
  window: &mut Window<'_, S>,
  child: Header,
  mut attachment: Attachment,
  limit: u64,
) -> Result<Attachment, Error> {
  let Ok(body) = child.body(limit);
  let size = body.end.saturating_sub(body.start);

  match (child.id, size <= SMALL_CHILD) {
    (FILE_DATA, _) => {
      attachment.data_offset = body.start;
      attachment.data_size = size;
    }
    (_, false) => {}
    (_, true) => {
      let value = window.peek(body)?;
      let Ok(said) = string(value);
      let Ok(number) = uint(value);

      match child.id {
        FILE_NAME => attachment.name = said,
        FILE_MEDIA_TYPE => attachment.media_type = said,
        FILE_DESCRIPTION => attachment.description = Some(said),
        FILE_UID => attachment.uid = number,
        _ => {}
      }
    }
  }

  Ok(attachment)
}

impl<'w, 'a, S: ByteSource + ?Sized> Walking<'w, 'a, S> {
  pub(crate) fn step(mut self) -> Result<Step<Self, Self>, Error> {
    let segment_end = self.file.segment_end;

    match ::core::primitive::bool::then_some(self.at < segment_end, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(Step::Halt(self)),
    }

    let found = self.window.header(Span {
      start: self.at,
      end: segment_end,
    })?;
    let header = match found {
      Some(header) => header,
      None => return self.past_damage(),
    };

    match header.id {
      CLUSTER => self.cluster(&header),
      SEEK_HEAD | INFO | TRACKS | CUES | CHAPTERS | TAGS | ATTACHMENTS => {
        let end = self.level1(&header, segment_end)?;

        self.at = end;

        Ok(Step::Again(self))
      }
      EBML | SEGMENT => Ok(Step::Halt(self)),
      _ => self.unlisted(&header),
    }
  }

  fn past_damage(mut self) -> Result<Step<Self, Self>, Error> {
    let from = self.at;
    let found = self.window.resync(Span {
      start: from.saturating_add(1),
      end: self.file.segment_end,
    })?;

    Ok(match found {
      Some(to) => {
        self.file.warnings.push(Warning::Resynced { from, to });
        self.at = to;

        Step::Again(self)
      }
      None => Step::Halt(self),
    })
  }

  fn cluster(mut self, header: &Header) -> Result<Step<Self, Self>, Error> {
    let segment_end = self.file.segment_end;

    match self.file.first_cluster {
      Some(_) => {}
      None => {
        self.file.first_cluster = Some(header.at);
        self.follow_seek_head(Pass::Headers)?;

        match ::core::primitive::bool::then_some(self.options.index, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Ok(Step::Halt(self)),
        }
      }
    }

    let Ok(numbers) = numbers(&self.file.tracks);
    let (info, blocks) = scan_cluster(
      self.window,
      header,
      segment_end,
      &numbers,
      &mut self.file.warnings,
    )?;
    let Ok(data_start) = header.data_start();

    match (self.options.verify_crc, header.size) {
      (true, Size::Known(_)) => {
        let data = self.window.read_vec(Span {
          start: data_start,
          end: info.end,
        })?;
        let Ok(()) = check_crc(self.file, header.at, &data);
      }
      (true, Size::Unknown) | (false, _) => {}
    }

    let Ok(cluster) = fitted::<_, u32>(self.file.clusters.len());
    let Ok(()) = add_blocks(self.file, cluster, &blocks);

    self.file.clusters.push(info);
    self.at = info.end.max(data_start);

    Ok(Step::Again(self))
  }

  fn unlisted(mut self, header: &Header) -> Result<Step<Self, Self>, Error> {
    let segment_end = self.file.segment_end;
    let Ok(body) = header.body(segment_end);
    let Ok(bounds) = header.overruns(segment_end);

    match (header.size, bounds, header.id) {
      (Size::Unknown, _, _) => {
        let end = self.window.unknown_end(body)?;

        self.at = end;

        Ok(Step::Again(self))
      }
      (Size::Known(_), ElementBounds::Within, _)
      | (Size::Known(_), ElementBounds::Overrun, VOID) => {
        self.at = body.end;

        Ok(Step::Again(self))
      }
      (Size::Known(_), ElementBounds::Overrun, _) => {
        let found = self.window.resync(Span {
          start: header.at.saturating_add(1),
          end: segment_end,
        })?;

        Ok(match found {
          Some(to) => {
            self.file.warnings.push(Warning::OversizedElement {
              id: header.id,
              at: header.at,
              to,
            });
            self.at = to;

            Step::Again(self)
          }
          None => Step::Halt(self),
        })
      }
    }
  }

  pub(crate) fn follow_seek_head(&mut self, pass: Pass) -> Result<(), Error> {
    let segment_end = self.file.segment_end;
    let entries = self.file.seek_head.clone();

    for entry in entries {
      let Ok(followed) = pass.follows(entry.id);
      let at = match (
        followed,
        self.file.segment_data_start.checked_add(entry.position),
      ) {
        (Follow::Read, Some(at)) => at,
        (Follow::Read, None) | (Follow::Skip, _) => continue,
      };

      match ::core::primitive::bool::then_some(self.parsed.contains(&at) || at >= segment_end, ())
      {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => {}
      }

      let found = self.window.header(Span {
        start: at,
        end: segment_end,
      })?;

      match found.filter(|header| header.id == entry.id) {
        Some(header) => {
          let _read_to = self.level1(&header, segment_end)?;
        }
        None => self
          .file
          .warnings
          .push(Warning::MissedSeek { id: entry.id, at }),
      }
    }

    Ok(())
  }

  fn level1(&mut self, header: &Header, limit: u64) -> Result<u64, Error> {
    let Ok(start) = header.data_start();
    let end = match header.size {
      Size::Unknown => self.window.unknown_end(Span { start, end: limit })?,
      Size::Known(_) => {
        let Ok(end) = header.end(limit);

        end
      }
    };
    let Ok(bounds) = header.overruns(limit);

    match bounds {
      ElementBounds::Overrun => self.file.warnings.push(Warning::TruncatedElement {
        id: header.id,
        at: header.at,
      }),
      ElementBounds::Within => {}
    }

    match ::core::primitive::bool::then_some(self.parsed.insert(header.at), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(end),
    }

    let size = end.saturating_sub(start);

    match (header.id, size > MAX_META) {
      (ATTACHMENTS, _) => self.attachments(Span { start, end })?,
      (_, true) => self.file.warnings.push(Warning::TooLarge {
        id: header.id,
        at: header.at,
        size,
      }),
      (_, false) => {
        let data = self.window.read_vec(Span { start, end })?;

        match ::core::primitive::bool::then_some(self.options.verify_crc, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(()) = check_crc(self.file, header.at, &data);
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = read_level1(self.file, header.id, &data);
      }
    }

    Ok(end)
  }

  fn attachments(&mut self, span: Span) -> Result<(), Error> {
    let found = self.window.siblings(
      span,
      Vec::new(),
      |window, header, mut found: Vec<Attachment>| {
        match header.id {
          ATTACHED_FILE => {
            let Ok(body) = header.body(span.end);
            let attachment =
              window.siblings(body, Attachment::default(), |window_2, child, held| {
                read_attached(window_2, child, held, body.end)
              })?;

            found.push(attachment);
          }
          _ => {}
        }

        Ok(found)
      },
    )?;

    self.file.attachments.extend(found);

    Ok(())
  }
}
