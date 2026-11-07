#!/usr/bin/env sh
# Instala a regra udev que permite ao usuário da sessão abrir a interface de
# iluminação do Logitech G213 Prodigy sem rodar o aplicativo como root.
set -eu

RULE_NAME="60-g213-rgb.rules"
SRC_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SRC="$SRC_DIR/udev/$RULE_NAME"
DEST="/etc/udev/rules.d/$RULE_NAME"

if [ ! -f "$SRC" ]; then
    echo "Erro: regra não encontrada em $SRC" >&2
    exit 1
fi

if [ "$(id -u)" -ne 0 ]; then
    echo "Este instalador precisa de sudo para gravar em /etc/udev/rules.d."
    exec sudo -- "$0" "$@"
fi

install -m 0644 "$SRC" "$DEST"
echo "Regra instalada em $DEST"

if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules
    udevadm trigger --subsystem-match=hidraw
    echo "Regras udev recarregadas."
fi

cat <<'EOF'

Pronto! Agora desconecte e reconecte o teclado G213 (ou reinicie a sessão)
para que a nova permissão valha para /dev/hidraw*.

Verifique com:
    ./g213-rgb --diagnose
EOF
