#!/usr/bin/env sh
# Compila (quando necessário) e executa o G213 RGB sem instalar.
#
# Exemplos:
#   ./run.sh
#   ./run.sh --preview
#   ./run.sh --diagnose
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec cargo run --locked --release --manifest-path "$ROOT_DIR/Cargo.toml" -- "$@"
