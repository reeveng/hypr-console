# Direct module imports must not copy target/, caches, or local secrets into
# the Nix store. Flake inputs already filter untracked files through Git.
{ lib, src }:
lib.cleanSourceWith {
  inherit src;
  name = "console-source";
  filter = path: type:
    let name = builtins.baseNameOf path;
    in !(builtins.elem name [ ".git" "target" ".direnv" ".envrc" ".env"
      ".claude" ".crush" ".codex" ".stage" ])
      && !(lib.hasPrefix ".env." name)
      && !(type == "symlink" && lib.hasPrefix "result" name);
}
