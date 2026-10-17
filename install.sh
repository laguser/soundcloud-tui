#!/usr/bin/env bash
set -e

# soundcloud-tui installer script (minimalist & clean like ttype)
LANG_PREF="${LANG:-en}"

is_ru() {
    [[ "$LANG_PREF" =~ ^ru ]]
}

msg() {
    if is_ru; then
        echo -e "\033[1;35m==>\033[0m \033[1m$1\033[0m"
    else
        echo -e "\033[1;35m==>\033[0m \033[1m$2\033[0m"
    fi
}

err() {
    if is_ru; then
        echo -e "\033[1;31m[ошибка]\033[0m $1" >&2
    else
        echo -e "\033[1;31m[error]\033[0m $2" >&2
    fi
    exit 1
}

# check dependencies
command -v git >/dev/null 2>&1 || err "git не найден. Установите git." "git is required but not installed."
command -v cargo >/dev/null 2>&1 || err "cargo (Rust) не найден. Установите rustup/cargo." "cargo (Rust) is required. Install rustup/cargo."
command -v mpv >/dev/null 2>&1 || err "mpv не найден. Установите: sudo pacman -S mpv (или apt install mpv)." "mpv is required for audio streaming."
command -v yt-dlp >/dev/null 2>&1 || err "yt-dlp не найден. Установите: sudo pacman -S yt-dlp." "yt-dlp is required for search and streaming links."

TMP_DIR="$(mktemp -d /tmp/sctui-build.XXXXXX)"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

msg "Клонирование репозитория..." "Cloning repository..."
git clone --depth 1 https://github.com/laguser/soundcloud-tui.git "$TMP_DIR" >/dev/null 2>&1 || {
    # fallback to local repo if running locally
    if [ -f "./Cargo.toml" ]; then
        cp -r ./* "$TMP_DIR/"
    else
        err "Не удалось загрузить репозиторий" "Failed to clone repository"
    fi
}

msg "Сборка релизного бинарника..." "Compiling release binary..."
cd "$TMP_DIR"
cargo build --release --quiet

INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"
cp target/release/sctui "$INSTALL_DIR/sctui"
chmod +x "$INSTALL_DIR/sctui"

msg "Готово! Установлено в $INSTALL_DIR/sctui" "Done! Installed to $INSTALL_DIR/sctui"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    if is_ru; then
        echo -e "\033[0;33mВнимание:\033[0m $INSTALL_DIR отсутствует в вашем \$PATH."
        echo "Добавьте в ~/.bashrc или ~/.zshrc: export PATH=\"\$HOME/.local/bin:\$PATH\""
    else
        echo -e "\033[0;33mNote:\033[0m $INSTALL_DIR is not in your \$PATH."
        echo "Add to ~/.bashrc or ~/.zshrc: export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi
fi

if is_ru; then
    echo -e "Запуск: \033[1;35msctui\033[0m"
else
    echo -e "Run: \033[1;35msctui\033[0m"
fi
