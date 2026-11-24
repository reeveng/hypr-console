# Software Store

Browse featured desktop apps, including OBS Studio, or search Nixpkgs by
name or description. Select a package to install it. **Installed** shows
packages managed by the store and lets you open or remove them.

Run from a Wayland session with `cargo run --locked -p console-application-software-store`.
The Console desktop manifest installs the **Software Store** launcher.

The store invokes the Nix already available on the machine and keeps installed
packages in its own profile under `$XDG_STATE_HOME/console/software-store`.
The first search may download the Nixpkgs catalogue. If Nix reports a failure,
**Details** shows its diagnostic.

The [OBS feature artwork](assets/obs-feature.png) was made with the built-in
image generation tool using this prompt.

> Use case: product-mockup. Asset type: OBS Studio feature banner artwork for a dark desktop software store. A polished photographic 3D studio still life of a compact video streaming setup: a dark graphite video switcher with tactile glowing pastel square keys and a single dark widescreen monitor behind it displaying an abstract violet and ice-blue lightwave on a black screen. Deep bluish-purple seamless stage, soft purple and cyan side lighting, clean premium hardware editorial photography. Wide landscape framing, all objects concentrated on the right two-thirds with breathing room and fading to solid near-black indigo on the left for an overlaid headline. No people, no text, no letters, no logos, no watermarks.
