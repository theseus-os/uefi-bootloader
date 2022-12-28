set -e

cargo b --target x86_64-unknown-uefi
cargo b --target aarch64-unknown-uefi
# unsupported
cargo b --target i686-unknown-uefi
