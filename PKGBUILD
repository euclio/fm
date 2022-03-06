
# Maintainer: Andy Russell <arussell123@gmail.com>

pkgname=fm-git
pkgver=VERSION
pkgrel=1
pkgdesc="Small, general purpose file manager built with GTK"
arch=('x86_64')
url="https://github.com/euclio/fm"
license=('MIT')
groups=()
depends=('gtk4' 'libadwaita' 'libpanel' 'gtksourceview5')
makedepends=('git' 'rust' 'cargo')
provides=("fm")
conflicts=("fm")
source=("git+$url")
sha256sums=('SKIP')

pkgver() {
  cd "${pkgname%-git}"
  ( set -o pipefail
    git describe --long 2>/dev/null | sed 's/\([^-]*-g\)/r\1/;s/-/./g' ||
    printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
  )
}

build() {
  cd "$srcdir/${pkgname%-git}"
  cargo build --release
}

check() {
  cd "$srcdir/${pkgname%-git}"
  cargo test --release
}

package() {
  cd "$srcdir/${pkgname%-git}"

  install -Dm755 "$srcdir/${pkgname%-git}/target/release/fm" "$pkgdir/usr/bin/fm"
  install -Dm644 LICENSE -t "${pkgdir}/usr/share/licenses/${pkgname%-git}"
}
