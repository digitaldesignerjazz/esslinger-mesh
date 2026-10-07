# Security Policy

Esslinger Cyberspace (repository `esslinger-mesh`) is an **experimental research
and test network**. The CosmWasm contracts in `contracts/` (AMM, CW721
collection, NFT escrow) are prototypes and **have not been audited**. Do not
use them with real assets.

## Supported versions

Only the current `main` branch is maintained. There are no released versions
with security support.

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

- Use GitHub's private vulnerability reporting:
  **Security → Report a vulnerability** on this repository.
- Include: affected file/contract, a description of the issue, steps or a
  test case to reproduce, and the impact you expect.

We aim to acknowledge reports within 7 days. This is a single-operator
project, so there is no bug bounty and no guaranteed fix timeline.

## Scope

In scope:

- contract code in `contracts/amm`, `contracts/nft`, `contracts/nft-market`;
- build and deploy scripts in `contracts/`;
- CI configuration in `.github/workflows/`.

Out of scope:

- the private operator infrastructure (nodes, relayer, mesh), which is not
  part of this repository and not publicly exposed;
- third-party dependencies (please report those upstream);
- findings that require already-compromised operator keys.

## Secrets

This repository must never contain keys, mnemonics, keyrings, validator
material, genesis files, tokens or join cards with identities. If you find
any such material here, report it privately as above.
