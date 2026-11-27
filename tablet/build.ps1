#requires -Version 7.0
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

$build = @'
set -eu
apt-get update -qq
apt-get install --no-install-recommends -qy gcc-aarch64-linux-gnu libc6-dev-arm64-cross >/dev/null
rustup target add aarch64-unknown-linux-gnu
cargo build --locked --release --target aarch64-unknown-linux-gnu --target-dir target/tablet
file target/tablet/aarch64-unknown-linux-gnu/release/rembiblestudy
'@

& docker run --rm --mount "type=bind,source=$root,target=/work" --workdir /work `
    -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc `
    rust:1-bookworm bash -c $build
if ($LASTEXITCODE -ne 0) {
    throw "Paper Pro build failed with exit code $LASTEXITCODE"
}
