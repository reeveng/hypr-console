# Console releases

The first-generation Legion Go (83E1) can install prebuilt releases from
`reeveng/hypr-console`. A release includes the programs named by `desktop.conf`
and the Legion Go section of `machines.conf`.

## Publish a release

Export the intended commit into a checkout of the public repository with
`console-manifest-publish --export-only`. This checks the exported tree for
private machine and account names. Commit the export to the public repository
and push a new tag such as `console-v0.1.8` on that commit. GitHub Actions
builds the bundle and publishes the immutable release after the build succeeds.

The build uses an Arch Linux container and the `x86-64-v3` CPU target. The
`console-x86_64.tar.gz` archive contains the built binaries and their source
commit. GitHub publishes its SHA-256 digest with the immutable release metadata.

For the one-time migration, push `console-v0.1.7` and `legion-go-v0.1.7`
together on the same commit. GitHub Actions publishes the same bundle under
both tags. It names the legacy copy `legion-go-x86_64.tar.gz` and marks that
release as latest so older updaters can find it. Later releases use `console-v`
tags and `console-x86_64.tar.gz`.

## Set up update checks

The Legion Go starts the update timer at boot. It checks hourly and shows a
popup for each new stable tag when the Console desktop is running. The popup
stays until dismissed. Checks that run away from the Console desktop leave
the notification for a later check.

After installing the updater from a verified release, start the timer
immediately with these commands.

```sh
sudo systemctl daemon-reload
sudo systemctl start console-update.timer
```

Open **Console Updates** from the launcher, choose **Check for updates**,
then choose **Install** when ready.
An administrator can also install a specific tag with
`sudo console-update console-v0.1.8`.

If a device with an older updater missed the migration, run
`sudo console-update legion-go-v0.1.7` once. The installed updater can then
find `console-v` releases and read earlier installation and rollback records.

On a Legion Go with a login pattern, an administrator can enable pattern
approval with `sudo console-update --enroll-pattern`. Console Updates then
opens its own dialog with the available tag and accepts the saved login pattern
by touch or gamepad. Use the close button to dismiss the dialog.
The updater keeps a root-owned copy of the hash. Re-enroll after changing the
login pattern. Use `sudo console-update --forget-pattern` to return to password
approval.

Console Updates shows separate notices while downloading, verifying, and
installing. The command line reports the same steps.

The updater checks the archive against GitHub's asset digest and compares its
source commit with the fetched tag. `console apply` takes the existing Snapper
snapshots before exchanging the new checkout with `/etc/console`, then applies
the bundled binaries and manifest. The updater runs the desktop health check
before recording success.

To return to the previous release, open **Console Updates** and select
**Undo last update**. The updater shows which release it will restore and
asks for the enrolled update pattern or administrator password. Undo works
offline. You can also run `sudo console-update --rollback`.

The updater preserves the previous checkout and its binaries, and applies
them as a new generation. Background checks hold the rolled-back tag until a
newer stable tag appears. The previous snapshots are available through the
normal boot menu.
