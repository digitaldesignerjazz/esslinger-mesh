#!/usr/bin/env bash
# Build both CosmWasm crates. Does not store or instantiate.
set -euo pipefail

root="$(cd "$(dirname "$0")" && pwd)"
cd "$root"

if ! rustup target list --installed | grep -qx 'wasm32-unknown-unknown'; then
  rustup target add wasm32-unknown-unknown
fi

cargo build --release --target wasm32-unknown-unknown --manifest-path "$root/nft/Cargo.toml"
cargo build --release --target wasm32-unknown-unknown --manifest-path "$root/nft-market/Cargo.toml"

mkdir -p "$root/artifacts"
cp "$root/nft/target/wasm32-unknown-unknown/release/esslinger_mesh_cw721.wasm" "$root/artifacts/"
cp "$root/nft-market/target/wasm32-unknown-unknown/release/esslinger_mesh_nft_market.wasm" "$root/artifacts/"
(
  cd "$root/artifacts"
  sha256sum esslinger_mesh_cw721.wasm esslinger_mesh_nft_market.wasm | tee SHA256SUMS
)
echo "wasm ready in $root/artifacts"
echo "Not stored. See DEPLOY.md. These contracts are not audited."
