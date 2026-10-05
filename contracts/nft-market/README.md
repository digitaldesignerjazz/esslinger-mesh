# esslinger-mesh-nft-market

> **WARNUNG: Dieser Contract ist nicht auditiert.** Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst.
>
> **WARNING: This contract is not audited.** Use at your own risk. There is no guarantee of fund or NFT safety.

Fixed-price escrow for one or more CW721 collections. Own code, Apache-2.0 (see `LICENSE`). The buyer pays the listing price. The fee (`fee_bps`, deploy value **30** = 0.3%) is taken out of the seller's proceeds. The contract does not keep a balance.

Deploy **only on `nexus-qcoin-1`**. Commands are in [`../DEPLOY.md`](../DEPLOY.md).

## Flows

1. **Mint** on the CW721 (minter). The owner holds the token.
2. **Transfer** with `transfer_nft` when no sale is wanted.
3. **List** either by:
   - `send_nft` to this market with msg `{"list":{"price":{"denom":"aqcoin","amount":"1000000"}}}` (or the XCOIN IBC denom), or
   - `approve` this market, then `list` `{ nft_contract, token_id, price }`.
4. **Buy** with funds equal to the listing price, same denom. Seller receives `price - fee`. `fee_recipient` receives the fee. The NFT is transferred to the buyer. The listing is deleted before those messages; if the transfer fails, the whole transaction rolls back.
5. **Cancel** by the seller. Allowed while the market is paused. The NFT returns to the seller.

`list` reads `owner_of` and rejects any caller who is not the owner, so a third party cannot hijack an approval and name themselves seller.

Allowed price denoms are set at instantiate. Anything else returns `unexpected denom`. v1 deploy allows `aqcoin` and `ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C`.

## Admin

The instantiate field `admin` changes `fee_bps`, `fee_recipient`, allowed denoms, and the collection list, and can `pause` / `resume`. Deploy sets that address to the nexus-qcoin-1 gov module, same as the wasm `--admin` used for migration. Pause blocks list and buy. Cancel still works. `rescue` is gov-only and only for an NFT that was transferred here **without** a listing. It refuses while a listing exists.

## Not audited

Instantiate writes `not_audited=true` and the German warning above. `{"config":{}}` and `{"warning":{}}` repeat it. There is no audit report. Do not describe this market as audited in a UI.
