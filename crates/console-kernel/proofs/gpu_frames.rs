//! Two tiles over four pixels. A trusted driver reports visibility only
//! after the device and memory dependency have both completed. The display
//! releases scanout ownership separately from GPU DMA retirement.

use super::super::dma::{Grant, GrantId, Released};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tile {
  Left,
  Right,
}

type Pixels = [u8; 2];
type Frame = [u8; 4];

#[derive(Clone, Copy)]
struct Draw {
  start: u8,
  end: u8,
  shade: u8,
  opaque: bool,
}

impl Draw {
  fn over(self, under: u8) -> u8 {
    if self.opaque {
      self.shade
    } else {
      (self.shade & under) + ((self.shade ^ under) >> 1)
    }
  }
}

fn serial(list: [Draw; 2], mut pixels: Frame) -> Frame {
  for draw in list {
    for (index, pixel) in (0_u8..4).zip(&mut pixels) {
      if (draw.start..draw.end).contains(&index) {
        *pixel = draw.over(*pixel);
      }
    }
  }
  pixels
}

fn tile(list: [Draw; 2], [a, b, c, d]: Frame, which: Tile) -> Pixels {
  let sample = |index, base| {
    list.into_iter().fold(base, |under, draw| {
      if draw.start <= index && index < draw.end {
        draw.over(under)
      } else {
        under
      }
    })
  };
  match which {
    Tile::Left => [sample(0, a), sample(1, b)],
    Tile::Right => [sample(2, c), sample(3, d)],
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameId {
  surface: u64,
  generation: u64,
}

#[derive(Clone, Copy)]
struct Device {
  number: u16,
  domain: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ticket {
  frame: FrameId,
  grant: GrantId,
}

#[derive(Clone, Copy)]
enum Visibility {
  Executed,
  Visible,
}

struct Completion {
  ticket: Ticket,
  tile: Tile,
  pixels: Pixels,
  visibility: Visibility,
}

#[derive(Debug, PartialEq, Eq)]
enum Use {
  Free,
  Rendering {
    ticket: Ticket,
    left: Option<Pixels>,
    right: Option<Pixels>,
  },
  Displayed {
    ticket: Ticket,
    dma_released: bool,
  },
  Retiring(Ticket),
  AwaitingScrub(Ticket),
}

struct Buffer {
  address: u64,
  epoch: u64,
  held: Use,
}

impl Buffer {
  fn new(address: u64) -> Self {
    Self {
      address,
      epoch: 0,
      held: Use::Free,
    }
  }

  fn start(&mut self, frame: FrameId, device: Device) -> Option<Ticket> {
    if self.held != Use::Free {
      return None;
    }
    let epoch = self.epoch.checked_add(1)?;
    let grant = GrantId {
      device: device.number,
      domain: device.domain,
      frame: self.address,
      epoch,
    };
    let ticket = Ticket { frame, grant };
    self.epoch = epoch;
    self.held = Use::Rendering {
      ticket,
      left: None,
      right: None,
    };
    Some(ticket)
  }

  fn complete(&mut self, done: Completion) {
    match (&mut self.held, done.visibility) {
      (
        Use::Rendering {
          ticket,
          left,
          right,
        },
        Visibility::Visible,
      ) if *ticket == done.ticket => {
        let slot = match done.tile {
          Tile::Left => left,
          Tile::Right => right,
        };
        if slot.is_none() {
          *slot = Some(done.pixels);
        }
      }
      _ => {}
    }
  }

  fn present(&mut self, wanted: Ticket) -> Option<Frame> {
    match self.held {
      Use::Rendering {
        ticket,
        left: Some([a, b]),
        right: Some([c, d]),
      } if ticket == wanted => {
        self.held = Use::Displayed {
          ticket,
          dma_released: false,
        };
        Some([a, b, c, d])
      }
      _ => None,
    }
  }

  fn cancel(&mut self, abandoned: Ticket) {
    if matches!(self.held, Use::Rendering { ticket, .. } if ticket == abandoned) {
      self.held = Use::Retiring(abandoned);
    }
  }

  fn scanout_released(&mut self, finished: Ticket) {
    if let Use::Displayed {
      ticket,
      dma_released,
    } = self.held
    {
      if ticket == finished {
        self.held = if dma_released {
          Use::AwaitingScrub(ticket)
        } else {
          Use::Retiring(ticket)
        };
      }
    }
  }

  fn dma_released(&mut self, receipt: Released) {
    match self.held {
      Use::Displayed { ticket, .. } if ticket.grant == receipt.grant() => {
        self.held = Use::Displayed {
          ticket,
          dma_released: true,
        };
      }
      Use::Retiring(ticket) if ticket.grant == receipt.grant() => {
        self.held = Use::AwaitingScrub(ticket);
      }
      _ => {}
    }
  }

  fn scrubbed(&mut self, cleared: Ticket) {
    if matches!(self.held, Use::AwaitingScrub(ticket) if ticket == cleared) {
      self.held = Use::Free;
    }
  }
}

fn draw() -> Draw {
  Draw {
    start: kani::any(),
    end: kani::any(),
    shade: kani::any(),
    opaque: kani::any(),
  }
}

fn finish(ticket: Ticket, tile: Tile, pixels: Pixels) -> Completion {
  Completion {
    ticket,
    tile,
    pixels,
    visibility: Visibility::Visible,
  }
}

fn orders() -> (Tile, Tile) {
  if kani::any() {
    (Tile::Left, Tile::Right)
  } else {
    (Tile::Right, Tile::Left)
  }
}

#[kani::proof]
#[kani::unwind(6)]
fn either_tile_completion_order_matches_serial_draw_order() {
  let list = [draw(), draw()];
  let base: Frame = kani::any();
  let mut buffer = Buffer::new(kani::any());
  let wanted = FrameId {
    surface: kani::any(),
    generation: kani::any(),
  };
  let ticket = buffer
    .start(
      wanted,
      Device {
        number: kani::any(),
        domain: kani::any(),
      },
    )
    .unwrap();
  let (first, last) = orders();
  buffer.complete(finish(ticket, first, tile(list, base, first)));
  assert!(buffer.present(ticket).is_none());

  let replacement: Pixels = kani::any();
  buffer.complete(finish(ticket, first, replacement));
  buffer.complete(finish(ticket, last, tile(list, base, last)));
  let [a, b, c, d] = serial(list, base);
  let [p, q, r, s] = buffer.present(ticket).unwrap();
  assert_eq!((p, q, r, s), (a, b, c, d));
  assert!(buffer.present(ticket).is_none());
}

#[kani::proof]
fn executed_or_stale_tiles_cannot_present_a_new_frame() {
  let surface = kani::any();
  let old_id = FrameId {
    surface,
    generation: 1,
  };
  let wanted = FrameId {
    surface,
    generation: 2,
  };
  let device = Device {
    number: kani::any(),
    domain: kani::any(),
  };
  let mut previous = Buffer::new(17);
  let mut next = Buffer::new(18);
  let old = previous.start(old_id, device).unwrap();
  let current = next.start(wanted, device).unwrap();
  let (first, last) = orders();
  let early: Pixels = kani::any();
  let late: Pixels = kani::any();
  let newer: Pixels = kani::any();
  previous.complete(finish(old, Tile::Left, early));
  next.complete(finish(current, first, early));

  let forged = Ticket {
    frame: wanted,
    grant: old.grant,
  };
  let mixed = if kani::any() { old } else { forged };
  next.complete(finish(mixed, last, late));
  next.complete(Completion {
    ticket: current,
    tile: last,
    pixels: late,
    visibility: Visibility::Executed,
  });
  previous.complete(finish(old, Tile::Right, late));
  assert!(previous.present(current).is_none());
  assert!(next.present(current).is_none());

  next.complete(finish(current, last, newer));
  let [a, b] = early;
  let [c, d] = newer;
  let [p, q, r, s] = next.present(current).unwrap();
  match first {
    Tile::Left => assert_eq!((p, q, r, s), (a, b, c, d)),
    Tile::Right => assert_eq!((p, q, r, s), (c, d, a, b)),
  }
}

fn dma_release(id: GrantId) -> Released {
  let mut grant = Grant::new(id, true);
  grant.stop();
  grant.drained(id);
  grant.invalidated(id);
  grant.release().unwrap()
}

#[kani::proof]
fn buffer_reuse_waits_for_scanout_dma_release_and_scrub() {
  let device = Device {
    number: kani::any(),
    domain: kani::any(),
  };
  let surface = kani::any();
  let old_id = FrameId {
    surface,
    generation: 1,
  };
  let wanted = FrameId {
    surface,
    generation: 2,
  };
  let mut buffer = Buffer::new(kani::any());
  let old = buffer.start(old_id, device).unwrap();
  let displayed = kani::any();
  let early_release = displayed && kani::any::<bool>();
  if displayed {
    buffer.complete(finish(old, Tile::Left, kani::any()));
    buffer.complete(finish(old, Tile::Right, kani::any()));
    assert!(buffer.present(old).is_some());
    buffer.cancel(old);
    if early_release {
      buffer.dma_released(dma_release(old.grant));
    }
    buffer.scrubbed(old);
    assert!(buffer.start(wanted, device).is_none());
    buffer.scanout_released(old);
  } else {
    buffer.cancel(old);
  }

  if !early_release {
    let unrelated = GrantId {
      epoch: 0,
      ..old.grant
    };
    buffer.dma_released(dma_release(unrelated));
    buffer.scrubbed(old);
    assert!(buffer.start(wanted, device).is_none());
    buffer.dma_released(dma_release(old.grant));
  }
  assert!(buffer.start(wanted, device).is_none());
  buffer.scrubbed(old);
  let fresh = buffer.start(wanted, device).unwrap();
  assert!(fresh.grant.epoch > old.grant.epoch);
  buffer.complete(finish(old, Tile::Left, kani::any()));
  buffer.complete(finish(old, Tile::Right, kani::any()));
  assert!(buffer.present(fresh).is_none());
}
