//! esslinger-mesh-nft-market: fixed-price CW721 escrow.
//! Own code, Apache-2.0. Separate from the NFT collection, same shape as the AMM:
//! minimal messages, fee_bps taken inside the sale, unexpected denoms rejected.
//!
//! The buyer pays the listing price. The fee is deducted from the seller's proceeds
//! (fee_bps = 30 is 0.3%). The contract does not keep a balance.
//!
//! NOT AUDITED. Use at your own risk.
//!
//! List paths:
//! 1. Owner SendNft to this contract with msg `{"list":{"price":{"denom","amount"}}}`.
//! 2. Owner Approve this contract, then List { nft_contract, token_id, price }.
//!    List checks OwnerOf so an operator approval cannot name a third party as seller.
//!
//! Bank and wasm messages are returned after storage updates. A failed transfer
//! rolls the whole transaction back, including the listing and any payout.
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, Uint256, WasmMsg,
};
use cw_storage_plus::{Bound, Item, Map};

pub const NOT_AUDITED_WARNING: &str = "Dieser Contract ist nicht auditiert. Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst.";

const MAX_TOKEN_ID_LEN: usize = 128;
const MAX_COLLECTIONS: usize = 32;
const MAX_DENOMS: usize = 8;
const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

#[cw_serde]
pub struct InstantiateMsg {
    /// Parameter admin (pause, fee, collections). Deploy sets the gov module.
    /// Wasm migration admin is the chain `--admin` flag and should be the same account.
    pub admin: String,
    pub fee_bps: u16,
    pub fee_recipient: String,
    pub allowed_denoms: Vec<String>,
    pub nft_contracts: Vec<String>,
}

#[cw_serde]
pub struct MigrateMsg {}

/// Payload inside CW721 SendNft.msg.
#[cw_serde]
pub enum HookMsg {
    List { price: Coin },
}

#[cw_serde]
pub struct Cw721ReceiveMsg {
    pub sender: String,
    pub token_id: String,
    pub msg: Binary,
}

#[cw_serde]
pub enum ExecuteMsg {
    ReceiveNft(Cw721ReceiveMsg),
    List {
        nft_contract: String,
        token_id: String,
        price: Coin,
    },
    Buy {
        nft_contract: String,
        token_id: String,
    },
    Cancel {
        nft_contract: String,
        token_id: String,
    },
    Pause {},
    Resume {},
    UpdateConfig {
        fee_bps: Option<u16>,
        fee_recipient: Option<String>,
        allowed_denoms: Option<Vec<String>>,
    },
    AddCollection {
        nft_contract: String,
    },
    RemoveCollection {
        nft_contract: String,
    },
    /// Gov recovery for an NFT sent here without a listing. Refuses while a listing exists.
    Rescue {
        nft_contract: String,
        token_id: String,
        recipient: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(ListingResponse)]
    Listing {
        nft_contract: String,
        token_id: String,
    },
    #[returns(ListingsResponse)]
    Listings {
        nft_contract: Option<String>,
        start_after_token: Option<String>,
        limit: Option<u32>,
    },
    #[returns(WarningResponse)]
    Warning {},
}

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    pub fee_bps: u16,
    pub fee_recipient: Addr,
    pub allowed_denoms: Vec<String>,
    pub nft_contracts: Vec<Addr>,
    pub paused: bool,
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub fee_bps: u16,
    pub fee_recipient: String,
    pub allowed_denoms: Vec<String>,
    pub nft_contracts: Vec<String>,
    pub paused: bool,
    pub not_audited: bool,
    pub warning: String,
}

#[cw_serde]
pub struct Listing {
    pub seller: Addr,
    pub price: Coin,
}

#[cw_serde]
pub struct ListingInfo {
    pub nft_contract: String,
    pub token_id: String,
    pub seller: String,
    pub price: Coin,
}

#[cw_serde]
pub struct ListingResponse {
    pub listing: Option<ListingInfo>,
}

#[cw_serde]
pub struct ListingsResponse {
    pub listings: Vec<ListingInfo>,
}

#[cw_serde]
pub struct WarningResponse {
    pub not_audited: bool,
    pub warning: String,
}

#[cw_serde]
enum NftExecute {
    TransferNft { recipient: String, token_id: String },
}

#[cw_serde]
enum NftQuery {
    OwnerOf {
        token_id: String,
        include_expired: Option<bool>,
    },
}

#[cw_serde]
struct OwnerOfResp {
    owner: String,
}

const CONFIG: Item<Config> = Item::new("config");
const LISTINGS: Map<(&Addr, &str), Listing> = Map::new("listings");

fn err(msg: &str) -> StdError {
    StdError::generic_err(msg)
}

fn no_funds(info: &MessageInfo) -> StdResult<()> {
    if info.funds.is_empty() {
        Ok(())
    } else {
        Err(err("no funds expected"))
    }
}

fn check_token_id(token_id: &str) -> StdResult<()> {
    if token_id.is_empty() || token_id.len() > MAX_TOKEN_ID_LEN || token_id.trim().is_empty() {
        return Err(err("invalid token_id"));
    }
    Ok(())
}

fn check_fee(fee_bps: u16) -> StdResult<()> {
    if fee_bps >= 10_000 {
        Err(err("fee_bps must be < 10000"))
    } else {
        Ok(())
    }
}

fn valid_denom(denom: &str) -> bool {
    !denom.is_empty()
        && denom.len() <= 128
        && denom
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_'))
}

fn normalize_denoms(denoms: &[String]) -> StdResult<Vec<String>> {
    if denoms.is_empty() || denoms.len() > MAX_DENOMS {
        return Err(err("invalid denoms"));
    }
    let mut out = Vec::with_capacity(denoms.len());
    for denom in denoms {
        if !valid_denom(denom) || out.iter().any(|d: &String| d == denom) {
            return Err(err("invalid denoms"));
        }
        out.push(denom.clone());
    }
    Ok(out)
}

fn only_admin(cfg: &Config, info: &MessageInfo) -> StdResult<()> {
    if info.sender == cfg.admin {
        Ok(())
    } else {
        Err(err("not admin"))
    }
}

fn ensure_collection(cfg: &Config, nft: &Addr) -> StdResult<()> {
    if cfg.nft_contracts.iter().any(|c| c == nft) {
        Ok(())
    } else {
        Err(err("collection not allowed"))
    }
}

fn not_paused(cfg: &Config) -> StdResult<()> {
    if cfg.paused {
        Err(err("paused"))
    } else {
        Ok(())
    }
}

fn check_price(cfg: &Config, price: &Coin) -> StdResult<()> {
    if price.amount.is_zero() {
        return Err(err("zero price"));
    }
    if !cfg.allowed_denoms.iter().any(|d| d == &price.denom) || !valid_denom(&price.denom) {
        return Err(err("unexpected denom"));
    }
    Ok(())
}

/// Fee is rounded down. Seller receives `price - fee`, so the contract keeps nothing.
pub fn split_fee(price: Uint128, fee_bps: u16) -> StdResult<(Uint128, Uint128)> {
    check_fee(fee_bps)?;
    let fee256 =
        (Uint256::from(price) * Uint256::from(u128::from(fee_bps))) / Uint256::from(10_000u128);
    let fee = Uint128::try_from(fee256).map_err(|_| err("fee overflow"))?;
    let seller = price
        .checked_sub(fee)
        .map_err(|_| err("fee exceeds price"))?;
    Ok((seller, fee))
}

fn transfer_nft(nft: &Addr, recipient: impl Into<String>, token_id: &str) -> StdResult<WasmMsg> {
    Ok(WasmMsg::Execute {
        contract_addr: nft.to_string(),
        msg: to_json_binary(&NftExecute::TransferNft {
            recipient: recipient.into(),
            token_id: token_id.to_string(),
        })?,
        funds: vec![],
    })
}

fn pay(to: &Addr, denom: String, amount: Uint128) -> Option<BankMsg> {
    if amount.is_zero() {
        None
    } else {
        Some(BankMsg::Send {
            to_address: to.to_string(),
            amount: vec![Coin { denom, amount }],
        })
    }
}

fn listing_info(nft: &Addr, token_id: &str, listing: Listing) -> ListingInfo {
    ListingInfo {
        nft_contract: nft.to_string(),
        token_id: token_id.to_string(),
        seller: listing.seller.to_string(),
        price: listing.price,
    }
}

fn validate_addr_not_self(deps: Deps, env: &Env, addr: &str, self_err: &str) -> StdResult<Addr> {
    let validated = deps.api.addr_validate(addr)?;
    if validated == env.contract.address {
        return Err(err(self_err));
    }
    Ok(validated)
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    check_fee(msg.fee_bps)?;
    let admin = validate_addr_not_self(deps.as_ref(), &env, &msg.admin, "admin is the contract")?;
    let fee_recipient = validate_addr_not_self(
        deps.as_ref(),
        &env,
        &msg.fee_recipient,
        "fee recipient is the contract",
    )?;
    let allowed_denoms = normalize_denoms(&msg.allowed_denoms)?;
    if msg.nft_contracts.len() > MAX_COLLECTIONS {
        return Err(err("too many collections"));
    }
    let mut nft_contracts = Vec::with_capacity(msg.nft_contracts.len());
    for raw in &msg.nft_contracts {
        let addr = deps.api.addr_validate(raw)?;
        if addr == env.contract.address || nft_contracts.contains(&addr) {
            return Err(err("invalid collection"));
        }
        nft_contracts.push(addr);
    }
    CONFIG.save(
        deps.storage,
        &Config {
            admin,
            fee_bps: msg.fee_bps,
            fee_recipient,
            allowed_denoms,
            nft_contracts,
            paused: false,
        },
    )?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", msg.admin)
        .add_attribute("fee_bps", msg.fee_bps.to_string())
        .add_attribute("fee_recipient", msg.fee_recipient)
        .add_attribute("not_audited", "true")
        .add_attribute("warning", NOT_AUDITED_WARNING))
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("not_audited", "true")
        .add_attribute("warning", NOT_AUDITED_WARNING))
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::ReceiveNft(rcv) => {
            no_funds(&info)?;
            execute_receive(deps, env, info, rcv)
        }
        ExecuteMsg::List {
            nft_contract,
            token_id,
            price,
        } => {
            no_funds(&info)?;
            execute_list(deps, env, info, nft_contract, token_id, price)
        }
        ExecuteMsg::Buy {
            nft_contract,
            token_id,
        } => execute_buy(deps, env, info, nft_contract, token_id),
        ExecuteMsg::Cancel {
            nft_contract,
            token_id,
        } => {
            no_funds(&info)?;
            execute_cancel(deps, info, nft_contract, token_id)
        }
        ExecuteMsg::Pause {} => {
            no_funds(&info)?;
            set_paused(deps, info, true)
        }
        ExecuteMsg::Resume {} => {
            no_funds(&info)?;
            set_paused(deps, info, false)
        }
        ExecuteMsg::UpdateConfig {
            fee_bps,
            fee_recipient,
            allowed_denoms,
        } => {
            no_funds(&info)?;
            execute_update_config(deps, env, info, fee_bps, fee_recipient, allowed_denoms)
        }
        ExecuteMsg::AddCollection { nft_contract } => {
            no_funds(&info)?;
            execute_add_collection(deps, env, info, nft_contract)
        }
        ExecuteMsg::RemoveCollection { nft_contract } => {
            no_funds(&info)?;
            execute_remove_collection(deps, info, nft_contract)
        }
        ExecuteMsg::Rescue {
            nft_contract,
            token_id,
            recipient,
        } => {
            no_funds(&info)?;
            execute_rescue(deps, info, nft_contract, token_id, recipient)
        }
    }
}

fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    rcv: Cw721ReceiveMsg,
) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    not_paused(&cfg)?;
    ensure_collection(&cfg, &info.sender)?;
    check_token_id(&rcv.token_id)?;
    let HookMsg::List { price } = from_json(&rcv.msg).map_err(|_| err("invalid hook msg"))?;
    check_price(&cfg, &price)?;
    let seller = deps.api.addr_validate(&rcv.sender)?;
    if seller == env.contract.address {
        return Err(err("invalid seller"));
    }
    if LISTINGS
        .may_load(deps.storage, (&info.sender, &rcv.token_id))?
        .is_some()
    {
        return Err(err("already listed"));
    }
    LISTINGS.save(
        deps.storage,
        (&info.sender, &rcv.token_id),
        &Listing {
            seller: seller.clone(),
            price: price.clone(),
        },
    )?;
    Ok(Response::new()
        .add_attribute("action", "list")
        .add_attribute("seller", seller)
        .add_attribute("nft_contract", info.sender)
        .add_attribute("token_id", rcv.token_id)
        .add_attribute("price_denom", price.denom)
        .add_attribute("price_amount", price.amount))
}

fn execute_list(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nft_contract: String,
    token_id: String,
    price: Coin,
) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    not_paused(&cfg)?;
    check_token_id(&token_id)?;
    check_price(&cfg, &price)?;
    let nft = deps.api.addr_validate(&nft_contract)?;
    ensure_collection(&cfg, &nft)?;
    if LISTINGS
        .may_load(deps.storage, (&nft, &token_id))?
        .is_some()
    {
        return Err(err("already listed"));
    }
    let owner: OwnerOfResp = deps.querier.query_wasm_smart(
        nft.to_string(),
        &NftQuery::OwnerOf {
            token_id: token_id.clone(),
            include_expired: Some(false),
        },
    )?;
    let owner = deps.api.addr_validate(&owner.owner)?;
    if owner != info.sender {
        return Err(err("only owner can list"));
    }
    LISTINGS.save(
        deps.storage,
        (&nft, &token_id),
        &Listing {
            seller: info.sender.clone(),
            price: price.clone(),
        },
    )?;
    let pull = transfer_nft(&nft, env.contract.address.to_string(), &token_id)?;
    Ok(Response::new()
        .add_message(pull)
        .add_attribute("action", "list")
        .add_attribute("seller", info.sender)
        .add_attribute("nft_contract", nft)
        .add_attribute("token_id", token_id)
        .add_attribute("price_denom", price.denom)
        .add_attribute("price_amount", price.amount))
}

fn execute_buy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nft_contract: String,
    token_id: String,
) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    not_paused(&cfg)?;
    check_token_id(&token_id)?;
    let nft = deps.api.addr_validate(&nft_contract)?;
    let listing = LISTINGS
        .may_load(deps.storage, (&nft, &token_id))?
        .ok_or_else(|| err("listing not found"))?;
    if info.sender == listing.seller {
        return Err(err("seller cannot buy"));
    }
    check_price(&cfg, &listing.price)?;
    check_payment(&info, &listing.price)?;
    let (seller_amt, fee) = split_fee(listing.price.amount, cfg.fee_bps)?;
    LISTINGS.remove(deps.storage, (&nft, &token_id));
    let mut resp = Response::new();
    if let Some(msg) = pay(&listing.seller, listing.price.denom.clone(), seller_amt) {
        resp = resp.add_message(msg);
    }
    if let Some(msg) = pay(&cfg.fee_recipient, listing.price.denom.clone(), fee) {
        resp = resp.add_message(msg);
    }
    resp = resp.add_message(transfer_nft(&nft, info.sender.to_string(), &token_id)?);
    Ok(resp
        .add_attribute("action", "buy")
        .add_attribute("buyer", info.sender)
        .add_attribute("seller", listing.seller)
        .add_attribute("nft_contract", nft)
        .add_attribute("token_id", token_id)
        .add_attribute("price_denom", listing.price.denom)
        .add_attribute("price_amount", listing.price.amount)
        .add_attribute("seller_proceeds", seller_amt)
        .add_attribute("fee", fee)
        .add_attribute("escrow", env.contract.address))
}

fn check_payment(info: &MessageInfo, price: &Coin) -> StdResult<()> {
    if info.funds.is_empty() {
        return Err(err("send exactly the listing price"));
    }
    let mut sum = Uint128::zero();
    for coin in &info.funds {
        if coin.denom != price.denom {
            return Err(err("unexpected denom"));
        }
        sum = sum
            .checked_add(coin.amount)
            .map_err(|_| err("payment overflow"))?;
    }
    if sum != price.amount {
        return Err(err("wrong payment"));
    }
    Ok(())
}

fn execute_cancel(
    deps: DepsMut,
    info: MessageInfo,
    nft_contract: String,
    token_id: String,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    let nft = deps.api.addr_validate(&nft_contract)?;
    let listing = LISTINGS
        .may_load(deps.storage, (&nft, &token_id))?
        .ok_or_else(|| err("listing not found"))?;
    if listing.seller != info.sender {
        return Err(err("only seller can cancel"));
    }
    LISTINGS.remove(deps.storage, (&nft, &token_id));
    Ok(Response::new()
        .add_message(transfer_nft(&nft, listing.seller.to_string(), &token_id)?)
        .add_attribute("action", "cancel")
        .add_attribute("seller", info.sender)
        .add_attribute("nft_contract", nft)
        .add_attribute("token_id", token_id))
}

fn set_paused(deps: DepsMut, info: MessageInfo, paused: bool) -> StdResult<Response> {
    let mut cfg = CONFIG.load(deps.storage)?;
    only_admin(&cfg, &info)?;
    cfg.paused = paused;
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", if paused { "pause" } else { "resume" })
        .add_attribute("paused", paused.to_string()))
}

fn execute_update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    fee_bps: Option<u16>,
    fee_recipient: Option<String>,
    allowed_denoms: Option<Vec<String>>,
) -> StdResult<Response> {
    let mut cfg = CONFIG.load(deps.storage)?;
    only_admin(&cfg, &info)?;
    if let Some(bps) = fee_bps {
        check_fee(bps)?;
        cfg.fee_bps = bps;
    }
    if let Some(recipient) = fee_recipient {
        cfg.fee_recipient = validate_addr_not_self(
            deps.as_ref(),
            &env,
            &recipient,
            "fee recipient is the contract",
        )?;
    }
    if let Some(denoms) = allowed_denoms {
        cfg.allowed_denoms = normalize_denoms(&denoms)?;
    }
    let fee_bps_out = cfg.fee_bps.to_string();
    let fee_recipient_out = cfg.fee_recipient.to_string();
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("fee_bps", fee_bps_out)
        .add_attribute("fee_recipient", fee_recipient_out))
}

fn execute_add_collection(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nft_contract: String,
) -> StdResult<Response> {
    let mut cfg = CONFIG.load(deps.storage)?;
    only_admin(&cfg, &info)?;
    let addr = deps.api.addr_validate(&nft_contract)?;
    if addr == env.contract.address {
        return Err(err("invalid collection"));
    }
    if cfg.nft_contracts.contains(&addr) {
        return Err(err("collection exists"));
    }
    if cfg.nft_contracts.len() >= MAX_COLLECTIONS {
        return Err(err("too many collections"));
    }
    cfg.nft_contracts.push(addr);
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", "add_collection")
        .add_attribute("nft_contract", nft_contract))
}

fn execute_remove_collection(
    deps: DepsMut,
    info: MessageInfo,
    nft_contract: String,
) -> StdResult<Response> {
    let mut cfg = CONFIG.load(deps.storage)?;
    only_admin(&cfg, &info)?;
    let addr = deps.api.addr_validate(&nft_contract)?;
    if !cfg.nft_contracts.contains(&addr) {
        return Err(err("collection not allowed"));
    }
    let still = LISTINGS
        .prefix(&addr)
        .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .next()
        .transpose()?;
    if still.is_some() {
        return Err(err("collection has listings"));
    }
    cfg.nft_contracts.retain(|c| *c != addr);
    CONFIG.save(deps.storage, &cfg)?;
    Ok(Response::new()
        .add_attribute("action", "remove_collection")
        .add_attribute("nft_contract", nft_contract))
}

fn execute_rescue(
    deps: DepsMut,
    info: MessageInfo,
    nft_contract: String,
    token_id: String,
    recipient: String,
) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    only_admin(&cfg, &info)?;
    check_token_id(&token_id)?;
    let nft = deps.api.addr_validate(&nft_contract)?;
    if LISTINGS
        .may_load(deps.storage, (&nft, &token_id))?
        .is_some()
    {
        return Err(err("listing exists"));
    }
    let recipient_addr = deps.api.addr_validate(&recipient)?;
    Ok(Response::new()
        .add_message(transfer_nft(&nft, recipient_addr.to_string(), &token_id)?)
        .add_attribute("action", "rescue")
        .add_attribute("nft_contract", nft)
        .add_attribute("token_id", token_id)
        .add_attribute("recipient", recipient))
}

fn page_limit(limit: Option<u32>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let cfg = CONFIG.load(deps.storage)?;
            to_json_binary(&ConfigResponse {
                admin: cfg.admin.to_string(),
                fee_bps: cfg.fee_bps,
                fee_recipient: cfg.fee_recipient.to_string(),
                allowed_denoms: cfg.allowed_denoms,
                nft_contracts: cfg.nft_contracts.iter().map(|a| a.to_string()).collect(),
                paused: cfg.paused,
                not_audited: true,
                warning: NOT_AUDITED_WARNING.to_string(),
            })
        }
        QueryMsg::Listing {
            nft_contract,
            token_id,
        } => {
            let nft = deps.api.addr_validate(&nft_contract)?;
            let listing = if check_token_id(&token_id).is_ok() {
                LISTINGS.may_load(deps.storage, (&nft, &token_id))?
            } else {
                None
            };
            to_json_binary(&ListingResponse {
                listing: listing.map(|l| listing_info(&nft, &token_id, l)),
            })
        }
        QueryMsg::Listings {
            nft_contract,
            start_after_token,
            limit,
        } => {
            let limit = page_limit(limit);
            if start_after_token.is_some() && nft_contract.is_none() {
                return Err(err("start_after requires nft_contract"));
            }
            let listings = if let Some(nft_raw) = nft_contract {
                let nft = deps.api.addr_validate(&nft_raw)?;
                let start = start_after_token.as_deref().map(Bound::exclusive);
                LISTINGS
                    .prefix(&nft)
                    .range(deps.storage, start, None, cosmwasm_std::Order::Ascending)
                    .take(limit)
                    .map(|item| {
                        item.map(|(token_id, listing)| listing_info(&nft, &token_id, listing))
                    })
                    .collect::<StdResult<Vec<_>>>()?
            } else {
                LISTINGS
                    .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
                    .take(limit)
                    .map(|item| {
                        item.map(|((nft, token_id), listing)| {
                            listing_info(&nft, &token_id, listing)
                        })
                    })
                    .collect::<StdResult<Vec<_>>>()?
            };
            to_json_binary(&ListingsResponse { listings })
        }
        QueryMsg::Warning {} => to_json_binary(&WarningResponse {
            not_audited: true,
            warning: NOT_AUDITED_WARNING.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
    use cosmwasm_std::{coins, from_json, CosmosMsg};

    const XCOIN: &str = "ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C";

    struct Accounts {
        admin: Addr,
        nft: Addr,
        seller: Addr,
        buyer: Addr,
        fee: Addr,
    }

    fn accounts(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::MemoryStorage,
            cosmwasm_std::testing::MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
    ) -> Accounts {
        Accounts {
            admin: deps.api.addr_make("admin"),
            nft: deps.api.addr_make("nft"),
            seller: deps.api.addr_make("seller"),
            buyer: deps.api.addr_make("buyer"),
            fee: deps.api.addr_make("fee"),
        }
    }

    fn init(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::MemoryStorage,
            cosmwasm_std::testing::MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
        accts: &Accounts,
    ) {
        instantiate(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            InstantiateMsg {
                admin: accts.admin.to_string(),
                fee_bps: 30,
                fee_recipient: accts.fee.to_string(),
                allowed_denoms: vec!["aqcoin".into(), XCOIN.into()],
                nft_contracts: vec![accts.nft.to_string()],
            },
        )
        .unwrap();
    }

    fn list_via_hook(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::MemoryStorage,
            cosmwasm_std::testing::MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
        accts: &Accounts,
        token_id: &str,
        price: Coin,
    ) {
        let msg = to_json_binary(&HookMsg::List { price }).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.nft, &[]),
            ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
                sender: accts.seller.to_string(),
                token_id: token_id.into(),
                msg,
            }),
        )
        .unwrap();
    }

    fn bank_to(msg: &CosmosMsg, addr: &Addr) -> Option<Uint128> {
        if let CosmosMsg::Bank(BankMsg::Send { to_address, amount }) = msg {
            if to_address == &addr.to_string() {
                return Some(amount[0].amount);
            }
        }
        None
    }

    #[test]
    fn warning_text_is_the_required_sentence() {
        assert_eq!(
            NOT_AUDITED_WARNING,
            "Dieser Contract ist nicht auditiert. Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst."
        );
    }

    #[test]
    fn instantiate_rejects_bad_config_and_records_warning() {
        let mut deps = mock_dependencies();
        let accts = accounts(&mut deps);
        let bad_fee = instantiate(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            InstantiateMsg {
                admin: accts.admin.to_string(),
                fee_bps: 10_000,
                fee_recipient: accts.fee.to_string(),
                allowed_denoms: vec!["aqcoin".into()],
                nft_contracts: vec![],
            },
        )
        .unwrap_err();
        assert!(bad_fee.to_string().contains("fee_bps"));
        let bad_denom = instantiate(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            InstantiateMsg {
                admin: accts.admin.to_string(),
                fee_bps: 30,
                fee_recipient: accts.fee.to_string(),
                allowed_denoms: vec!["aqcoin".into(), "aqcoin".into()],
                nft_contracts: vec![],
            },
        )
        .unwrap_err();
        assert!(bad_denom.to_string().contains("invalid denoms"));
        let self_fee = instantiate(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            InstantiateMsg {
                admin: accts.admin.to_string(),
                fee_bps: 30,
                fee_recipient: mock_env().contract.address.to_string(),
                allowed_denoms: vec!["aqcoin".into()],
                nft_contracts: vec![],
            },
        )
        .unwrap_err();
        assert!(self_fee
            .to_string()
            .contains("fee recipient is the contract"));
        init(&mut deps, &accts);
        let res_warning = query(deps.as_ref(), mock_env(), QueryMsg::Warning {}).unwrap();
        let warning: WarningResponse = from_json(res_warning).unwrap();
        assert!(warning.not_audited);
        let cfg: ConfigResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
        assert_eq!(cfg.fee_bps, 30);
        assert_eq!(cfg.admin, accts.admin.to_string());
        assert!(!cfg.paused);
        assert!(cfg.not_audited);
    }

    #[test]
    fn list_buy_cancel_fee_and_rejections() {
        let mut deps = mock_dependencies();
        let accts = accounts(&mut deps);
        let stranger = deps.api.addr_make("stranger");
        init(&mut deps, &accts);

        let rogue = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&stranger, &[]),
            ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
                sender: accts.seller.to_string(),
                token_id: "1".into(),
                msg: to_json_binary(&HookMsg::List {
                    price: Coin::new(10_000u128, "aqcoin"),
                })
                .unwrap(),
            }),
        )
        .unwrap_err();
        assert!(rogue.to_string().contains("collection not allowed"));

        let zero = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.nft, &[]),
            ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
                sender: accts.seller.to_string(),
                token_id: "1".into(),
                msg: to_json_binary(&HookMsg::List {
                    price: Coin::new(0u128, "aqcoin"),
                })
                .unwrap(),
            }),
        )
        .unwrap_err();
        assert!(zero.to_string().contains("zero price"));

        let wrong = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.nft, &[]),
            ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
                sender: accts.seller.to_string(),
                token_id: "1".into(),
                msg: to_json_binary(&HookMsg::List {
                    price: Coin::new(10_000u128, "ustake"),
                })
                .unwrap(),
            }),
        )
        .unwrap_err();
        assert!(wrong.to_string().contains("unexpected denom"));

        list_via_hook(&mut deps, &accts, "1", Coin::new(10_000u128, "aqcoin"));
        let again = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.nft, &[]),
            ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
                sender: accts.seller.to_string(),
                token_id: "1".into(),
                msg: to_json_binary(&HookMsg::List {
                    price: Coin::new(10_000u128, "aqcoin"),
                })
                .unwrap(),
            }),
        )
        .unwrap_err();
        assert!(again.to_string().contains("already listed"));

        let no_funds = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &[]),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap_err();
        assert!(no_funds
            .to_string()
            .contains("send exactly the listing price"));
        let still: ListingResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Listing {
                    nft_contract: accts.nft.to_string(),
                    token_id: "1".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(still.listing.is_some());

        let bad_amount = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(9999, "aqcoin")),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap_err();
        assert!(bad_amount.to_string().contains("wrong payment"));

        let bad_denom = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(10_000, XCOIN)),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap_err();
        assert!(bad_denom.to_string().contains("unexpected denom"));

        let self_buy = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.seller, &coins(10_000, "aqcoin")),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap_err();
        assert!(self_buy.to_string().contains("seller cannot buy"));

        let not_seller = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &[]),
            ExecuteMsg::Cancel {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap_err();
        assert!(not_seller.to_string().contains("only seller can cancel"));

        let buy = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(10_000, "aqcoin")),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap();
        assert_eq!(
            bank_to(&buy.messages[0].msg, &accts.seller),
            Some(Uint128::new(9970))
        );
        assert_eq!(
            bank_to(&buy.messages[1].msg, &accts.fee),
            Some(Uint128::new(30))
        );
        match &buy.messages[2].msg {
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr,
                msg,
                funds,
            }) => {
                assert_eq!(contract_addr, &accts.nft.to_string());
                assert!(funds.is_empty());
                let parsed: NftExecute = from_json(msg).unwrap();
                match parsed {
                    NftExecute::TransferNft {
                        recipient,
                        token_id,
                    } => {
                        assert_eq!(recipient, accts.buyer.to_string());
                        assert_eq!(token_id, "1");
                    }
                }
            }
            _ => panic!("expected nft transfer"),
        }
        let gone: ListingResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Listing {
                    nft_contract: accts.nft.to_string(),
                    token_id: "1".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(gone.listing.is_none());
        let double = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(10_000, "aqcoin")),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap_err();
        assert!(double.to_string().contains("listing not found"));
    }

    #[test]
    fn cancel_returns_nft_and_pause_blocks_buy_not_cancel() {
        let mut deps = mock_dependencies();
        let accts = accounts(&mut deps);
        init(&mut deps, &accts);
        list_via_hook(&mut deps, &accts, "2", Coin::new(1u128, XCOIN));
        let tiny = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(1, XCOIN)),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "2".into(),
            },
        )
        .unwrap();
        // 1 * 30 / 10000 = 0, seller receives the whole atom, no fee message.
        assert_eq!(tiny.messages.len(), 2);
        assert_eq!(
            bank_to(&tiny.messages[0].msg, &accts.seller),
            Some(Uint128::new(1))
        );

        list_via_hook(&mut deps, &accts, "3", Coin::new(5_000u128, "aqcoin"));
        let not_admin = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.seller, &[]),
            ExecuteMsg::Pause {},
        )
        .unwrap_err();
        assert!(not_admin.to_string().contains("not admin"));
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            ExecuteMsg::Pause {},
        )
        .unwrap();
        let paused_buy = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(5_000, "aqcoin")),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "3".into(),
            },
        )
        .unwrap_err();
        assert!(paused_buy.to_string().contains("paused"));
        let paused_list = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.nft, &[]),
            ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
                sender: accts.seller.to_string(),
                token_id: "4".into(),
                msg: to_json_binary(&HookMsg::List {
                    price: Coin::new(5_000u128, "aqcoin"),
                })
                .unwrap(),
            }),
        )
        .unwrap_err();
        assert!(paused_list.to_string().contains("paused"));
        let cancel = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.seller, &[]),
            ExecuteMsg::Cancel {
                nft_contract: accts.nft.to_string(),
                token_id: "3".into(),
            },
        )
        .unwrap();
        match &cancel.messages[0].msg {
            CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                let parsed: NftExecute = from_json(msg).unwrap();
                match parsed {
                    NftExecute::TransferNft {
                        recipient,
                        token_id,
                    } => {
                        assert_eq!(recipient, accts.seller.to_string());
                        assert_eq!(token_id, "3");
                    }
                }
            }
            _ => panic!("expected transfer back"),
        }
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            ExecuteMsg::Resume {},
        )
        .unwrap();
    }

    #[test]
    fn removed_denom_blocks_buy_and_rescue_refuses_listed_tokens() {
        let mut deps = mock_dependencies();
        let accts = accounts(&mut deps);
        init(&mut deps, &accts);
        list_via_hook(&mut deps, &accts, "9", Coin::new(10_000u128, "aqcoin"));
        let rescue = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            ExecuteMsg::Rescue {
                nft_contract: accts.nft.to_string(),
                token_id: "9".into(),
                recipient: accts.seller.to_string(),
            },
        )
        .unwrap_err();
        assert!(rescue.to_string().contains("listing exists"));
        let stranger = deps.api.addr_make("stranger");
        let denied = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&stranger, &[]),
            ExecuteMsg::Rescue {
                nft_contract: accts.nft.to_string(),
                token_id: "8".into(),
                recipient: stranger.to_string(),
            },
        )
        .unwrap_err();
        assert!(denied.to_string().contains("not admin"));
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.admin, &[]),
            ExecuteMsg::UpdateConfig {
                fee_bps: None,
                fee_recipient: None,
                allowed_denoms: Some(vec![XCOIN.into()]),
            },
        )
        .unwrap();
        let blocked = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.buyer, &coins(10_000, "aqcoin")),
            ExecuteMsg::Buy {
                nft_contract: accts.nft.to_string(),
                token_id: "9".into(),
            },
        )
        .unwrap_err();
        assert!(blocked.to_string().contains("unexpected denom"));
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&accts.seller, &[]),
            ExecuteMsg::Cancel {
                nft_contract: accts.nft.to_string(),
                token_id: "9".into(),
            },
        )
        .unwrap();
        let remove_busy = {
            list_via_hook(&mut deps, &accts, "10", Coin::new(2u128, XCOIN));
            execute(
                deps.as_mut(),
                mock_env(),
                message_info(&accts.admin, &[]),
                ExecuteMsg::RemoveCollection {
                    nft_contract: accts.nft.to_string(),
                },
            )
            .unwrap_err()
        };
        assert!(remove_busy.to_string().contains("collection has listings"));
    }

    #[test]
    fn fee_split_table() {
        let (seller, fee) = split_fee(Uint128::new(10_000), 30).unwrap();
        assert_eq!((seller.u128(), fee.u128()), (9970, 30));
        let (seller, fee) = split_fee(Uint128::new(1), 30).unwrap();
        assert_eq!((seller.u128(), fee.u128()), (1, 0));
        assert!(split_fee(Uint128::new(1), 10_000).is_err());
    }
}
