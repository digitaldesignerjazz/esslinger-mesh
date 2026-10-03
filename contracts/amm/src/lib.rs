//! esslinger-mesh-amm: minimal constant-product AMM (x*y=k) for two bank denoms (native or ibc/...).
//! Own code, Apache-2.0. No LP token contract: LP shares are tracked in contract storage.
//! Fee (fee_bps) stays in the pool and accrues to liquidity providers.
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, Uint256,
};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct InstantiateMsg {
    pub denom_a: String,
    pub denom_b: String,
    pub fee_bps: u16,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Send both denoms as funds. First deposit sets the price; later deposits get
    /// min(a*S/Ra, b*S/Rb) shares (any excess beyond the pool ratio stays in the pool).
    ProvideLiquidity { min_shares: Option<Uint128> },
    WithdrawLiquidity { shares: Uint128 },
    /// Send exactly one of the two denoms as funds; receive the other.
    Swap { min_out: Uint128 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(PoolResponse)]
    Pool {},
    #[returns(SimulateResponse)]
    Simulate { offer: Coin },
    #[returns(Uint128)]
    Shares { address: String },
}

#[cw_serde]
pub struct Config {
    pub denom_a: String,
    pub denom_b: String,
    pub fee_bps: u16,
}

#[cw_serde]
pub struct PoolResponse {
    pub denom_a: String,
    pub denom_b: String,
    pub reserve_a: Uint128,
    pub reserve_b: Uint128,
    pub total_shares: Uint128,
    pub fee_bps: u16,
}

#[cw_serde]
pub struct SimulateResponse {
    pub return_amount: Uint128,
    pub ask_denom: String,
}

const CONFIG: Item<Config> = Item::new("config");
const RESERVES: Item<(Uint128, Uint128)> = Item::new("reserves");
const TOTAL_SHARES: Item<Uint128> = Item::new("total_shares");
const SHARES: Map<&Addr, Uint128> = Map::new("shares");

fn err(msg: &str) -> StdError {
    StdError::generic_err(msg)
}

fn to128(v: Uint256) -> StdResult<Uint128> {
    Uint128::try_from(v).map_err(|_| err("overflow"))
}

fn isqrt(n: Uint256) -> Uint256 {
    if n.is_zero() {
        return n;
    }
    let mut x = n;
    let mut y = (x + Uint256::one()) >> 1;
    while y < x {
        x = y;
        y = (x + n / x) >> 1;
    }
    x
}

#[entry_point]
pub fn instantiate(deps: DepsMut, _env: Env, _info: MessageInfo, msg: InstantiateMsg) -> StdResult<Response> {
    if msg.denom_a == msg.denom_b || msg.denom_a.is_empty() || msg.denom_b.is_empty() {
        return Err(err("invalid denoms"));
    }
    if msg.fee_bps >= 10_000 {
        return Err(err("fee_bps must be < 10000"));
    }
    CONFIG.save(deps.storage, &Config { denom_a: msg.denom_a.clone(), denom_b: msg.denom_b.clone(), fee_bps: msg.fee_bps })?;
    RESERVES.save(deps.storage, &(Uint128::zero(), Uint128::zero()))?;
    TOTAL_SHARES.save(deps.storage, &Uint128::zero())?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("denom_a", msg.denom_a)
        .add_attribute("denom_b", msg.denom_b)
        .add_attribute("fee_bps", msg.fee_bps.to_string()))
}

fn paid(info: &MessageInfo, denom: &str) -> Uint128 {
    info.funds.iter().filter(|c| c.denom == denom).map(|c| c.amount).fold(Uint128::zero(), |a, b| a + b)
}

#[entry_point]
pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;
    let (ra, rb) = RESERVES.load(deps.storage)?;
    let total = TOTAL_SHARES.load(deps.storage)?;
    match msg {
        ExecuteMsg::ProvideLiquidity { min_shares } => {
            if info.funds.iter().any(|c| c.denom != cfg.denom_a && c.denom != cfg.denom_b) {
                return Err(err("unexpected denom in funds"));
            }
            let a = paid(&info, &cfg.denom_a);
            let b = paid(&info, &cfg.denom_b);
            if a.is_zero() || b.is_zero() {
                return Err(err("both denoms required"));
            }
            let minted = if total.is_zero() {
                to128(isqrt(Uint256::from(a) * Uint256::from(b)))?
            } else {
                let sa = Uint256::from(a) * Uint256::from(total) / Uint256::from(ra);
                let sb = Uint256::from(b) * Uint256::from(total) / Uint256::from(rb);
                to128(std::cmp::min(sa, sb))?
            };
            if minted.is_zero() {
                return Err(err("zero shares"));
            }
            if let Some(m) = min_shares {
                if minted < m {
                    return Err(err("slippage: shares below min_shares"));
                }
            }
            RESERVES.save(deps.storage, &(ra + a, rb + b))?;
            TOTAL_SHARES.save(deps.storage, &(total + minted))?;
            SHARES.update(deps.storage, &info.sender, |s| -> StdResult<_> { Ok(s.unwrap_or_default() + minted) })?;
            Ok(Response::new()
                .add_attribute("action", "provide_liquidity")
                .add_attribute("provider", info.sender)
                .add_attribute("amount_a", a)
                .add_attribute("amount_b", b)
                .add_attribute("shares", minted))
        }
        ExecuteMsg::WithdrawLiquidity { shares } => {
            if !info.funds.is_empty() {
                return Err(err("no funds expected"));
            }
            let have = SHARES.may_load(deps.storage, &info.sender)?.unwrap_or_default();
            if shares.is_zero() || shares > have {
                return Err(err("invalid share amount"));
            }
            let a = to128(Uint256::from(ra) * Uint256::from(shares) / Uint256::from(total))?;
            let b = to128(Uint256::from(rb) * Uint256::from(shares) / Uint256::from(total))?;
            RESERVES.save(deps.storage, &(ra - a, rb - b))?;
            TOTAL_SHARES.save(deps.storage, &(total - shares))?;
            SHARES.save(deps.storage, &info.sender, &(have - shares))?;
            let mut coins = vec![];
            if !a.is_zero() { coins.push(Coin { denom: cfg.denom_a.clone(), amount: a }); }
            if !b.is_zero() { coins.push(Coin { denom: cfg.denom_b.clone(), amount: b }); }
            let mut resp = Response::new();
            if !coins.is_empty() {
                resp = resp.add_message(BankMsg::Send { to_address: info.sender.to_string(), amount: coins });
            }
            Ok(resp
                .add_attribute("action", "withdraw_liquidity")
                .add_attribute("shares", shares)
                .add_attribute("amount_a", a)
                .add_attribute("amount_b", b))
        }
        ExecuteMsg::Swap { min_out } => {
            if info.funds.len() != 1 {
                return Err(err("send exactly one coin"));
            }
            let offer = &info.funds[0];
            let (out, ask_denom, new_ra, new_rb) = swap_math(&cfg, ra, rb, offer)?;
            if out < min_out {
                return Err(err("slippage: return below min_out"));
            }
            RESERVES.save(deps.storage, &(new_ra, new_rb))?;
            Ok(Response::new()
                .add_message(BankMsg::Send { to_address: info.sender.to_string(), amount: vec![Coin { denom: ask_denom.clone(), amount: out }] })
                .add_attribute("action", "swap")
                .add_attribute("trader", info.sender)
                .add_attribute("offer_denom", offer.denom.clone())
                .add_attribute("offer_amount", offer.amount)
                .add_attribute("ask_denom", ask_denom)
                .add_attribute("return_amount", out))
        }
    }
}

/// Returns (return_amount, ask_denom, new_reserve_a, new_reserve_b).
fn swap_math(cfg: &Config, ra: Uint128, rb: Uint128, offer: &Coin) -> StdResult<(Uint128, String, Uint128, Uint128)> {
    if ra.is_zero() || rb.is_zero() {
        return Err(err("pool has no liquidity"));
    }
    if offer.amount.is_zero() {
        return Err(err("zero offer"));
    }
    let a_to_b = if offer.denom == cfg.denom_a {
        true
    } else if offer.denom == cfg.denom_b {
        false
    } else {
        return Err(err("offer denom not in pool"));
    };
    let (rin, rout) = if a_to_b { (ra, rb) } else { (rb, ra) };
    let in_net = Uint256::from(offer.amount) * Uint256::from(10_000u128 - cfg.fee_bps as u128) / Uint256::from(10_000u128);
    let out = to128(Uint256::from(rout) * in_net / (Uint256::from(rin) + in_net))?;
    if out.is_zero() || out >= rout {
        return Err(err("invalid return amount"));
    }
    if a_to_b {
        Ok((out, cfg.denom_b.clone(), ra + offer.amount, rb - out))
    } else {
        Ok((out, cfg.denom_a.clone(), ra - out, rb + offer.amount))
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let cfg = CONFIG.load(deps.storage)?;
    let (ra, rb) = RESERVES.load(deps.storage)?;
    match msg {
        QueryMsg::Pool {} => to_json_binary(&PoolResponse {
            denom_a: cfg.denom_a.clone(),
            denom_b: cfg.denom_b.clone(),
            reserve_a: ra,
            reserve_b: rb,
            total_shares: TOTAL_SHARES.load(deps.storage)?,
            fee_bps: cfg.fee_bps,
        }),
        QueryMsg::Simulate { offer } => {
            let (out, ask, _, _) = swap_math(&cfg, ra, rb, &offer)?;
            to_json_binary(&SimulateResponse { return_amount: out, ask_denom: ask })
        }
        QueryMsg::Shares { address } => {
            let addr = deps.api.addr_validate(&address)?;
            to_json_binary(&SHARES.may_load(deps.storage, &addr)?.unwrap_or_default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
    use cosmwasm_std::coins;

    #[test]
    fn provide_and_swap() {
        let mut deps = mock_dependencies();
        let op = deps.api.addr_make("op");
        instantiate(deps.as_mut(), mock_env(), message_info(&op, &[]),
            InstantiateMsg { denom_a: "ibc/X".into(), denom_b: "aqcoin".into(), fee_bps: 30 }).unwrap();
        let e18 = 1_000_000_000_000_000_000u128;
        let funds = vec![Coin::new(1000 * e18, "ibc/X"), Coin::new(2500 * e18, "aqcoin")];
        execute(deps.as_mut(), mock_env(), message_info(&op, &funds), ExecuteMsg::ProvideLiquidity { min_shares: None }).unwrap();
        let r = execute(deps.as_mut(), mock_env(), message_info(&op, &coins(10 * e18, "ibc/X")), ExecuteMsg::Swap { min_out: Uint128::zero() }).unwrap();
        let out: u128 = r.attributes.iter().find(|a| a.key == "return_amount").unwrap().value.parse().unwrap();
        // 2500 * 9.97 / (1000 + 9.97) = 24.6782... QCOIN
        assert!(out > 24 * e18 && out < 25 * e18);
        let (ra, rb) = RESERVES.load(&deps.storage).unwrap();
        assert_eq!(ra.u128(), 1010 * e18);
        assert_eq!(rb.u128(), 2500 * e18 - out);
    }
}
