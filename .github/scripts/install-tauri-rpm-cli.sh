#!/usr/bin/env bash
# Tauri CLI 2.12.4 passes SemVer prereleases directly to RPM, which rejects '-'.
# Apply only the upstream RPM fix until a released CLI includes tauri#16336.
set -euo pipefail

SOURCE="$RUNNER_TEMP/tauri-rpm-source"
PATCH="$RUNNER_TEMP/tauri-rpm-prerelease.patch"
INSTALL="$RUNNER_TEMP/tauri-rpm-install"
BASE_COMMIT=993038c49185307dfc895ab7657d77ca0d5ed7c7
FIX_COMMIT=a200fae133970fc85a50c2ba30ac184769278d17

git clone --depth 1 --branch tauri-cli-v2.12.4 https://github.com/tauri-apps/tauri.git "$SOURCE"
test "$(git -C "$SOURCE" rev-parse HEAD)" = "$BASE_COMMIT"
curl --fail --location "https://github.com/tauri-apps/tauri/commit/$FIX_COMMIT.patch" --output "$PATCH"
printf '%s  %s\n' 8962dd43c0b45751adbfcd0cc5730476869cd7028f9389c8edb285268848a42a "$PATCH" | sha256sum --check --status
git -C "$SOURCE" apply --include=crates/tauri-bundler/src/bundle/linux/rpm.rs "$PATCH"

# Keep tool compilation separate from the application's locked dependencies.
export CARGO_TARGET_DIR="$RUNNER_TEMP/tauri-rpm-target"
cargo test --locked --release --manifest-path "$SOURCE/crates/tauri-bundler/Cargo.toml" bundle::linux::rpm::tests
cargo install --locked --path "$SOURCE/crates/tauri-cli" --root "$INSTALL"
printf '%s\n' "$INSTALL/bin" >> "$GITHUB_PATH"
