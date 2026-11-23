# Legion Go releases

The first-generation Legion Go (83E1) can install prebuilt releases from
`reeveng/hypr-console`. A release includes the programs named by `desktop.conf`
and the Legion Go section of `machines.conf`.

## Publish a release

Export the intended commit into a checkout of the public repository with
`console-manifest-publish --export-only`. This checks the exported tree for
private machine and account names. Commit the export to the public repository
and push a new tag such as `legion-go-v0.1.0` on that commit. GitHub Actions
builds the bundle and publishes the immutable release after the build succeeds.

The build uses an Arch Linux container and the `x86-64-v3` CPU target. The
archive contains the built binaries and their source commit. GitHub publishes
the archive's SHA-256 digest with the immutable release metadata.

## Set up update checks

Install `console-update`, its launcher entry, and the check service and timer
from a verified release for the initial switch. Enable the timer to check
hourly and show a notification for each new stable tag.

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now console-update.timer
```

Open **Console Updates** from the launcher and choose **Install** when ready.
An administrator can also install a specific tag with
`sudo console-update legion-go-v0.1.1`.

On a Legion Go with a login pattern, an administrator can enable pattern
approval with `sudo console-update --enroll-pattern`. Console Updates then
opens its own dialog with the available tag and accepts the saved login pattern
by touch or gamepad. Use the close button to dismiss the dialog.
The updater keeps a root-owned copy of the hash. Re-enroll after changing the
login pattern. Use `sudo console-update --forget-pattern` to return to password
approval.

The updater checks the archive against GitHub's asset digest and compares its
source commit with the fetched tag. `console apply` takes the existing Snapper
snapshots before exchanging the new checkout with `/etc/console`, then applies
the bundled binaries and manifest. The updater runs the desktop health check
before recording success.

To return to the previous release, run `sudo console-update --rollback`. The
updater preserves that checkout and its binaries, and applies them as a new
generation. Background checks hold the rolled-back tag until a newer stable tag
appears. The previous snapshots are available through the normal boot menu.
