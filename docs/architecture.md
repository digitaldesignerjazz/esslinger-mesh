# Esslinger Mesh architecture (public overview)

Esslinger Mesh is a private experiment design for two independent Cosmos
SDK/CometBFT chains: one for X-Coin and one for Q-Coin. IBC is the planned
transport for controlled cross-chain transfers. A CosmWasm pool can provide
an opt-in constant-product exchange between the two denoms.

## Safety boundaries

- Chain identifiers, denoms, addresses, validator keys, and deployment
  endpoints are environment-specific and are intentionally not included here.
- Genesis files and runtime state stay outside this source tree.
- Wallet onboarding is handled out of band; no identity list or wallet address
  is published in this repository.
- Pool parameters and initial liquidity require explicit governance approval.
- This is not a production deployment guide and does not authorize a launch.

## Components

1. **X-Coin chain:** application-specific accounting and governance modules.
2. **Q-Coin chain:** application-specific accounting, governance, and an
   optional CosmWasm pool.
3. **IBC relaying:** a separately reviewed relayer configuration connects the
   chains; credentials are never stored in source control.
4. **AMM prototype:** `contracts/amm` implements two-denom liquidity,
   withdrawals, swaps, slippage limits, and a basis-point fee.
5. **NFT prototype:** `contracts/nft` is a CW721-compatible collection and
   `contracts/nft-market` is a separate fixed-price escrow. Both are for the
   Q-Coin chain only, use the same 30 bps fee scale as the pool, and are not
   audited. They do not change the AMM. Deploy commands live in
   `contracts/DEPLOY.md`.

The design remains subject to protocol, licensing, trademark, and security
review before any public service or token activity. Public source in this
repository other than `contracts/amm`, `contracts/nft` and `contracts/nft-market`
is dedicated to the public domain under CC0 1.0; those three folders remain
Apache-2.0.
