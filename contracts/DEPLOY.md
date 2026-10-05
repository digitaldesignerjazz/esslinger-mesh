# Deploy esslinger-mesh CW721 and NFT escrow

> **WARNUNG: Dieser Contract ist nicht auditiert.** Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst.
>
> **WARNING: These contracts are not audited.** Use at your own risk. There is no guarantee of fund or NFT safety.

This file is the operator runbook for **nexus-qcoin-1 only**. It does not contain keys, mnemonics, or a public RPC. The node address is whatever `wasmd` on the Hannover box already uses. A cloud agent cannot reach `127.0.0.1` on that box. Hannover-Box runs the commands below after this change is reviewed. Do not store or instantiate on `nexus-xcoin-1`, the harvester chains, or any other chain id.

`contracts/deploy-nft.sh` prints these commands. It broadcasts only when `DEPLOY_CONFIRM=yes`.

## Constants

| Item | Value |
|---|---|
| Chain | `nexus-qcoin-1` |
| wasmd | v0.61.15 (Cosmos SDK v0.53.6), address prefix `nexus` |
| Wasm admin (both contracts) | gov module, same role as the AMM |
| CW721 minter | same gov module, unless `MINTER` is set before instantiate |
| Marketplace `admin` | same gov module (pause, fee, collections) |
| `fee_bps` | `30` (0.3%). Taken from the seller's proceeds. The buyer pays the list price only. |
| `fee_recipient` | same gov module, unless `FEE_RECIPIENT` is set before instantiate |
| Price denoms | `aqcoin` and `ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C` |
| Collection | name `Mesh Badge v1`, symbol `MESH`. Cosmetic. No governance rights. |
| Labels | `esslinger-mesh-cw721`, `esslinger-mesh-nft-market` |
| Gas denom | `aqstake` |
| Gov module address | `nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq` |

The gov address is SDK 0.53 `NewModuleAddress("gov")` with no derivation key: `sha256("gov")[:20]`, bech32 prefix `nexus`. Confirm it on the box before any broadcast:

```bash
wasmd query auth module-account gov --node "$NODE" -o json
```

Use `.account.base_account.address`. If it is not `nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq`, stop. Do not instantiate with a guessed address.

## 0. Environment

Run on the box that already operates `nexus-qcoin-1`. Export only what that host already uses:

```bash
export NODE="${NODE:-tcp://127.0.0.1:26657}"
export CHAIN_ID=nexus-qcoin-1
export KEY="${KEY:?name of the local key that may store wasm}"
export KEYRING_BACKEND="${KEYRING_BACKEND:-os}"
export GAS_PRICES="${GAS_PRICES:-0.025aqstake}"
export GOV_ADDR=nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq
```

`GAS_PRICES` must match the node's `minimum-gas-prices`. `0.025aqstake` is a starting point, not a promise that the node accepts it. The signer needs `aqstake` for gas. This repository does not fund that account.

`deploy-nft.sh` refuses `CHAIN_ID` other than `nexus-qcoin-1`.

## 1. Build

From `contracts/`, with Rust stable (1.85 or newer; the market tests pull `prost` 0.14) and the wasm target:

```bash
rustup target add wasm32-unknown-unknown
./build-wasm.sh
```

That runs, in each crate directory so the release profile in that `Cargo.toml` applies (`panic = abort`, `overflow-checks = true`, LTO):

```bash
cargo build --release --target wasm32-unknown-unknown
```

Artifacts:

- `contracts/artifacts/esslinger_mesh_cw721.wasm`
- `contracts/artifacts/esslinger_mesh_nft_market.wasm`
- `contracts/artifacts/SHA256SUMS`

Optional smaller binary, same source, if Docker is available. The cargo build above is enough to store:

```bash
docker run --rm -v "$PWD/nft":/code \
  --mount type=volume,source=esslinger_nft_cache,target=/code/target \
  --mount type=volume,source=cosmwasm_registry,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
docker run --rm -v "$PWD/nft-market":/code \
  --mount type=volume,source=esslinger_market_cache,target=/code/target \
  --mount type=volume,source=cosmwasm_registry,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
```

Record the sha256 you actually upload. Do not store a wasm that was not built from this commit.

## 2. Store

`DEPLOYER` is the address of `$KEY`. Both that address and gov may instantiate. Migration admin is set later, on instantiate, and is gov only.

```bash
DEPLOYER="$(wasmd keys show "$KEY" -a --keyring-backend "$KEYRING_BACKEND")"

wasmd tx wasm store artifacts/esslinger_mesh_cw721.wasm \
  --from "$KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto \
  --gas-adjustment 1.4 \
  --gas-prices "$GAS_PRICES" \
  --instantiate-anyof-addresses "${GOV_ADDR},${DEPLOYER}" \
  --broadcast-mode sync \
  -y -o json

wasmd tx wasm store artifacts/esslinger_mesh_nft_market.wasm \
  --from "$KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto \
  --gas-adjustment 1.4 \
  --gas-prices "$GAS_PRICES" \
  --instantiate-anyof-addresses "${GOV_ADDR},${DEPLOYER}" \
  --broadcast-mode sync \
  -y -o json
```

Read each code id after the tx is in a block:

```bash
wasmd query tx "$TXHASH" --node "$NODE" -o json
```

`events[].type == "store_code"` and attribute `code_id`. Export them as `NFT_CODE_ID` and `MARKET_CODE_ID`.

Dry-run, no state change:

```bash
wasmd tx wasm store artifacts/esslinger_mesh_cw721.wasm \
  --from "$KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto \
  --gas-adjustment 1.4 \
  --gas-prices "$GAS_PRICES" \
  --instantiate-anyof-addresses "${GOV_ADDR},${DEPLOYER}" \
  --dry-run
```

Repeat for the market wasm.

## 3. Instantiate the collection

Wasm `--admin` is gov, so a later migration is a governance proposal. The signer of this tx is `$KEY`, not gov. `minter` in the JSON is also gov, so the first mint is a gov execute unless you change `MINTER` before you broadcast.

```bash
wasmd tx wasm instantiate "$NFT_CODE_ID" \
  '{"name":"Mesh Badge v1","symbol":"MESH","minter":"nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq"}' \
  --label esslinger-mesh-cw721 \
  --admin nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq \
  --from "$KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto \
  --gas-adjustment 1.4 \
  --gas-prices "$GAS_PRICES" \
  --broadcast-mode sync \
  -y -o json
```

From `wasmd query tx "$TXHASH"`, event `instantiate`, attribute `_contract_address`. Export it as `NFT`.

The instantiate response attributes include `not_audited=true` and the German warning from the README.

## 4. Instantiate the escrow

`fee_bps` is the number `30`, not a string. `nft_contracts` is the address from step 3. `admin` and `fee_recipient` are the gov module. `--admin` is the same address.

```bash
wasmd tx wasm instantiate "$MARKET_CODE_ID" \
  '{"admin":"nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq","fee_bps":30,"fee_recipient":"nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq","allowed_denoms":["aqcoin","ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C"],"nft_contracts":["'"$NFT"'"]}' \
  --label esslinger-mesh-nft-market \
  --admin nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq \
  --from "$KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto \
  --gas-adjustment 1.4 \
  --gas-prices "$GAS_PRICES" \
  --broadcast-mode sync \
  -y -o json
```

Export `_contract_address` as `MARKET`.

Dry-run the same command with `--dry-run` once `$NFT` and `$MARKET_CODE_ID` are known. Dry-run does not write code ids or addresses.

## 5. Read back

```bash
wasmd query wasm contract "$NFT" --node "$NODE" -o json
wasmd query wasm contract "$MARKET" --node "$NODE" -o json
wasmd query wasm contract-state smart "$NFT" '{"contract_info":{}}' --node "$NODE"
wasmd query wasm contract-state smart "$NFT" '{"minter":{}}' --node "$NODE"
wasmd query wasm contract-state smart "$NFT" '{"warning":{}}' --node "$NODE"
wasmd query wasm contract-state smart "$MARKET" '{"config":{}}' --node "$NODE"
wasmd query wasm contract-state smart "$MARKET" '{"warning":{}}' --node "$NODE"
```

Expect contract admin `nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq`, minter the same address, `fee_bps` 30, both denoms, `paused` false, and `not_audited` true. If any of those differ, do not mint.

## 6. Message templates

These are the flows. They move tokens. Run them only after step 5 looks right. Prices below are examples (`1000000` base units). The buyer attaches exactly that amount. The 0.3% fee is not added on top.

Minter is gov, so a mint from `$KEY` fails until governance executes it, or until gov has called `update_minter`.

```bash
# Mint
wasmd tx wasm execute "$NFT" \
  '{"mint":{"token_id":"badge-1","owner":"'"$OWNER"'","token_uri":"ipfs://REPLACE_WITH_PUBLIC_CID"}}' \
  --from "$MINTER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y

# Peer transfer
wasmd tx wasm execute "$NFT" \
  '{"transfer_nft":{"recipient":"'"$RECIPIENT"'","token_id":"badge-1"}}' \
  --from "$OWNER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y

# List via SendNft. Hook JSON, base64:
# {"list":{"price":{"denom":"aqcoin","amount":"1000000"}}}
# eyJsaXN0Ijp7InByaWNlIjp7ImRlbm9tIjoiYXFjb2luIiwiYW1vdW50IjoiMTAwMDAwMCJ9fX0=
wasmd tx wasm execute "$NFT" \
  '{"send_nft":{"contract":"'"$MARKET"'","token_id":"badge-1","msg":"eyJsaXN0Ijp7InByaWNlIjp7ImRlbm9tIjoiYXFjb2luIiwiYW1vdW50IjoiMTAwMDAwMCJ9fX0="}}' \
  --from "$OWNER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y

# List via approval, then List. Same price.
wasmd tx wasm execute "$NFT" \
  '{"approve":{"spender":"'"$MARKET"'","token_id":"badge-1","expires":{"never":{}}}}' \
  --from "$OWNER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y
wasmd tx wasm execute "$MARKET" \
  '{"list":{"nft_contract":"'"$NFT"'","token_id":"badge-1","price":{"denom":"aqcoin","amount":"1000000"}}}' \
  --from "$OWNER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y

# Buy. Funds are the list price, not price plus fee.
wasmd tx wasm execute "$MARKET" \
  '{"buy":{"nft_contract":"'"$NFT"'","token_id":"badge-1"}}' \
  --amount 1000000aqcoin \
  --from "$BUYER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y

# Cancel (seller). Also works while the market is paused.
wasmd tx wasm execute "$MARKET" \
  '{"cancel":{"nft_contract":"'"$NFT"'","token_id":"badge-1"}}' \
  --from "$OWNER_KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y
```

XCOIN IBC price uses denom `ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C` in the `price` object and in `--amount` (for example `--amount 1000000ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C`). A hook for amount `1000000` of that denom base64-encodes to:

```text
eyJsaXN0Ijp7InByaWNlIjp7ImRlbm9tIjoiaWJjLzI0OUIxQkE3RTI0ODY5NDY4MzYwMzE4N0Q0MzE3NEVBODBFQzI1QkExRTM2MTQ5RDc5ODAxMzNEQ0E5OUMyMkMiLCJhbW91bnQiOiIxMDAwMDAwIn19fQ==
```

Queries:

```bash
wasmd query wasm contract-state smart "$NFT" '{"owner_of":{"token_id":"badge-1"}}' --node "$NODE"
wasmd query wasm contract-state smart "$MARKET" '{"listing":{"nft_contract":"'"$NFT"'","token_id":"badge-1"}}' --node "$NODE"
```

## 7. If the node rejects a direct store

The chain may still allow wasm upload only from governance. A direct `tx wasm store` then fails with an unauthorized error. Do not retry in a loop. Submit the same wasm through governance and vote with the existing process. Deposit must be the chain's real min deposit. This repo does not set that number.

```bash
wasmd tx wasm submit-proposal store artifacts/esslinger_mesh_cw721.wasm \
  --title "Store esslinger-mesh-cw721" \
  --summary "CW721 collection for nexus-qcoin-1. Not audited." \
  --deposit "$DEPOSIT" \
  --instantiate-anyof-addresses "$GOV_ADDR" \
  --from "$KEY" \
  --keyring-backend "$KEYRING_BACKEND" \
  --chain-id nexus-qcoin-1 \
  --node "$NODE" \
  --gas auto --gas-adjustment 1.4 --gas-prices "$GAS_PRICES" \
  -y
```

Repeat for `esslinger_mesh_nft_market.wasm`. If instantiate is also gov-only, `wasmd tx wasm submit-proposal instantiate` carries the same JSON and `--admin` as sections 3 and 4. `wasmd tx wasm submit-proposal store -h` and `instantiate -h` on the box win if a flag name differs.

## 8. What this runbook does not do

- No deploy on `nexus-xcoin-1` or the harvester chains.
- No change to the AMM.
- No claim that an audit exists.
- No mint of a public supply. Amount and recipients stay a separate gov decision.
- No keys, vaults, or relayer material in this repository.
