{
  description = "The Rust Console desktop with an optional NixOS deployment backend";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      console = pkgs.callPackage ./nix/package.nix { src = self; };
      wpewebkit = pkgs.callPackage ./nix/wpewebkit.nix {
        webkitBubblewrapPatch =
          nixpkgs + "/pkgs/by-name/we/webkitgtk_6_0/fix-bubblewrap-paths.patch";
      };
      webkitWorker = pkgs.callPackage ./nix/browser-webkit-worker.nix { inherit wpewebkit; src = self; };
      browser = pkgs.callPackage ./nix/browser.nix { inherit webkitWorker; src = self; };
      chromiumBrowser = pkgs.callPackage ./nix/browser.nix { src = self; };
      manifest = import ./nix/manifest.nix ./desktop.conf;
    in
    {
      packages.${system} = {
        default = console;
        legion-console = console;
        inherit browser wpewebkit;
      };
      checks.${system} = {
        browser = import ./nix/browser-test.nix { inherit pkgs browser; };
        browser-chromium = import ./nix/browser-test.nix {
          inherit pkgs;
          browser = chromiumBrowser;
          webkit = false;
        };
      };
      devShells.${system}.default = pkgs.mkShell {
        inputsFrom = [ console ];
        packages = [ pkgs.rustc pkgs.cargo pkgs.clippy pkgs.rustfmt pkgs.just ];
      };
      nixosModules = {
        default = import ./nix/nixos-module.nix;
        console-desktop = self.nixosModules.default;
        legion-go = import ./nix/legion-go.nix;
      };
      # These describe inputs and coverage without compiling or installing.
      migration = {
        inherit manifest;
        packageDecisions = builtins.fromJSON (builtins.readFile ./nix/package-map.json);
        minimumRust = (builtins.fromTOML (builtins.readFile ./Cargo.toml))
          .workspace.package.rust-version;
        selectedRust = pkgs.rustc.version;
        selectedHyprland = pkgs.hyprland.version;
      };
    };
}
