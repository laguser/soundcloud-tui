# Maintainer: laguser <laguser@users.noreply.github.com>
pkgname=soundcloud-tui-git
pkgver=0.2.0.r0.ga814938
pkgrel=1
pkgdesc="Minimalist terminal SoundCloud & YouTube player for Linux in Rust (@ttype aesthetic, MPRIS2, Synced Lyrics)"
arch=('x86_64')
url="https://github.com/laguser/soundcloud-tui"
license=('MIT')
depends=('mpv' 'dbus')
makedepends=('cargo' 'git' 'pkg-config')
optdepends=(
    'yt-dlp: For resolving tracks and playlists'
    'curl: For network fetching'
)
provides=('soundcloud-tui' 'sctui')
conflicts=('soundcloud-tui')
source=("$pkgname::git+https://github.com/laguser/soundcloud-tui.git")
sha256sums=('SKIP')

pkgver() {
    cd "$pkgname"
    git describe --long --tags --always 2>/dev/null | sed 's/\([^-]*-g\)/r\1/;s/-/./g' || printf "0.2.0.r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
    cd "$pkgname"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-features
}

check() {
    cd "$pkgname"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen --all-features
}

package() {
    cd "$pkgname"
    install -Dm755 "target/release/sctui" "$pkgdir/usr/bin/sctui"
    ln -sf "/usr/bin/sctui" "$pkgdir/usr/bin/soundcloud-tui"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE" 2>/dev/null || true
}
