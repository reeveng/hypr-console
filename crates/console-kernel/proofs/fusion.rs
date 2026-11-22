//! One-send model of I9. A trusted capability graph verifies a route before
//! dispatch. The ordinary and fused paths expose the same serialized events.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Route {
  sender: u64,
  receiver: u64,
  channel: u64,
  generation: u64,
}

struct Graph {
  live: Route,
  send: bool,
  receive: bool,
}

struct Certificate(Route);
struct VerifiedRoute<'a>(&'a Graph);

impl Graph {
  fn verify(&self, proposed: Certificate) -> Option<VerifiedRoute<'_>> {
    (self.send && self.receive && self.live == proposed.0).then_some(VerifiedRoute(self))
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Request {
  payload: u64,
  charge: u32,
}

#[derive(Clone, Copy)]
struct Admission {
  sender_room: u32,
  peer_window: u32,
}

#[derive(Clone, Copy)]
enum Event {
  Receive { room: bool },
  Cancel,
  PeerGone,
}

#[derive(Debug, PartialEq, Eq)]
enum Effect {
  None,
  Delivered { receiver: u64, request: Request },
  Withdrawn { sender: u64, request: Request },
}

#[derive(Debug, PartialEq, Eq)]
struct Accounting {
  sender_held: u32,
  receiver_held: u32,
}

struct Buffered {
  route: Route,
  queued: Option<Request>,
  accounting: Accounting,
}

impl Buffered {
  fn admit(graph: &Graph, request: Request, room: Admission) -> Option<Self> {
    if !graph.send
      || !graph.receive
      || request.charge > room.sender_room
      || request.charge > room.peer_window
    {
      return None;
    }
    Some(Self {
      route: graph.live,
      queued: Some(request),
      accounting: Accounting {
        sender_held: request.charge,
        receiver_held: 0,
      },
    })
  }

  fn dispatch(&mut self, event: Event) -> Effect {
    let Some(held) = self.queued else {
      return Effect::None;
    };
    let effect = match event {
      Event::Receive { room: false } => return Effect::None,
      Event::Receive { room: true } => {
        self.accounting.receiver_held = held.charge;
        Effect::Delivered {
          receiver: self.route.receiver,
          request: held,
        }
      }
      Event::Cancel | Event::PeerGone => Effect::Withdrawn {
        sender: self.route.sender,
        request: held,
      },
    };
    self.accounting.sender_held = 0;
    self.queued = None;
    effect
  }
}

enum Direct<'a> {
  Pending(VerifiedRoute<'a>, Request),
  Delivered(u32),
  Withdrawn,
}

impl<'a> Direct<'a> {
  fn admit(route: VerifiedRoute<'a>, request: Request, room: Admission) -> Option<Self> {
    (request.charge <= room.sender_room.min(room.peer_window))
      .then_some(Self::Pending(route, request))
  }

  fn accounting(&self) -> Accounting {
    match self {
      Self::Pending(_, request) => Accounting {
        sender_held: request.charge,
        receiver_held: 0,
      },
      Self::Delivered(charge) => Accounting {
        sender_held: 0,
        receiver_held: *charge,
      },
      Self::Withdrawn => Accounting {
        sender_held: 0,
        receiver_held: 0,
      },
    }
  }

  fn dispatch(&mut self, event: Event) -> Effect {
    match (self, event) {
      (state @ Self::Pending(_, _), Event::Cancel | Event::PeerGone) => {
        let previous = core::mem::replace(state, Self::Withdrawn);
        let Self::Pending(route, request) = previous else {
          unreachable!()
        };
        Effect::Withdrawn {
          sender: route.0.live.sender,
          request,
        }
      }
      (state @ Self::Pending(_, _), Event::Receive { room: true }) => {
        let Self::Pending(route, held) = &*state else {
          unreachable!()
        };
        let request = *held;
        let receiver = route.0.live.receiver;
        *state = Self::Delivered(request.charge);
        Effect::Delivered { receiver, request }
      }
      (Self::Pending(_, _), Event::Receive { room: false })
      | (Self::Delivered(_) | Self::Withdrawn, _) => Effect::None,
    }
  }

  fn peer(&self) -> Option<u64> {
    match self {
      Self::Pending(route, _) => Some(route.0.live.receiver),
      Self::Delivered(_) | Self::Withdrawn => None,
    }
  }
}

fn route() -> Route {
  Route {
    sender: kani::any(),
    receiver: kani::any(),
    channel: kani::any(),
    generation: kani::any(),
  }
}

fn event() -> Event {
  match (kani::any::<bool>(), kani::any::<bool>()) {
    (false, room) => Event::Receive { room },
    (true, false) => Event::Cancel,
    (true, true) => Event::PeerGone,
  }
}

#[kani::proof]
fn a_fusion_certificate_authorizes_only_the_exact_live_route() {
  let live = route();
  let claimed = route();
  let graph = Graph {
    live,
    send: kani::any(),
    receive: kani::any(),
  };
  let matches_route = live.sender == claimed.sender
    && live.receiver == claimed.receiver
    && live.channel == claimed.channel
    && live.generation == claimed.generation;
  assert_eq!(
    graph.verify(Certificate(claimed)).is_some(),
    matches_route && graph.send && graph.receive
  );
  kani::cover!(matches_route && graph.send && graph.receive);
}

#[kani::proof]
#[kani::unwind(6)]
fn fused_delivery_preserves_effects_cancellation_and_accounting() {
  let graph = Graph {
    live: route(),
    send: kani::any(),
    receive: kani::any(),
  };
  let request = Request {
    payload: kani::any(),
    charge: kani::any(),
  };
  let room = Admission {
    sender_room: kani::any(),
    peer_window: kani::any(),
  };
  let ordinary = Buffered::admit(&graph, request, room);
  let optimized = graph
    .verify(Certificate(graph.live))
    .and_then(|authority| Direct::admit(authority, request, room));
  assert_eq!(ordinary.is_some(), optimized.is_some());
  let (Some(mut buffered), Some(mut direct)) = (ordinary, optimized) else {
    return;
  };
  assert_eq!(direct.peer(), Some(graph.live.receiver));
  assert_eq!(buffered.accounting, direct.accounting());

  for _ in 0..5 {
    let event = event();
    assert_eq!(buffered.dispatch(event), direct.dispatch(event));
    assert_eq!(buffered.accounting, direct.accounting());
    assert_eq!(direct.peer().is_some(), buffered.queued.is_some());
  }
}
