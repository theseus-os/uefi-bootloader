set -e

cargo clippy --manifest-path uefi-bootloader/Cargo.toml --target x86_64-unknown-uefi
cargo clippy --manifest-path uefi-bootloader/Cargo.toml --target i686-unknown-uefi
cargo clippy --manifest-path uefi-bootloader/Cargo.toml --target aarch64-unknown-uefi
# unsupported
