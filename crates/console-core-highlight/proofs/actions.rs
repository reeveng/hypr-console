//! `cargo kani -p console-core-highlight --lib --harness actions`.

use super::*;
use console_core_geometry::Size;

fn point() -> Point<i32> {
  Point {
    x: kani::any(),
    y: kani::any(),
  }
}

fn frame() -> Rectangle<i32> {
  Rectangle {
    origin: point(),
    size: Size {
      width: kani::any(),
      height: kani::any(),
    },
  }
}

fn resolve(actions: &Actions<'_, u8>, press: Press, focused: Option<u32>) -> Resolution<u8> {
  let Ok(resolution) = actions.resolve(press, focused);

  resolution
}

#[kani::proof]
#[kani::unwind(6)]
fn a_hit_and_a_focused_selection_activate_the_same_registered_action() {
  let frames = [frame(), frame(), frame()];
  let values: [u8; 3] = kani::any();
  let actions = Actions::new(&frames, &values).unwrap();
  let at = point();
  let focused = Some(kani::any());
  let Ok(tapped) = actions.resolve(Press::Tap(at), focused);
  let Ok(hovered) = actions.resolve(Press::Hover(at), focused);

  match tapped {
    Resolution::Activate { target, action } => {
      assert!(target < 3);
      assert_eq!(action, values[target as usize]);
      assert_eq!(hovered, Resolution::Focus(target));
      assert_eq!(resolve(&actions, Press::Select, Some(target)), tapped);
    }
    Resolution::Pass => assert_eq!(hovered, Resolution::Pass),
    Resolution::Focus(_) => unreachable!(),
  }
  assert_eq!(resolve(&actions, Press::Select, None), Resolution::Pass);
  assert_eq!(
    resolve(&actions, Press::Drag(at), focused),
    Resolution::Pass
  );
  kani::cover!(matches!(tapped, Resolution::Activate { target: 0, .. }));
  kani::cover!(matches!(tapped, Resolution::Activate { target: 2, .. }));
}

#[kani::proof]
#[kani::unwind(6)]
fn every_action_in_a_visible_row_has_directional_tab_and_tap_paths() {
  let width = i32::from(kani::any::<u8>()).saturating_add(1);
  let gap = i32::from(kani::any::<u8>());
  let origin = Point {
    x: i32::from(kani::any::<i16>()),
    y: i32::from(kani::any::<i16>()),
  };
  let frames = [0, 1, 2].map(|position| Rectangle {
    origin: Point {
      x: origin.x + position * (width + gap),
      y: origin.y,
    },
    size: Size { width, height: 1 },
  });
  let values: [u8; 3] = kani::any();
  let actions = Actions::new(&frames, &values).unwrap();

  for target in 0..3 {
    let at = target as usize;
    match target {
      0 => {}
      1.. => {
        for heading in [Heading::Right, Heading::Next] {
          assert_eq!(
            resolve(&actions, Press::Toward(heading), Some(target - 1)),
            Resolution::Focus(target),
          );
        }
      }
    }
    let activation = Resolution::Activate {
      target,
      action: values[at],
    };
    assert_eq!(resolve(&actions, Press::Select, Some(target)), activation);
    assert_eq!(
      resolve(&actions, Press::Tap(frames[at].origin), None),
      activation
    );
  }
}
