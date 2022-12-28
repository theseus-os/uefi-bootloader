set -e

cargo clippy --target x86_64-unknown-uefi
cargo clippy --target aarch64-unknown-uefi
# unsupported
cargo clippy --target i686-unknown-uefi
