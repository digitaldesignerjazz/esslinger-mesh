# Esslinger Mesh

Esslinger Mesh – two Cosmos SDK-based chains (X-Coin, Q-Coin) with IBC.

This repository is a **local, public-source preparation only** for a private
experiment network. It contains a small, reviewed constant-product AMM
prototype and neutral design documentation. It does not contain live chain
state, wallets, validator material, join cards, genesis files, credentials, or
network access instructions.

## Scope

- two independent Cosmos SDK/CometBFT chains for X-Coin and Q-Coin;
- IBC as the planned inter-chain transport;
- an optional CosmWasm constant-product pool for the two denoms;
- governance and safety review before any deployment or value-bearing use.

The AMM is a prototype, not audited software. Do not deploy it with real
assets without independent review and explicit governance approval.

## Layout

- `contracts/amm/` — reviewed source for the minimal CosmWasm AMM;
- `docs/architecture.md` — public, implementation-neutral architecture notes;
- `docs/join-card-template.md` — empty onboarding template without identities;
- `examples/` — non-secret configuration examples only.

## Whitepaper

Esslinger Mesh Whitepaper v1.1 (04.10.2026):

- [PDF](whitepaper/esslinger-mesh-whitepaper-v1.1.pdf)
- [Markdown](whitepaper/esslinger-mesh-whitepaper-v1.1.md)

## Lizenz

Lizenz: Public Domain – freigegeben unter Creative Commons CC0 1.0 Universal (https://creativecommons.org/publicdomain/zero/1.0/), siehe [LICENSE](LICENSE).

Der Ordner `contracts/amm` enthält Code unter Apache-2.0 (siehe `contracts/amm/LICENSE`); für ihn gilt diese Lizenz.
