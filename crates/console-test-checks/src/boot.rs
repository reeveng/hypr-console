//! What a boot leaves behind for the person holding the machine and for the
//! boot after it.
//!
//! Both are read after the fact because a boot cannot be pressed from here:
//! the run would have to restart somebody's machine to watch one. What can be
//! asked is the state a boot settles into, and both faults these stand guard
//! over are states that last -- a pad InputPlumber claimed half of stays half
//! claimed until something restarts it, and a boot that forgot to say it came
//! up leaves the next one sitting at the menu.

use console_test_stages::checking::{Body, Check, CheckResult, cannot, empty, same};
use console_test_stages::device::Device;

pub const PAD: Check = Check {
  name: "490-the-pad-is-claimed-whole",
  about: "Every gamepad node the controller has is one InputPlumber holds.",
  feature: "boot",
  since: "2026-09-26",
  bodies: &[Body::Device(pad)],
};

fn pad(stage: &mut Device) -> CheckResult {
  let said = stage.inspect(&["claimed-gamepad"])?;

  let nodes: Vec<&str> = said
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .collect();

  match ::core::primitive::bool::then_some(nodes.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return cannot("no controller is attached, so there is no pad to have claimed");
    }
    ::core::option::Option::None => {}
  }

  let unclaimed: Vec<&str> = nodes
    .iter()
    .copied()
    .filter(|line| line.ends_with(" unclaimed"))
    .collect();

  empty(&unclaimed, || {
    format!(
      "InputPlumber came up holding only part of the pad, so these buttons reach \
             nothing: {unclaimed:?}"
    )
  })
}

pub const MENU: Check = Check {
  name: "491-a-boot-that-came-up-skips-the-next-menu",
  about: "A boot of the ordinary root has told Limine the next boot needs no menu.",
  feature: "boot",
  since: "2026-09-26",
  bodies: &[Body::Device(menu)],
};

const ORDINARY: &str = "/@";

const SKIPPED: &str = "menu-disabled";

fn menu(stage: &mut Device) -> CheckResult {
  let said = stage.inspect(&["boot-menu"])?;

  let (root, left) = match said.split_once('\n') {
    Some((root, left)) => (root, left.trim()),
    None => (said.trim(), ""),
  };

  match ::core::primitive::bool::then_some(root == ORDINARY, ()) {
    ::core::option::Option::Some(()) => same(left, SKIPPED, || {
      format!(
        "this boot came up and did not say so, so the next one waits ten seconds at \
                 the boot menu: the one-shot timeout reads {left:?}"
      )
    }),
    ::core::option::Option::None => same(left, "", || {
      format!(
        "this is a snapshot ({root}) and the next boot would skip the menu that is the \
                 only way back out of it: the one-shot timeout reads {left:?}"
      )
    }),
  }
}
