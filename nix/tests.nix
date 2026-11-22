# Offline contract checks; no packages or Nixpkgs input are needed.
let
  source = ../.;
  manifest = import ./manifest.nix (source + "/desktop.conf");
  decisions = builtins.fromJSON (builtins.readFile ./package-map.json);
  unique = names: builtins.attrNames (builtins.listToAttrs
    (map (name: { inherit name; value = true; }) names));
  crates = builtins.filter (name:
    builtins.pathExists (source + "/crates/${name}/Cargo.toml")
  ) (builtins.attrNames (builtins.readDir (source + "/crates")));
  binaries = builtins.concatLists (map (name:
    let
      root = source + "/crates/${name}";
      cargo = builtins.fromTOML (builtins.readFile (root + "/Cargo.toml"));
      explicit = map (bin: bin.name) (cargo.bin or []);
      implicit = if builtins.pathExists (root + "/src/main.rs")
        && !(builtins.elem cargo.package.name explicit)
        then [ cargo.package.name ] else [];
    in explicit ++ implicit
  ) crates);
  missingBinaries = builtins.filter (name: !(builtins.elem name binaries)) manifest.build;
  unhandledPackages = builtins.filter
    (name: !(builtins.hasAttr name decisions)) manifest.packages;
  missingFiles = builtins.filter
    (name: !(builtins.pathExists (source + "/files${name}"))) manifest.files;
  unknown = builtins.tryEval (builtins.deepSeq
    (import ./manifest.nix (builtins.toFile "console-invalid-manifest" "[unexpected]\nentry\n")) true);
  whitespace = import ./manifest.nix (builtins.toFile "console-whitespace-manifest"
    "  # comment\n [packages] \n  example  \n\n[build]\nprogram\n");
in
assert missingBinaries == [];
assert unhandledPackages == [];
assert missingFiles == [];
assert builtins.length (unique manifest.build) == builtins.length manifest.build;
assert !unknown.success;
assert whitespace.packages == [ "example" ];
{
  passed = true;
  binaryCount = builtins.length manifest.build;
  packageCount = builtins.length manifest.packages;
  fileCount = builtins.length manifest.files;
  inherit missingBinaries unhandledPackages missingFiles;
}
