//! One logical stream: the packets its pages carry, with the timestamps and trims its first and
//! last pages decide.
//!
//! A packet that a page does not finish is held until the page that continues it, and dropped
//! when the pages say there was a gap, so a stream never hands back half of one glued to the
//! start of another. Timestamps are given to the packets of a page from its end backwards: the
//! granule position is where the page ends, and each packet starts where the one after it
//! stops.

use std::cmp::Ordering;
use std::collections::VecDeque;

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_audio_decoding_interfaces::formats::util::trim_packet;
use console_core_audio_decoding_interfaces::formats::{GaplessPlayback, Packet};
use console_core_audio_decoding_interfaces::meta::MetadataRevision;
use console_core_number_conversion::{fitted, index};

use crate::mappings::{MapResult, Mapper, Parser};
use crate::page::{PacketStart, Page, PageHeader, StreamEnd};
use crate::stepping::Certain;

const MAX_PACKET_LEN: u64 = 16 * 1024 * 1024;

#[derive(Copy, Clone)]
struct Bound {
  ts: u64,
  delay: u64,
}

#[derive(Default)]
pub struct InspectState {
  bound: Option<Bound>,
  parser: Option<Parser>,
}

pub struct LogicalStream {
  mapper: Mapper,
  packets: VecDeque<Packet>,
  part_buf: Vec<u8>,
  prev_sequence: Option<u32>,
  start_bound: Option<Bound>,
  end_bound: Option<Bound>,
  gapless: GaplessPlayback,
}

fn page_duration(parser: &mut Parser, page: &Page<'_>) -> Certain<u64> {
  let Ok(packets) = page.packets();
  let mut duration = 0u64;

  for packet in packets {
    let Ok(playing) = parser.parse_next_packet_dur(packet);

    duration = duration.saturating_add(playing);
  }

  Ok(duration)
}

impl LogicalStream {
  pub fn new(mapper: Mapper, gapless: GaplessPlayback) -> Certain<Self> {
    Ok(LogicalStream {
      mapper,
      packets: VecDeque::new(),
      part_buf: Vec::new(),
      prev_sequence: None,
      start_bound: None,
      end_bound: None,
      gapless,
    })
  }

  pub fn reset(&mut self) -> Certain<()> {
    self.part_buf.clear();
    self.prev_sequence = None;
    self.packets.clear();

    self.mapper.reset()
  }

  pub fn codec_params(&self) -> Certain<&CodecParameters> {
    self.mapper.codec_params()
  }

  pub fn read_page(&mut self, page: &Page<'_>) -> Result<Vec<MetadataRevision>> {
    let mut side_data = Vec::new();
    let Ok(()) = self.check_continuity(page.header.sequence);

    self.prev_sequence = Some(page.header.sequence);

    let Ok(mut iter) = page.packets();

    match (page.header.packet_start, self.part_buf.as_slice()) {
      (PacketStart::Fresh, [_, ..]) => self.part_buf.clear(),
      (PacketStart::Continued, []) => match iter.next() {
        Some(_incomplete) => {}
        None => return Ok(side_data),
      },
      (PacketStart::Fresh, []) | (PacketStart::Continued, [_, ..]) => {}
    }

    let Ok(before) = fitted::<_, u32>(self.packets.len());

    for buf in &mut iter {
      let Ok(data) = self.joined_packet(buf);

      match self.mapper.map_packet(&data) {
        Ok(MapResult::Audio { dur }) => {
          let Ok(value_1) = Packet::new_from_boxed_slice(page.header.serial, 0, dur, data);

          self.packets.push_back(value_1)
        }
        Ok(MapResult::Metadata(revision)) => side_data.push(revision),
        Ok(MapResult::Skipped) => {}
        Err(_unmapped) => continue,
      }
    }

    let Ok(partial) = iter.partial_packet();

    match partial {
      Some(buf) => {
        self.save_partial_packet(buf)?;
      }
      None => {}
    }

    let Ok(after) = fitted::<_, u32>(self.packets.len());
    let Ok(()) = self.assign_timestamps(&page.header, after.saturating_sub(before));

    Ok(side_data)
  }

  pub fn peek_packet(&self) -> Certain<Option<&Packet>> {
    Ok(self.packets.front())
  }

  pub fn next_packet(&mut self) -> Certain<Option<Packet>> {
    Ok(self.packets.pop_front())
  }

  pub fn consume_packet(&mut self) -> Certain<()> {
    let _consumed = self.packets.pop_front();

    Ok(())
  }

  pub fn inspect_start_page(&mut self, page: &Page<'_>) -> Certain<()> {
    match self.start_bound {
      Some(_found) => return Ok(()),
      None => {}
    }

    let Ok(made) = self.mapper.make_parser();

    let mut parser = match made {
      Some(parser) => parser,
      None => return Ok(()),
    };

    let Ok(page_dur) = page_duration(&mut parser, page);
    let page_end_ts = page.header.absgp;

    let bound = match page_end_ts.checked_sub(page_dur) {
      Some(ts) => Bound { ts, delay: 0 },
      None => Bound {
        ts: 0,
        delay: page_dur.saturating_sub(page_end_ts),
      },
    };

    let Ok(codec_params) = self.mapper.codec_params_mut();

    let Ok(_configured_with_start_ts) = codec_params.with_start_ts(bound.ts);

    match bound.delay {
      0 => {}
      delay => {
        let Ok(frames) = fitted::<u64, u32>(delay);

        let Ok(_configured_with_delay) = codec_params.with_delay(frames);
      }
    }

    self.start_bound = Some(bound);

    Ok(())
  }

  pub fn inspect_end_page(
    &mut self,
    mut state: InspectState,
    page: &Page<'_>,
  ) -> Certain<InspectState> {
    match self.end_bound {
      Some(_found) => return Ok(state),
      None => {}
    }

    let mut parser = match state.parser.take() {
      Some(parser) => parser,
      None => {
        let Ok(made) = self.mapper.make_parser();

        match made {
          Some(parser) => parser,
          None => return Ok(state),
        }
      }
    };

    let Ok(start_delay) = self.start_delay();
    let Ok(waited) = self.unless_gapless(start_delay);
    let page_end_ts = page.header.absgp.saturating_add(waited);
    let Ok(page_dur) = page_duration(&mut parser, page);

    let end_delay = match (page.header.stream_end, state.bound) {
      (StreamEnd::Final, Some(last)) => {
        last.ts.saturating_add(page_dur).saturating_sub(page_end_ts)
      }
      (StreamEnd::Final, None) | (StreamEnd::MorePages, _) => 0,
    };

    let bound = Bound {
      ts: page_end_ts,
      delay: end_delay,
    };

    match page.header.stream_end {
      StreamEnd::Final => {
        let Ok(()) = self.finish_end(bound);
      }
      StreamEnd::MorePages => {}
    }

    state.bound = Some(bound);
    state.parser = Some(parser);

    Ok(state)
  }

  pub fn inspect_page(&self, page: &Page<'_>) -> Certain<(u64, u64)> {
    let Ok(made) = self.mapper.make_parser();

    let page_dur = match made {
      Some(mut parser) => {
        let Ok(duration) = page_duration(&mut parser, page);

        duration
      }
      None => 0,
    };

    let end_delay = match page.header.stream_end {
      StreamEnd::Final => {
        let Ok(delay) = self.end_delay();

        delay
      }
      StreamEnd::MorePages => 0,
    };

    let Ok(start_delay) = self.start_delay();
    let delay = start_delay.saturating_add(end_delay);
    let page_end_ts = page.header.absgp.saturating_add(delay);
    let page_start_ts = page_end_ts.saturating_sub(page_dur);

    Ok(match self.gapless {
      GaplessPlayback::Disabled => (page_start_ts, page_end_ts),
      GaplessPlayback::Enabled => (
        page_start_ts.saturating_sub(delay),
        page_end_ts.saturating_sub(delay),
      ),
    })
  }

  fn start_delay(&self) -> Certain<u64> {
    Ok(match self.start_bound {
      Some(bound) => bound.delay,
      None => 0,
    })
  }

  fn end_delay(&self) -> Certain<u64> {
    Ok(match self.end_bound {
      Some(bound) => bound.delay,
      None => 0,
    })
  }

  fn unless_gapless(&self, delay: u64) -> Certain<u64> {
    Ok(match self.gapless {
      GaplessPlayback::Enabled => 0,
      GaplessPlayback::Disabled => delay,
    })
  }

  fn check_continuity(&mut self, sequence: u32) -> Certain<()> {
    match self.prev_sequence {
      Some(last) => match sequence.checked_sub(last) {
        None => self.part_buf.clear(),
        Some(0 | 1) => {}
        Some(_gap) => self.part_buf.clear(),
      },
      None => {}
    }

    Ok(())
  }

  fn assign_timestamps(&mut self, header: &PageHeader, count: u32) -> Certain<()> {
    let Ok(newest) = index(count);
    let Ok(start_delay) = self.start_delay();

    let ending = match header.stream_end {
      StreamEnd::Final => {
        let Ok(delay) = self.end_delay();

        delay
      }
      StreamEnd::MorePages => 0,
    };

    let page_end_ts = header
      .absgp
      .saturating_add(start_delay)
      .saturating_add(ending);
    let mut page_dur = 0u64;

    for packet in self.packets.iter_mut().rev().take(newest) {
      page_dur = page_dur.saturating_add(packet.dur);
      packet.ts = page_end_ts.saturating_sub(page_dur);
    }

    match self.gapless {
      GaplessPlayback::Enabled => {
        let Ok(delay) = fitted::<u64, u32>(start_delay);
        let end_ts = self.end_bound.map(|bound| bound.ts);

        for packet in self.packets.iter_mut().rev().take(newest) {
          let Ok(()) = trim_packet(packet, delay, end_ts);
        }
      }
      GaplessPlayback::Disabled => {}
    }

    Ok(())
  }

  fn finish_end(&mut self, bound: Bound) -> Certain<()> {
    let Ok(waited) = self.unless_gapless(bound.delay);
    let block_end_ts = bound.ts.saturating_add(waited);
    let Ok(codec_params) = self.mapper.codec_params_mut();

    match block_end_ts.checked_sub(codec_params.start_ts) {
      Some(0) | None => {}
      Some(frames) => {
        let Ok(_configured_with_n_frames) = codec_params.with_n_frames(frames);
      }
    }

    match bound.delay {
      0 => {}
      delay => {
        let Ok(frames) = fitted::<u64, u32>(delay);

        let Ok(_configured_with_padding) = codec_params.with_padding(frames);
      }
    }

    self.end_bound = Some(bound);

    Ok(())
  }

  fn joined_packet(&mut self, packet_buf: &[u8]) -> Certain<Box<[u8]>> {
    Ok(match self.part_buf.as_slice() {
      [] => Box::from(packet_buf),
      [_, ..] => {
        self.part_buf.extend_from_slice(packet_buf);

        let joined = Box::from(self.part_buf.as_slice());

        self.part_buf.clear();

        joined
      }
    })
  }

  fn save_partial_packet(&mut self, buf: &[u8]) -> Result<()> {
    let Ok(kept) = fitted::<_, u64>(self.part_buf.len());
    let Ok(arriving) = fitted::<_, u64>(buf.len());
    let held = kept.saturating_add(arriving);

    match held.cmp(&MAX_PACKET_LEN) {
      Ordering::Greater => decode_error("ogg: packet buffer would exceed max size"),
      Ordering::Less | Ordering::Equal => {
        self.part_buf.extend_from_slice(buf);

        Ok(())
      }
    }
  }
}
