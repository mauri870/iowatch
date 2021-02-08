pkgname=iowatch
pkgver=0.1.0
pkgrel=0
arch=('x86_64')
pkgdesc='Cross-platform way to run arbitrary commands when files change'
url='https://github.com/mauri870/iowatch'
provides=('iowatch')
conflicts=('iowatch')
license=('MIT')
makedepends=('rust' 'git')
source=("${pkgname}::git+${url}.git")
md5sums=("SKIP")

build() {
    cd "${srcdir}/${pkgname}"
    cargo build --release
}

check() {
    cd "${srcdir}/${pkgname}"
    cargo test --release
}

package() {
    cd "${srcdir}/${pkgname}"
    install -Dm755 target/release/iowatch "$pkgdir"/usr/bin/iowatch
}
