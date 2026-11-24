#!/usr/bin/env bash
set -euo pipefail

base=$1
root=$(git rev-parse --show-toplevel)
cd "$root"
toolchain=nightly-2026-05-28
export RUSTUP_TOOLCHAIN=$toolchain

scratch=$(mktemp -d "${TMPDIR:-/var/tmp}/explicit-ci.XXXXXXXX")
cleanup() {
    if [[ -d "$scratch/base" ]]; then
        git worktree remove --force "$scratch/base"
    fi
    rm -rf "$scratch"
}
trap cleanup EXIT

git diff --name-only --no-renames -z "$base" HEAD > "$scratch/changed"
if [[ ! -s "$scratch/changed" ]]; then
    exit 0
fi

git worktree add --quiet --detach "$scratch/base" "$base"
for tree in "$root" "$scratch/base"; do
    cargo +"$toolchain" fetch --locked --manifest-path "$tree/Cargo.toml"
done

package_scope() {
    local tree=$1
    (
        cd "$tree"
        cargo +"$toolchain" metadata --offline --locked --all-features --format-version=1
    ) | python3 -c '
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
paths = Path(sys.argv[2]).read_bytes().decode().split("\0")
metadata = json.load(sys.stdin)
whole_workspace = any(
    path == ".cargo/config.toml" or (
        path.startswith("tools/explicit-rust/")
        and Path(path).suffix in (".rs", ".toml", ".lock")
    )
    for path in paths
)
for package in sorted(metadata["packages"], key=lambda entry: entry["name"]):
    directory = Path(package["manifest_path"]).parent
    if not directory.is_relative_to(root):
        continue
    relative = directory.relative_to(root)
    if package["id"] not in metadata["workspace_members"] and (
        package["source"] is not None or relative.parts[0] != "forks"
    ):
        continue
    if whole_workspace or any(
        Path(path).is_relative_to(relative)
        and Path(path).suffix in (".rs", ".toml", ".lock")
        for path in paths if path
    ):
        print(package["name"])
' "$tree" "$scratch/changed"
}

package_scope "$root" > "$scratch/head-packages"
if [[ ! -s "$scratch/head-packages" ]]; then
    echo "No changed packages in the Explicit Rust audit."
    exit 0
fi
package_scope "$scratch/base" > "$scratch/base-packages"

for tree in "$root" "$scratch/base"; do
    cargo +"$toolchain" fetch --locked --manifest-path "$tree/tools/explicit-rust/Cargo.toml"
    (cd "$tree" && cargo +"$toolchain" dylint --lib explicit012_safety_doc -- --locked -p console-core-never)
done

audit() {
    local tree=$1 output=$2 packages=$3 status
    local -a scope=()
    while IFS= read -r package; do
        scope+=(--package "$package")
    done < "$packages"

    status=0
    "$scratch/target/debug/batch" audit --root "$tree" --output "$output" \
        "${scope[@]}" "${@:4}" || status=$?
    if [[ "$status" == 1 ]]; then
        find "$output" -name '*.stderr' -print -exec tail -n 60 {} \;
    elif [[ "$status" == 2 && "$tree" == "$root" ]]; then
        find "$output" -name regressions.json -print -exec head -n 160 {} \;
    fi
    return "$status"
}

cargo +"$toolchain" build --locked --manifest-path tools/explicit-rust/migration-collector/Cargo.toml \
    --target-dir "$scratch/target" --bin batch

if [[ -s "$scratch/base-packages" ]]; then
    status=0
    audit "$scratch/base" "$scratch/baseline" "$scratch/base-packages" || status=$?
    if [[ "$status" != 0 && "$status" != 2 ]]; then
        exit "$status"
    fi
    reports=("$scratch"/baseline/run-*/remaining.json)
    audit "$root" "$root/target/explicit-ci" "$scratch/head-packages" --baseline "${reports[0]}"
else
    audit "$root" "$root/target/explicit-ci" "$scratch/head-packages"
fi
