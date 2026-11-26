# Console releases

Console desktops on x86-64 Arch Linux and compatible distributions can install
prebuilt releases from `reeveng/hypr-console`. A release includes the programs
named by `desktop.conf` and all machine profiles in `machines.conf`. The apply
engine selects the profile that matches the host.

## Publish a release

Export the intended commit into a checkout of the public repository with
`console-manifest-publish --export-only`. Commit the verified export to the
public repository and push a new tag such as `console-v0.1.8` on that commit.
GitHub Actions builds and publishes the release.

The exporter runs the privacy check locally. It scans exported paths and file
contents for the build account, hostname, and configured Git name and email.
Use `--private-identifiers FILE` to also check for other known identifiers,
such as a personal name or a device nickname. Keep that file outside the source
and export checkouts, with one identifier per line. Export-only mode exits with
an error if it finds a match or cannot finish the check.

During a staged rollout, add `Console-Latest: false` as a trailer on the
annotated tag to keep the current release marked latest. The new release
can be installed by tag as soon as it is published.

The build uses an Arch Linux container. From `console-v0.1.13` it targets
baseline x86-64. The `console-x86_64.tar.gz` archive contains the built binaries
and their source commit. GitHub publishes its SHA-256 digest with the immutable
release metadata.

## Set up update checks

Console starts the update timer at boot. It checks hourly and shows a
popup for each new stable tag when the Console desktop is running. The popup
stays until dismissed. Checks that run away from the Console desktop leave
the notification for a later check.

After installing the updater from a verified release, start the timer
immediately with these commands.

```sh
sudo systemctl daemon-reload
sudo systemctl start console-update.timer
```

Open **Console Updates** from the launcher, choose **Check for updates**, then
choose **Install update** when ready. An administrator can also install a
specific tag with `sudo console-update console-v0.1.13`.

To switch an existing source or package installation, first install the shared
updater through that installation's usual update method. Source installations
can run `sudo console apply --root "$PWD"` from the updated source checkout.
Then use **Console Updates** or `sudo console-update` to install the latest
release. New release installations require `console-v0.1.13` or later.

If a device still runs an updater that requires `legion-go-v` tags, run
`sudo console-update legion-go-v0.1.7` once. The installed updater can then
find `console-v` releases and read earlier installation and rollback records.

On a Console desktop with a login pattern, an administrator can enable pattern
approval with `sudo console-update --enroll-pattern`. Console Updates then
opens its own dialog with the available tag and accepts the saved login pattern
by touch or gamepad. Use the close button to dismiss the dialog.
The updater keeps a root-owned copy of the hash. Re-enroll after changing the
login pattern. Use `sudo console-update --forget-pattern` to return to password
approval.

Console Updates shows separate notices while downloading, verifying, and
installing. The command line reports the same steps.

The updater checks the archive against GitHub's asset digest and compares its
source commit with the fetched tag. `console apply` takes Snapper snapshots
where configured before activating the checkout, then applies the bundled
binaries and manifest. Existing Console checkouts in `/etc/console` use a
checkout exchange. If `/etc/console` only contains configuration, the first
release checkout is installed at `/etc/console/.release`. The updater runs the
desktop health check before recording success. If the first installation stops
after activation, `sudo console-update` resumes it from the saved release.

To return to the previous release, open **Console Updates** and select
**Undo last update**. The updater shows which release it will restore and
asks for the enrolled update pattern or administrator password. Undo works
offline. You can also run `sudo console-update --rollback`.

The updater preserves the previous checkout and its binaries, and applies
them as a new generation. **Undo last update** becomes available once there is
a previous managed checkout. Background checks hold the rolled-back tag until
a newer stable tag appears. Where the boot menu supports snapshots, the
previous snapshots are available there.
