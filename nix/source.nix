{ lib, src }:
lib.cleanSourceWith {
  inherit src;
  name = "console-source";
  filter = path: type:
    let
      name = builtins.baseNameOf path;
      top = builtins.head (lib.splitString "/" (lib.removePrefix "${toString src}/" (toString path)));
    in builtins.elem top [
      ".cargo" ".github" ".gitignore" "AGENTS.md" "BALANCED-PLAYBACK.md"
      "Cargo.toml" "Cargo.lock" "LICENSE" "README.md" "configuration.nix"
      "crates" "desktop.conf" "docs" "files" "flake.lock" "flake.nix" "forks"
      "justfile" "machines.conf" "migrations" "mise.lock" "mise.toml" "nix"
      "os_design.md" "rustfmt.toml" "scenarios" "theme" "tools" "vocabulary.conf" "words.conf"
    ]
      && !(builtins.elem name [ ".git" "target" ".direnv" ".envrc" ".env"
        ".claude" ".crush" ".codex" ".stage" "node_modules" ])
      && !(lib.hasPrefix ".env." name)
      && !(type == "symlink" && lib.hasPrefix "result" name);
}
