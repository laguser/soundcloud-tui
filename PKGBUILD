# Maintainer: laguser <laguser@users.noreply.github.com>
pkgname=soundcloud-tui-git
pkgver=0.2.0
pkgrel=1
pkgdesc="Minimalist terminal SoundCloud & YouTube player for Linux in Rust, crafted with @ttype aesthetic"
arch=('x86_64' 'aarch64')
url="https://github.com/laguser/soundcloud-tui"
license=('MIT')
depends=('mpv' 'yt-dlp')
makedepends=('cargo' 'git')
provides=('soundcloud-tui' 'sctui')
conflicts=('soundcloud-tui' 'sctui')
source=("git+https://github.com/laguser/soundcloud-tui.git")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/soundcloud-tui"
  printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

build() {
  cd "$srcdir/soundcloud-tui"
  cargo build --release --locked
}

package() {
  cd "$srcdir/soundcloud-tui"
  install -Dm755 "target/release/sctui" "$pkgdir/usr/bin/sctui"
  install -Dm644 "README.md" "$pkgdir/usr/share/doc/soundcloud-tui/README.md"
}
