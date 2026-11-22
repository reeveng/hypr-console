//! A window dragged in the overview lands where the overview lit.
//!
//! Where a dropped window goes is decided by the side of the window under the
//! finger that the finger is nearest, and the arithmetic is the overview's own
//! and is asked of it here rather than written again: the same windows, the
//! same room, the same cards. What is pressed is a finger -- a swipe up from
//! the bottom edge, and a drag from one card to the lower edge of the window in
//! another -- and what is asked afterwards is the compositor's, which is where
//! the window really is. A drop that joined the place beside the window rather
//! than under it is the fault this was written against, and it reads as a
//! window whose top is above the other's bottom.

use console_compositor::Window;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_overview::{Card, Frame, Tile, overview, places};
use console_test_stages::checking::{Body, Check, CheckResult, Why, cannot, happened};
use console_test_stages::device::{Device, OPENING, PATIENCE};
use console_waiting::Ready;

pub const UNDER: Check = Check {
  name: "500-a-window-dropped-low-goes-underneath",
  about: "Dragged in the overview onto the lower edge of another window, a window stands under it.",
  feature: "overview",
  since: "2026-09-28",
  bodies: &[Body::Device(under_there)],
};

const UPPER: &str = "stays-on-top";

const LOWER: &str = "dropped-underneath";

const LOW: f64 = 0.9;

const SWIPED: u32 = 400;

const TOUCHING: i64 = 8;

fn opened(stage: &mut Device, title: &str) -> Result<String, Why> {
  let Ok(came) = stage.opening(
    &format!("alacritty --title {title} -o window.dynamic_title=false"),
    OPENING,
  );

  came.ok_or_else(|| Why::Cannot(format!("no window called {title} would open on the device")))
}

fn middle(frame: Frame) -> Result<(u32, u32), Never> {
  let Ok(across) = whole_u32(frame.at.x + frame.size.width / 2.0);
  let Ok(down) = whole_u32(frame.at.y + frame.size.height / 2.0);

  Ok((across, down))
}

fn tile_of<'a>(cards: &'a [Card], address: &str) -> Result<Option<&'a Tile>, Never> {
  Ok(
    cards
      .iter()
      .flat_map(|card| card.tiles.iter())
      .find(|tile| tile.address == address),
  )
}

fn window_of<'a>(open: &'a [Window], address: &str) -> Result<Option<&'a Window>, Never> {
  Ok(open.iter().find(|window| window.address == address))
}

fn shown(stage: &mut Device) -> Result<Option<Size<u32>>, Why> {
  let Ok(edge) = stage.layer(console_onscreen::OVERVIEW_EDGE);

  let (left, top, wide, tall) = match edge {
    Some(edge_2) => edge_2,
    None => return Ok(None),
  };

  let across = left.saturating_add(wide.saturating_div(2));
  let from = top.saturating_add(tall.saturating_div(2));
  stage.swipe((across, from), (across, from.saturating_sub(SWIPED)))?;

  let Ok(_) = stage.until::<Never>(
    |seen| {
      let Ok(up) = seen.layer(console_onscreen::OVERVIEW);

      Ok(match up {
        Some(_) => Ready::Yes,
        None => Ready::NotYet,
      })
    },
    PATIENCE,
  );
  let Ok(up) = stage.layer(console_onscreen::OVERVIEW);

  Ok(up.map(|(_, _, wide_2, tall_2)| Size {
    width: wide_2,
    height: tall_2,
  }))
}

fn under_there(stage: &mut Device) -> CheckResult {
  let Ok(()) = stage.fresh();
  let upper = opened(stage, UPPER)?;
  let lower = opened(stage, LOWER)?;
  let room = shown(stage)?;

  let room_3 = match room {
    Some(room_2) => room_2,
    None => return cannot("a swipe up from the bottom edge brought no overview to drag in"),
  };

  let Ok(open) = stage.windows_open();
  let Ok(grouped) = places(open);
  let Ok(laid) = overview(&grouped, room_3);
  let Ok(held) = tile_of(&laid.cards, &lower);
  let Ok(onto) = tile_of(&laid.cards, &upper);

  let (held_3, onto_3) = match (held, onto) {
    (Some(held_2), Some(onto_2)) => (held_2.frame, onto_2.frame),
    (None, _) | (_, None) => {
      return cannot("the overview has no card for one of the two windows it was shown");
    }
  };

  let Ok(from) = middle(held_3);
  let Ok(across) = whole_u32(onto_3.at.x + onto_3.size.width / 2.0);
  let Ok(down) = whole_u32(onto_3.at.y + onto_3.size.height * LOW);

  stage.swipe(from, (across, down))?;

  let Ok(joined) = stage.until::<Never>(
    |seen| {
      let Ok(open_2) = seen.windows_open();
      let Ok(top) = window_of(&open_2, &upper);
      let Ok(bottom) = window_of(&open_2, &lower);

      Ok(match (top, bottom) {
        (Some(top_2), Some(bottom_2)) => match ::core::primitive::bool::then_some(top_2.workspace == bottom_2.workspace, ()) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
        (None, _) | (_, None) => Ready::NotYet,
      })
    },
    PATIENCE,
  );

  let Ok(()) = stage.fresh();

  happened(joined, || {
    "the window dragged onto the other card did not join its workspace".to_string()
  })?;

  let Ok(open_3) = stage.windows_open();
  let Ok(top) = window_of(&open_3, &upper);
  let Ok(bottom) = window_of(&open_3, &lower);

  let (top_4, bottom_4) = match (top, bottom) {
    (Some(top_3), Some(bottom_3)) => (top_3, bottom_3),
    (None, _) | (_, None) => {
      return Err(Why::Failed("one of the two windows went away".to_string()));
    }
  };

  let under = bottom_4.at.1.saturating_add(TOUCHING) >= top_4.at.1.saturating_add(top_4.size.1);

  match ::core::primitive::bool::then_some(under, ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Why::Failed(format!(
      "dropped on the lower edge of {UPPER}, {LOWER} stands at {:?} sized {:?} beside it at {:?} sized {:?}",
      bottom_4.at, bottom_4.size, top_4.at, top_4.size
    ))),
  }
}
