#!/usr/bin/env bash
# Print or broadcast store + instantiate for esslinger-mesh-cw721 and
# esslinger-mesh-nft-market. nexus-qcoin-1 only.
#
# DEPLOY_CONFIRM unset or "print" : print commands, do not broadcast.
# DEPLOY_CONFIRM=dry-run           : pass --dry-run to wasmd.
# DEPLOY_CONFIRM=yes               : broadcast. Requires wasmd, NODE, KEY.
#
# This script does not contain a mnemonic. It refuses every chain id except
# nexus-qcoin-1.
set -euo pipefail

CHAIN_ID="${CHAIN_ID:-nexus-qcoin-1}"
NODE="${NODE:-tcp://127.0.0.1:26657}"
KEYRING_BACKEND="${KEYRING_BACKEND:-os}"
GAS_PRICES="${GAS_PRICES:-0.025aqstake}"
GAS_ADJUSTMENT="${GAS_ADJUSTMENT:-1.4}"
GOV_EXPECTED="nexus10d07y265gmmuvt4z0w9aw880jnsr700j6rnthq"
GOV_ADDR="${GOV_ADDR:-$GOV_EXPECTED}"
FEE_BPS="30"
XCOIN="ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C"
CONFIRM="${DEPLOY_CONFIRM:-print}"

if [[ "$CHAIN_ID" != "nexus-qcoin-1" ]]; then
  echo "refusing: v1 deploys only on nexus-qcoin-1 (got $CHAIN_ID)" >&2
  exit 1
fi
if [[ ! "$GOV_ADDR" =~ ^nexus1[0-9a-z]+$ ]]; then
  echo "refusing: gov address is not a nexus bech32 account" >&2
  exit 1
fi
if [[ "$GOV_ADDR" != "$GOV_EXPECTED" && -z "${GOV_ADDR_CONFIRMED:-}" ]]; then
  echo "refusing: GOV_ADDR differs from the SDK 0.53 gov module address." >&2
  echo "query 'wasmd query auth module-account gov' and set GOV_ADDR_CONFIRMED=yes only if that query matches." >&2
  exit 1
fi

root="$(cd "$(dirname "$0")" && pwd)"
NFT_WASM="${NFT_WASM:-$root/artifacts/esslinger_mesh_cw721.wasm}"
MARKET_WASM="${MARKET_WASM:-$root/artifacts/esslinger_mesh_nft_market.wasm}"

common_flags=(
  --chain-id "$CHAIN_ID"
  --node "$NODE"
  --gas auto
  --gas-adjustment "$GAS_ADJUSTMENT"
  --gas-prices "$GAS_PRICES"
)
if [[ -n "${WASMD_HOME:-}" ]]; then
  common_flags+=(--home "$WASMD_HOME")
fi

nft_init='{"name":"Mesh Badge v1","symbol":"MESH","minter":"'"$GOV_ADDR"'"}'

echo "chain: $CHAIN_ID"
echo "gov:   $GOV_ADDR"
echo "fee:   ${FEE_BPS} bps, recipient ${FEE_RECIPIENT:-$GOV_ADDR}"
echo "denoms: aqcoin $XCOIN"
echo "not audited. DEPLOY_CONFIRM=$CONFIRM"
echo

if [[ "$CONFIRM" == "print" ]]; then
  fee_to="${FEE_RECIPIENT:-$GOV_ADDR}"
  cat <<EOF
# No broadcast. On the box: DEPLOY_CONFIRM=dry-run or DEPLOY_CONFIRM=yes.
# Build first: $root/build-wasm.sh
# export KEY=...
# export DEPLOYER=\$(wasmd keys show "\$KEY" -a --keyring-backend $KEYRING_BACKEND)

wasmd tx wasm store $NFT_WASM \\
  --from "\$KEY" --keyring-backend $KEYRING_BACKEND \\
  --chain-id $CHAIN_ID --node $NODE \\
  --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES \\
  --instantiate-anyof-addresses ${GOV_ADDR},\$DEPLOYER \\
  --broadcast-mode sync -y -o json

wasmd tx wasm store $MARKET_WASM \\
  --from "\$KEY" --keyring-backend $KEYRING_BACKEND \\
  --chain-id $CHAIN_ID --node $NODE \\
  --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES \\
  --instantiate-anyof-addresses ${GOV_ADDR},\$DEPLOYER \\
  --broadcast-mode sync -y -o json

wasmd tx wasm instantiate \$NFT_CODE_ID \\
  '$nft_init' \\
  --label esslinger-mesh-cw721 --admin $GOV_ADDR \\
  --from "\$KEY" --keyring-backend $KEYRING_BACKEND \\
  --chain-id $CHAIN_ID --node $NODE \\
  --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES \\
  --broadcast-mode sync -y -o json

wasmd tx wasm instantiate \$MARKET_CODE_ID \\
  '{"admin":"$GOV_ADDR","fee_bps":$FEE_BPS,"fee_recipient":"$fee_to","allowed_denoms":["aqcoin","$XCOIN"],"nft_contracts":["'"\$NFT"'"]}' \\
  --label esslinger-mesh-nft-market --admin $GOV_ADDR \\
  --from "\$KEY" --keyring-backend $KEYRING_BACKEND \\
  --chain-id $CHAIN_ID --node $NODE \\
  --gas auto --gas-adjustment $GAS_ADJUSTMENT --gas-prices $GAS_PRICES \\
  --broadcast-mode sync -y -o json
EOF
  exit 0
fi

if [[ "$CONFIRM" != "yes" && "$CONFIRM" != "dry-run" ]]; then
  echo "refusing: DEPLOY_CONFIRM must be print, dry-run, or yes" >&2
  exit 1
fi

: "${KEY:?set KEY to the local key name}"
command -v wasmd >/dev/null
command -v jq >/dev/null
[[ -f "$NFT_WASM" && -f "$MARKET_WASM" ]] || { echo "missing wasm. Run build-wasm.sh" >&2; exit 1; }

status_json="$(wasmd status --node "$NODE" 2>/dev/null || true)"
live_chain="$(printf '%s' "$status_json" | jq -r '.node_info.network // .NodeInfo.network // .default_node_info.network // empty' 2>/dev/null || true)"
if [[ -z "$live_chain" ]]; then
  live_chain="$(wasmd query block --node "$NODE" -o json | jq -r '.block.header.chain_id // .sdk_block.header.chain_id // empty')"
fi
if [[ "$live_chain" != "nexus-qcoin-1" ]]; then
  echo "refusing: node chain id is '$live_chain', want nexus-qcoin-1" >&2
  exit 1
fi

gov_json="$(wasmd query auth module-account gov --node "$NODE" -o json)"
gov_live="$(printf '%s' "$gov_json" | jq -r '.account.base_account.address // .account.value.address // .account.address // empty')"
if [[ "$gov_live" != "$GOV_ADDR" ]]; then
  echo "refusing: on-chain gov module is '$gov_live', script expects '$GOV_ADDR'" >&2
  exit 1
fi

DEPLOYER="$(wasmd keys show "$KEY" -a --keyring-backend "$KEYRING_BACKEND")"
dry=()
if [[ "$CONFIRM" == "dry-run" ]]; then
  dry=(--dry-run)
fi

tx() {
  wasmd tx "$@" "${common_flags[@]}" --from "$KEY" --keyring-backend "$KEYRING_BACKEND" "${dry[@]}" -y -o json
}

echo "sha256:"
sha256sum "$NFT_WASM" "$MARKET_WASM"

store_one() {
  local wasm="$1"
  local out
  out="$(tx wasm store "$wasm" --instantiate-anyof-addresses "${GOV_ADDR},${DEPLOYER}" --broadcast-mode sync)"
  printf '%s\n' "$out" >&2
  if [[ "$CONFIRM" == "dry-run" ]]; then
    printf '%s\n' "dry-run"
    return 0
  fi
  local hash code
  hash="$(printf '%s' "$out" | jq -r '.txhash // .tx_response.txhash')"
  local i=0
  local queried=""
  while [[ "$i" -lt 15 ]]; do
    if queried="$(wasmd query tx "$hash" --node "$NODE" -o json 2>/dev/null)"; then
      code="$(printf '%s' "$queried" | jq -r '.code // .tx_response.code // 0')"
      if [[ "$code" != "0" ]]; then
        echo "tx $hash failed: $queried" >&2
        exit 1
      fi
      printf '%s\n' "$queried" | jq -r '[.events[]?, .tx_response.events[]? | select(.type=="store_code") | .attributes[]? | select(.key=="code_id") | .value] | first // empty'
      return 0
    fi
    i=$((i + 1))
    sleep 2
  done
  echo "timed out waiting for $hash" >&2
  exit 1
}

echo "storing cw721"
NFT_CODE_ID="$(store_one "$NFT_WASM" | tail -n 1)"
echo "NFT_CODE_ID=$NFT_CODE_ID"
echo "storing market"
MARKET_CODE_ID="$(store_one "$MARKET_WASM" | tail -n 1)"
echo "MARKET_CODE_ID=$MARKET_CODE_ID"

if [[ "$CONFIRM" == "dry-run" ]]; then
  echo "dry-run store finished. Instantiate was not sent because code ids are not persisted."
  exit 0
fi

instantiate_one() {
  local code="$1"
  local msg="$2"
  local label="$3"
  local out hash queried code_rc addr i
  out="$(tx wasm instantiate "$code" "$msg" --label "$label" --admin "$GOV_ADDR" --broadcast-mode sync)"
  printf '%s\n' "$out" >&2
  hash="$(printf '%s' "$out" | jq -r '.txhash // .tx_response.txhash')"
  i=0
  while [[ "$i" -lt 15 ]]; do
    if queried="$(wasmd query tx "$hash" --node "$NODE" -o json 2>/dev/null)"; then
      code_rc="$(printf '%s' "$queried" | jq -r '.code // .tx_response.code // 0')"
      if [[ "$code_rc" != "0" ]]; then
        echo "instantiate $label failed: $queried" >&2
        exit 1
      fi
      addr="$(printf '%s' "$queried" | jq -r '[.events[]?, .tx_response.events[]? | select(.type=="instantiate") | .attributes[]? | select(.key=="_contract_address") | .value] | first // empty')"
      if [[ -n "$addr" ]]; then
        printf '%s\n' "$addr"
        return 0
      fi
    fi
    i=$((i + 1))
    sleep 2
  done
  echo "timed out waiting for instantiate $hash" >&2
  exit 1
}

FEE_TO="${FEE_RECIPIENT:-$GOV_ADDR}"
MINTER="${MINTER:-$GOV_ADDR}"
nft_init='{"name":"Mesh Badge v1","symbol":"MESH","minter":"'"$MINTER"'"}'
echo "instantiating cw721"
NFT="$(instantiate_one "$NFT_CODE_ID" "$nft_init" "esslinger-mesh-cw721")"
echo "NFT=$NFT"
market_init="$(jq -nc \
  --arg admin "$GOV_ADDR" \
  --arg fee "$FEE_TO" \
  --arg nft "$NFT" \
  --arg xcoin "$XCOIN" \
  --argjson fee_bps "$FEE_BPS" \
  '{admin:$admin,fee_bps:$fee_bps,fee_recipient:$fee,allowed_denoms:["aqcoin",$xcoin],nft_contracts:[$nft]}')"
echo "instantiating market $market_init"
MARKET="$(instantiate_one "$MARKET_CODE_ID" "$market_init" "esslinger-mesh-nft-market")"
echo "MARKET=$MARKET"
echo "not audited. Read back with the queries in DEPLOY.md before any mint."
