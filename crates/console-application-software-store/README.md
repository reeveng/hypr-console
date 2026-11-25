# Console Store

Browse featured desktop apps or search by name. Results identify the package
manager. Select a result to install it or open **Installed** to launch or
remove an installed package.

Run from a Wayland session with `cargo run --locked -p console-application-software-store`.
The Console desktop manifest installs the **Console Store** launcher.

Console provisions Nix on Arch and enables it on NixOS. The Store uses Nixpkgs
on Linux hosts with Nix and keeps its Nix packages in its own profile
under `$XDG_STATE_HOME/console/software-store`. The first Nixpkgs search may
download the catalogue.

Pacman, Flatpak, and mise appear when available. On Arch, pacman lists
explicitly installed packages and installed desktop apps. System changes ask
for administrator approval. With yay installed, search also includes the AUR
and installation opens a terminal to review the build. Flatpak uses its
configured user and system remotes. Mise installs into its shared tool cache
and removes individual tool versions.

Package managers can themselves appear as installable packages when a source
on the current system offers them.
