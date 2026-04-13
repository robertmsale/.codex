#!/bin/zsh

BIN_CMD="$1"
BIN_PATH="$HOME/.codex/shim/$BIN_CMD"

cat > "$BIN_PATH" <<EOF
#!/bin/zsh
set -euo pipefail

source "\$HOME/.codex/scripts/common.sh"

PATH="\$(unshim_current_path)" exec rtk $BIN_CMD "\$@"
EOF

chmod +x "$BIN_PATH"
