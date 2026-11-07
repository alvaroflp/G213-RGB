#!/usr/bin/env sh
# Instalador do G213 RGB para Linux.
#
# Por padrão instala para o usuário atual em ~/.local, sem precisar de root,
# exceto para gravar a regra udev em /etc/udev/rules.d (o sudo é pedido uma vez).
#
# Uso:
#   ./install.sh                compila e instala para o usuário atual
#   ./install.sh --system       instala em /usr/local (requer sudo)
#   ./install.sh --no-build     usa o binário já compilado em target/release
#   ./install.sh --no-udev      não instala a regra de acesso ao /dev/hidraw*
#   ./install.sh --uninstall    remove o aplicativo instalado
#   ./install.sh --uninstall --purge
#                               remove também a configuração e os perfis
#   ./install.sh --help         mostra esta ajuda
#
# O atalho "G213 RGB" é criado no menu de aplicativos do ambiente gráfico.
set -eu

APP_ID="g213-rgb"
APP_NAME="G213 RGB"
ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

MODE="user"
ACTION="install"
BUILD=1
INSTALL_UDEV=1
PURGE=0

usage() {
    cat <<'EOF'
Instalador do G213 RGB para Linux.

Uso:
  ./install.sh                compila e instala para o usuário atual
  ./install.sh --system       instala em /usr/local (requer sudo)
  ./install.sh --no-build     usa o binário já compilado em target/release
  ./install.sh --no-udev      não instala a regra de acesso ao /dev/hidraw*
  ./install.sh --uninstall    remove o aplicativo instalado
  ./install.sh --uninstall --purge
                              remove também a configuração e os perfis
  ./install.sh --help         mostra esta ajuda

Variáveis:
  PREFIX                      destino da instalação (padrão: ~/.local
                              ou /usr/local com --system)
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --user) MODE="user" ;;
        --system) MODE="system" ;;
        --no-build) BUILD=0 ;;
        --no-udev) INSTALL_UDEV=0 ;;
        --uninstall | --remove) ACTION="uninstall" ;;
        --purge) PURGE=1 ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "Opção desconhecida: $1" >&2
            echo >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

if [ "$MODE" = "system" ]; then
    PREFIX=${PREFIX:-/usr/local}
    if [ "$(id -u)" -ne 0 ] && ! command -v sudo >/dev/null 2>&1; then
        echo "Erro: a instalação em $PREFIX requer root ou sudo." >&2
        exit 1
    fi
else
    if [ "$(id -u)" -eq 0 ]; then
        echo "Erro: não execute este instalador como root." >&2
        echo "Para instalar em /usr/local, use: ./install.sh --system" >&2
        exit 1
    fi
    PREFIX=${PREFIX:-$HOME/.local}
fi

BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor"
RULE_SRC="$ROOT_DIR/udev/60-g213-rgb.rules"
RULE_DEST="/etc/udev/rules.d/60-g213-rgb.rules"

# Executa comandos que gravam em diretórios do sistema.
if [ "$(id -u)" -eq 0 ]; then
    SYSTEM_CMD=""
elif command -v sudo >/dev/null 2>&1; then
    SYSTEM_CMD="sudo"
else
    SYSTEM_CMD=""
fi

run_system() {
    if [ -n "$SYSTEM_CMD" ]; then
        "$SYSTEM_CMD" "$@"
    else
        "$@"
    fi
}

update_caches() {
    if [ "$MODE" = "system" ]; then
        if command -v update-desktop-database >/dev/null 2>&1; then
            run_system update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
        fi
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            run_system gtk-update-icon-cache -q -t "$ICON_DIR" >/dev/null 2>&1 || true
        fi
    else
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
        fi
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            gtk-update-icon-cache -q -t "$ICON_DIR" >/dev/null 2>&1 || true
        fi
    fi
}

install_udev() {
    if [ ! -f "$RULE_SRC" ]; then
        echo "Aviso: regra udev não encontrada em $RULE_SRC." >&2
        return 0
    fi
    if [ "$(id -u)" -ne 0 ] && ! command -v sudo >/dev/null 2>&1; then
        echo "Aviso: sudo não encontrado; a regra udev não foi instalada." >&2
        echo "Instale manualmente:" >&2
        echo "    sudo install -m 0644 '$RULE_SRC' '$RULE_DEST'" >&2
        return 0
    fi

    echo "Instalando regra de acesso ao teclado (sudo)..."
    run_system install -m 0644 "$RULE_SRC" "$RULE_DEST"

    if command -v udevadm >/dev/null 2>&1; then
        run_system udevadm control --reload-rules
        run_system udevadm trigger --subsystem-match=hidraw
    fi
}

remove_udev() {
    if [ -f "$RULE_DEST" ]; then
        if [ "$(id -u)" -ne 0 ] && ! command -v sudo >/dev/null 2>&1; then
            echo "Aviso: sudo não encontrado; remova manualmente $RULE_DEST." >&2
        else
            run_system rm -f "$RULE_DEST"
            if command -v udevadm >/dev/null 2>&1; then
                run_system udevadm control --reload-rules
                run_system udevadm trigger --subsystem-match=hidraw
            fi
        fi
    fi
}

install_app() {
    if [ "$BUILD" -eq 1 ]; then
        if ! command -v cargo >/dev/null 2>&1; then
            echo "Erro: cargo não encontrado." >&2
            echo "Instale o Rust em https://rustup.rs e execute novamente." >&2
            exit 1
        fi
        echo "Compilando o G213 RGB (cargo build --locked --release)..."
        (cd "$ROOT_DIR" && cargo build --locked --release)
    fi

    BIN_SRC="$ROOT_DIR/target/release/$APP_ID"
    if [ ! -x "$BIN_SRC" ]; then
        echo "Erro: executável não encontrado em $BIN_SRC." >&2
        echo "Compile com 'cargo build --locked --release' ou remova --no-build." >&2
        exit 1
    fi

    echo "Instalando em $PREFIX..."
    if [ "$MODE" = "system" ]; then
        run_system mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR/scalable/apps" "$ICON_DIR/256x256/apps" "$PREFIX/share/pixmaps"
        run_system install -m 0755 "$BIN_SRC" "$BIN_DIR/$APP_ID"
        run_system install -m 0644 "$ROOT_DIR/assets/g213-rgb.svg" "$ICON_DIR/scalable/apps/$APP_ID.svg"
        run_system install -m 0644 "$ROOT_DIR/assets/g213-rgb.png" "$ICON_DIR/256x256/apps/$APP_ID.png"
        run_system install -m 0644 "$ROOT_DIR/assets/g213-rgb.png" "$PREFIX/share/pixmaps/$APP_ID.png"
    else
        mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR/scalable/apps" "$ICON_DIR/256x256/apps" "$PREFIX/share/pixmaps"
        install -m 0755 "$BIN_SRC" "$BIN_DIR/$APP_ID"
        install -m 0644 "$ROOT_DIR/assets/g213-rgb.svg" "$ICON_DIR/scalable/apps/$APP_ID.svg"
        install -m 0644 "$ROOT_DIR/assets/g213-rgb.png" "$ICON_DIR/256x256/apps/$APP_ID.png"
        install -m 0644 "$ROOT_DIR/assets/g213-rgb.png" "$PREFIX/share/pixmaps/$APP_ID.png"
    fi

    DESKTOP_TMP=$(mktemp)
    cat >"$DESKTOP_TMP" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=Controle de iluminação do Logitech G213 Prodigy
Exec="$BIN_DIR/$APP_ID"
Icon=$APP_ID
Terminal=false
Categories=Utility;Settings;
Keywords=logitech;g213;rgb;keyboard;teclado;iluminacao;
StartupWMClass=$APP_ID
EOF
    if [ "$MODE" = "system" ]; then
        run_system install -m 0644 "$DESKTOP_TMP" "$APP_DIR/$APP_ID.desktop"
    else
        install -m 0644 "$DESKTOP_TMP" "$APP_DIR/$APP_ID.desktop"
    fi
    rm -f "$DESKTOP_TMP"

    if [ "$INSTALL_UDEV" -eq 1 ]; then
        install_udev
    else
        echo "Regra udev ignorada (--no-udev)."
    fi

    update_caches

    echo
    echo "Pronto! Abra \"$APP_NAME\" no menu de aplicativos ou execute:"
    echo "    $BIN_DIR/$APP_ID"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *)
            echo
            echo "Para executar pelo nome \"$APP_ID\", adicione ao PATH:"
            echo "    export PATH=\"$BIN_DIR:\$PATH\""
            ;;
    esac
    if [ "$INSTALL_UDEV" -eq 1 ]; then
        echo
        echo "Se o teclado já estava conectado, desconecte e reconecte-o"
        echo "para que a permissão de acesso ao dispositivo passe a valer."
    fi
}

uninstall_app() {
    echo "Removendo o G213 RGB de $PREFIX..."
    if [ "$MODE" = "system" ]; then
        run_system rm -f "$BIN_DIR/$APP_ID" "$APP_DIR/$APP_ID.desktop"
        run_system rm -f "$ICON_DIR/scalable/apps/$APP_ID.svg" "$ICON_DIR/256x256/apps/$APP_ID.png"
        run_system rm -f "$PREFIX/share/pixmaps/$APP_ID.png"
    else
        rm -f "$BIN_DIR/$APP_ID" "$APP_DIR/$APP_ID.desktop"
        rm -f "$ICON_DIR/scalable/apps/$APP_ID.svg" "$ICON_DIR/256x256/apps/$APP_ID.png"
        rm -f "$PREFIX/share/pixmaps/$APP_ID.png"
    fi

    if [ "$INSTALL_UDEV" -eq 1 ]; then
        remove_udev
    else
        echo "Regra udev mantida (--no-udev)."
    fi
    update_caches

    if [ "$PURGE" -eq 1 ]; then
        CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/$APP_ID"
        rm -rf "$CONFIG_DIR"
        echo "Configuração e perfis removidos de $CONFIG_DIR"
    fi

    echo "Pronto. O G213 RGB foi removido."
}

case "$ACTION" in
    install) install_app ;;
    uninstall) uninstall_app ;;
esac
