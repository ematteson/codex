#!/usr/bin/env bash
# Install the locally built codex-explore binary to a stable path.
#
# The alias/PATH entry ~/.local/bin/codex-explore is a symlink to
# ~/.local/libexec/codex-explore/codex — NOT into codex-rs/target/, because a
# cargo clean (or a target dir sweep) deletes the target binary and leaves the
# alias dangling. Build, then run this to promote.
#
#   cd codex-rs && cargo build --release --bin codex && cd .. && ./install-explore.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
built="$repo_root/codex-rs/target/release/codex"
install_dir="$HOME/.local/libexec/codex-explore"
install_path="$install_dir/codex"
link_path="$HOME/.local/bin/codex-explore"

[ -x "$built" ] || { echo "no release binary at $built — build first" >&2; exit 1; }

"$built" --version >/dev/null || { echo "built binary failed --version smoke test" >&2; exit 1; }

mkdir -p "$install_dir" "$(dirname "$link_path")"
if [ -e "$install_path" ]; then
  mv -f "$install_path" "$install_path.prev"   # one-deep rollback
fi
cp "$built" "$install_path"
chmod +x "$install_path"
ln -sfn "$install_path" "$link_path"

echo "installed: $("$link_path" --version) -> $install_path"
echo "source commit: $(git -C "$repo_root" rev-parse --short HEAD)"
