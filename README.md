# Esslinger Mesh

Esslinger Mesh – two Cosmos SDK-based chains (X-Coin, Q-Coin) with IBC.

This repository is a **local, public-source preparation only** for a private
experiment network. It contains a small constant-product AMM, a CW721
collection, a fixed-price NFT escrow, and neutral design documentation. It
does not contain live chain state, wallets, validator material, join cards,
genesis files, or credentials. Contract store and instantiate commands for the
operator host are in `contracts/DEPLOY.md`.

## Scope

- two independent Cosmos SDK/CometBFT chains for X-Coin and Q-Coin;
- IBC as the planned inter-chain transport;
- an optional CosmWasm constant-product pool for the two denoms;
- an optional CW721 collection and a separate fixed-price escrow, on the Q-Coin chain only;
- governance and safety review before any deployment or value-bearing use.

The AMM, the CW721 collection, and the NFT escrow are prototypes, not audited
software. Do not deploy them with real assets without independent review and
explicit governance approval.

## Layout

- `contracts/amm/` — reviewed source for the minimal CosmWasm AMM;
- `contracts/nft/` — CW721-compatible collection (Apache-2.0, not audited);
- `contracts/nft-market/` — fixed-price NFT escrow (Apache-2.0, not audited);
- `contracts/DEPLOY.md` — store and instantiate commands for `nexus-qcoin-1`;
- `docs/architecture.md` — public, implementation-neutral architecture notes;
- `docs/join-card-template.md` — empty onboarding template without identities;
- `examples/` — non-secret configuration examples only.

## Whitepaper

Esslinger Mesh Whitepaper v1.1 (04.10.2026):

- [PDF](whitepaper/esslinger-mesh-whitepaper-v1.1.pdf)
- [Markdown](whitepaper/esslinger-mesh-whitepaper-v1.1.md)

## Lizenz

Lizenz: Public Domain – freigegeben unter Creative Commons CC0 1.0 Universal (https://creativecommons.org/publicdomain/zero/1.0/), siehe [LICENSE](LICENSE).

Die Ordner `contracts/amm`, `contracts/nft` und `contracts/nft-market` enthalten Code unter Apache-2.0 (siehe die `LICENSE` in jedem Ordner); für sie gilt diese Lizenz. Die NFT-Contracts sind nicht auditiert.
