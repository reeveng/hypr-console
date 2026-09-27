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

const CLAIMED: &str = r#"claimed=$(for d in $(busctl --list tree org.shadowblip.InputPlumber | grep '/CompositeDevice[0-9]*$'); do
  busctl get-property org.shadowblip.InputPlumber "$d" org.shadowblip.Input.CompositeDevice SourceDevicePaths
done)
awk '/^N: Name=/ { pad = ($0 ~ /^N: Name="Legion-Controller [^ "]*"$/) }
     pad && /^H: / { for (i = 2; i <= NF; i++) { f = $i; sub(/^Handlers=/, "", f); if (f ~ /^event/) print f } }' \
  /proc/bus/input/devices |
while read -r node; do
  case "$claimed" in
    *"/dev/input/$node\""*) echo "$node claimed" ;;
    *) echo "$node unclaimed" ;;
  esac
done"#;

fn pad(stage: &mut Device) -> CheckResult {
    let Ok(said) = stage.ssh(CLAIMED);

    let nodes: Vec<&str> = said.lines().map(str::trim).filter(|line| !line.is_empty()).collect();

    match nodes.is_empty() {
        true => return cannot("no controller is attached, so there is no pad to have claimed"),
        false => {},
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

const LEFT: &str = r#"left=/sys/firmware/efi/efivars/LoaderConfigTimeoutOneShot-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f
echo "$(findmnt -no FSROOT /) $(test -e "$left" && tail -c +5 "$left" | tr -d '\0')""#;

const ORDINARY: &str = "/@";

const SKIPPED: &str = "menu-disabled";

fn menu(stage: &mut Device) -> CheckResult {
    let Ok(said) = stage.ssh(LEFT);

    let (root, left) = match said.split_once(' ') {
        Some((root, left)) => (root, left.trim()),
        None => (said.trim(), ""),
    };

    match root == ORDINARY {
        true => same(left, SKIPPED, || {
            format!(
                "this boot came up and did not say so, so the next one waits ten seconds at \
                 the boot menu: the one-shot timeout reads {left:?}"
            )
        }),
        false => same(left, "", || {
            format!(
                "this is a snapshot ({root}) and the next boot would skip the menu that is the \
                 only way back out of it: the one-shot timeout reads {left:?}"
            )
        }),
    }
}
