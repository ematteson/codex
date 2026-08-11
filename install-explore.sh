#!/usr/bin/env bash
# Install the locally built codex-explore binary to a stable path.
#
# The alias/PATH entry ~/.local/bin/codex-explore is a symlink to
# ~/.local/libexec/codex-explore/codex — NOT into codex-rs/target/, because a
# cargo clean (or a target dir sweep) deletes the target binary and leaves the
# alias dangling. Build, then run this to promote.
#
#   cd codex-rs && cargo build --release --bin codex && cd .. && ./install-explore.sh
#
# Second binary: since 0.147.0 Codex shells out to a `codex-code-mode-host`
# helper, resolved as a SIBLING OF current_exe. On macOS current_exe is the
# un-resolved symlink path, so the helper has to sit in ~/.local/bin, not just
# beside the real binary in libexec. Without it, every command the agent runs
# dies with "failed to spawn code-mode host ...: host executable was not found".
#
# The helper cannot be built here: code-mode-runtime needs v8/v8_enable_sandbox,
# and rusty_v8 publishes no ptrcomp_sandbox prebuilt for aarch64-apple-darwin
# (404 -> would need V8_FROM_SOURCE=1, hours + tens of GB). We use the official
# upstream artifact for the matching tag instead; our fork does not patch that
# crate, so upstream's build is the same code.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
built="$repo_root/codex-rs/target/release/codex"
install_dir="$HOME/.local/libexec/codex-explore"
install_path="$install_dir/codex"
bin_dir="$HOME/.local/bin"
link_path="$bin_dir/codex-explore"
host_name="codex-code-mode-host"
host_path="$install_dir/$host_name"
host_stamp="$install_dir/.code-mode-host-version"

[ -x "$built" ] || { echo "no release binary at $built — build first" >&2; exit 1; }

version="$("$built" --version | awk '{print $NF}')"   # "codex-cli 0.147.0" -> 0.147.0
[ -n "$version" ] || { echo "could not read version from $built" >&2; exit 1; }

mkdir -p "$install_dir" "$bin_dir"

# --- codex itself ---------------------------------------------------------
if [ -e "$install_path" ]; then
  mv -f "$install_path" "$install_path.prev"   # one-deep rollback
fi
cp "$built" "$install_path"
chmod +x "$install_path"
ln -sfn "$install_path" "$link_path"

# --- code-mode host helper -----------------------------------------------
# Skip the download when the installed helper already matches this version.
if [ -x "$host_path" ] && [ "$(cat "$host_stamp" 2>/dev/null || true)" = "$version" ]; then
  echo "code-mode host already at $version"
else
  arch="$(uname -m)"
  case "$arch" in
    arm64|aarch64) target="aarch64-apple-darwin" ;;
    x86_64)        target="x86_64-apple-darwin" ;;
    *) echo "unsupported arch $arch for $host_name" >&2; exit 1 ;;
  esac
  asset="$host_name-$target.tar.gz"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  echo "fetching $asset from openai/codex rust-v$version"
  gh release download "rust-v$version" --repo openai/codex \
    --pattern "$asset" --dir "$tmp" --clobber
  shasum -a 256 "$tmp/$asset"
  tar -xzf "$tmp/$asset" -C "$tmp"
  # The tarball member is named <host>-<target>; normalise it.
  mv -f "$tmp/$host_name-$target" "$tmp/$host_name" 2>/dev/null || true
  chmod +x "$tmp/$host_name"
  "$tmp/$host_name" --help >/dev/null || {
    echo "$host_name failed its --help smoke test" >&2; exit 1; }
  mv -f "$tmp/$host_name" "$host_path"
  echo "$version" > "$host_stamp"
fi
# Must live next to the SYMLINK: current_exe is the unresolved path on macOS.
ln -sfn "$host_path" "$bin_dir/$host_name"

echo "installed: $("$link_path" --version) -> $install_path"
echo "code-mode host: $host_path (linked into $bin_dir)"
echo "source commit: $(git -C "$repo_root" rev-parse --short HEAD)"
