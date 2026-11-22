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

## Set up automatic updates

Install `console-update` from a verified release for the initial switch. Run
the first install by tag on the device as root.

```sh
sudo console-update legion-go-v0.1.0
sudo systemctl daemon-reload
sudo systemctl enable --now console-update.timer
```

The updater checks the archive against GitHub's asset digest and compares its
source commit with the fetched tag. `console apply` takes the existing Snapper
snapshots before exchanging the new checkout with `/etc/console`, then applies
the bundled binaries and manifest. The updater runs the desktop health check
before recording success. The timer checks hourly and applies while the desktop
is idle or locked.

To return to the previous release, run `sudo console-update --rollback`. The
updater preserves that checkout and its binaries, and applies them as a new
generation. Automatic updates hold the rolled-back tag until a newer stable tag
appears. The previous snapshots are available through the normal boot menu.
