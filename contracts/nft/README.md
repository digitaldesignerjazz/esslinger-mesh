# esslinger-mesh-cw721

> **WARNUNG: Dieser Contract ist nicht auditiert.** Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst.
>
> **WARNING: This contract is not audited.** Use at your own risk. There is no guarantee of fund or NFT safety.

CW721-compatible collection for Esslinger Cyberspace (formerly Esslinger Mesh and Esslinger Net). Own code, Apache-2.0 (see `LICENSE`). Message names follow the CW721 JSON API (`mint`, `transfer_nft`, `send_nft`, `approve`, `owner_of`, `nft_info`, …). It is not a byte-for-byte upstream `cw721-base` build.

Deploy **only on `nexus-qcoin-1`**. Store and instantiate commands are in [`../DEPLOY.md`](../DEPLOY.md).

## What a badge is

`Mesh Badge v1` / `MESH` is a cosmetic peer emblem. Holding one does **not** grant governance rights, votes, or admin power.

`token_uri` is a public HTTPS or IPFS link to JSON (name, description, image). Do not put secrets in it.

## Roles

| Role | Who | Can |
|---|---|---|
| Wasm admin | gov module of nexus-qcoin-1 (`--admin` at instantiate) | migrate, via governance |
| Minter | instantiate `minter` (deploy default: the same gov module) | `mint`, `update_minter` |
| Owner | any address | `transfer_nft`, `send_nft`, `approve`, `burn` |
| Approved spender / operator | an address the owner approved | `transfer_nft`, `send_nft`, `burn` |

`send_nft` puts the **owner** in the hook field `sender`, not the operator. An operator can move the token, but a marketplace that trusts this hook pays the owner.

Approvals are cleared on transfer, send, and burn. Bank coins sent with these messages are rejected so they cannot get stuck.

## Not audited

Instantiate writes attributes `not_audited=true` and `warning` set to the German sentence above. Query `{"warning":{}}` returns the same sentence.
