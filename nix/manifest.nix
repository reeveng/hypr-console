# Read the existing manifest rather than maintaining a second binary list.
# Keep this parser deliberately strict: a new section requires a port decision.
file:
let
  trim = line:
    let matched = builtins.match "[[:space:]]*(.*[^[:space:]])[[:space:]]*" line;
    in if matched == null then "" else builtins.head matched;
  lines = builtins.filter builtins.isString (builtins.split "\n" (builtins.readFile file));
  sections = [ "packages" "build" "files" "services" "masked" ];
  initial = { section = ""; values = builtins.listToAttrs
    (map (name: { inherit name; value = []; }) sections); };
  step = state: raw:
    let
      line = trim raw;
      length = builtins.stringLength line;
      heading = length >= 2 && builtins.substring 0 1 line == "["
        && builtins.substring (length - 1) 1 line == "]";
    in
      if line == "" || builtins.substring 0 1 line == "#" then state
      else if heading then
        let section = builtins.substring 1 (length - 2) line;
        in if builtins.elem section sections then state // { inherit section; }
        else throw "Console manifest has an unhandled section: ${section}"
      else if state.section == "" then throw "Console manifest entry has no section: ${line}"
      else state // { values = state.values // {
        ${state.section} = state.values.${state.section} ++ [ line ];
      }; };
in
(builtins.foldl' step initial lines).values
