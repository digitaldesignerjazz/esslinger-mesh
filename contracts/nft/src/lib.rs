//! esslinger-mesh-cw721: CW721-compatible collection for Esslinger Cyberspace.
//! Own code, Apache-2.0. Message names match the CW721 JSON API
//! (Mint, TransferNft, SendNft, Approve, OwnerOf, NftInfo, ...).
//!
//! NOT AUDITED. Use at your own risk. Badges carry no governance rights.
//!
//! SendNft hook `sender` is the token owner before the transfer, so an
//! operator cannot redirect marketplace proceeds. Operators may still
//! TransferNft when approved.
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, BlockInfo, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, StdResult, Timestamp, WasmMsg,
};
use cw_storage_plus::{Bound, Item, Map};

pub const NOT_AUDITED_WARNING: &str = "Dieser Contract ist nicht auditiert. Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst.";

const MAX_TOKEN_ID_LEN: usize = 128;
const MAX_URI_LEN: usize = 2048;
const MAX_NAME_LEN: usize = 128;
const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    /// Address allowed to mint. Deploy uses the nexus-qcoin-1 gov module.
    pub minter: String,
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    Mint {
        token_id: String,
        owner: String,
        token_uri: Option<String>,
    },
    TransferNft {
        recipient: String,
        token_id: String,
    },
    SendNft {
        contract: String,
        token_id: String,
        msg: Binary,
    },
    Approve {
        spender: String,
        token_id: String,
        expires: Option<Expiration>,
    },
    Revoke {
        spender: String,
        token_id: String,
    },
    ApproveAll {
        operator: String,
        expires: Option<Expiration>,
    },
    RevokeAll {
        operator: String,
    },
    Burn {
        token_id: String,
    },
    /// Current minter only. `minter: null` freezes minting.
    UpdateMinter {
        minter: Option<String>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(OwnerOfResponse)]
    OwnerOf {
        token_id: String,
        include_expired: Option<bool>,
    },
    #[returns(ApprovalResponse)]
    Approval {
        token_id: String,
        spender: String,
        include_expired: Option<bool>,
    },
    #[returns(ApprovalsResponse)]
    Approvals {
        token_id: String,
        include_expired: Option<bool>,
    },
    #[returns(OperatorsResponse)]
    AllOperators {
        owner: String,
        include_expired: Option<bool>,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(NumTokensResponse)]
    NumTokens {},
    #[returns(ContractInfoResponse)]
    ContractInfo {},
    #[returns(NftInfoResponse)]
    NftInfo { token_id: String },
    #[returns(AllNftInfoResponse)]
    AllNftInfo {
        token_id: String,
        include_expired: Option<bool>,
    },
    #[returns(TokensResponse)]
    Tokens {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(TokensResponse)]
    AllTokens {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(MinterResponse)]
    Minter {},
    #[returns(WarningResponse)]
    Warning {},
}

/// CW721 approval expiry. CosmWasm 2 dropped `cosmwasm_std::Expiration`; the JSON
/// shape matches the CW721 standard (`at_height`, `at_time`, `never`).
#[cw_serde]
#[derive(Copy)]
pub enum Expiration {
    AtHeight(u64),
    AtTime(Timestamp),
    Never {},
}

impl Expiration {
    pub fn is_expired(&self, block: &BlockInfo) -> bool {
        match self {
            Expiration::AtHeight(height) => block.height >= *height,
            Expiration::AtTime(time) => block.time >= *time,
            Expiration::Never {} => false,
        }
    }
}

#[cw_serde]
pub struct OwnerOfResponse {
    pub owner: String,
    pub approvals: Vec<Approval>,
}

#[cw_serde]
pub struct Approval {
    pub spender: String,
    pub expires: Expiration,
}

#[cw_serde]
pub struct ApprovalResponse {
    pub approval: Approval,
}

#[cw_serde]
pub struct ApprovalsResponse {
    pub approvals: Vec<Approval>,
}

#[cw_serde]
pub struct OperatorsResponse {
    pub operators: Vec<Approval>,
}

#[cw_serde]
pub struct NumTokensResponse {
    pub count: u64,
}

#[cw_serde]
pub struct ContractInfoResponse {
    pub name: String,
    pub symbol: String,
}

#[cw_serde]
pub struct NftInfoResponse {
    pub token_uri: Option<String>,
    pub extension: Empty,
}

#[cw_serde]
pub struct AllNftInfoResponse {
    pub access: OwnerOfResponse,
    pub info: NftInfoResponse,
}

#[cw_serde]
pub struct TokensResponse {
    pub tokens: Vec<String>,
}

#[cw_serde]
pub struct MinterResponse {
    pub minter: Option<String>,
}

#[cw_serde]
pub struct WarningResponse {
    pub not_audited: bool,
    pub warning: String,
}

#[cw_serde]
struct ContractConfig {
    name: String,
    symbol: String,
}

#[cw_serde]
struct TokenApproval {
    spender: Addr,
    expires: Expiration,
}

#[cw_serde]
struct TokenInfo {
    owner: Addr,
    approvals: Vec<TokenApproval>,
    token_uri: Option<String>,
}

const CONFIG: Item<ContractConfig> = Item::new("config");
const MINTER: Item<Option<Addr>> = Item::new("minter");
const NUM_TOKENS: Item<u64> = Item::new("num_tokens");
const TOKENS: Map<&str, TokenInfo> = Map::new("tokens");
const OWNED: Map<(&Addr, &str), Empty> = Map::new("owned");
const OPERATORS: Map<(&Addr, &Addr), Expiration> = Map::new("operators");

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

fn check_uri(token_uri: &Option<String>) -> StdResult<()> {
    match token_uri {
        None => Ok(()),
        Some(uri) if uri.is_empty() || uri.len() > MAX_URI_LEN => Err(err("invalid token_uri")),
        Some(_) => Ok(()),
    }
}

fn page_limit(limit: Option<u32>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize
}

fn load_token(deps: Deps, token_id: &str) -> StdResult<TokenInfo> {
    check_token_id(token_id)?;
    TOKENS
        .may_load(deps.storage, token_id)?
        .ok_or_else(|| err("token not found"))
}

fn can_transfer(deps: Deps, env: &Env, info: &MessageInfo, token: &TokenInfo) -> StdResult<()> {
    if info.sender == token.owner {
        return Ok(());
    }
    if token
        .approvals
        .iter()
        .any(|a| a.spender == info.sender && !a.expires.is_expired(&env.block))
    {
        return Ok(());
    }
    if let Some(exp) = OPERATORS.may_load(deps.storage, (&token.owner, &info.sender))? {
        if !exp.is_expired(&env.block) {
            return Ok(());
        }
    }
    Err(err("unauthorized"))
}

fn apply_transfer(
    deps: DepsMut,
    token_id: &str,
    mut token: TokenInfo,
    recipient: Addr,
) -> StdResult<()> {
    OWNED.remove(deps.storage, (&token.owner, token_id));
    token.owner = recipient;
    token.approvals.clear();
    OWNED.save(deps.storage, (&token.owner, token_id), &Empty {})?;
    TOKENS.save(deps.storage, token_id, &token)?;
    Ok(())
}

fn visible_approvals(token: &TokenInfo, env: &Env, include_expired: Option<bool>) -> Vec<Approval> {
    let include = include_expired.unwrap_or(false);
    token
        .approvals
        .iter()
        .filter(|a| include || !a.expires.is_expired(&env.block))
        .map(|a| Approval {
            spender: a.spender.to_string(),
            expires: a.expires,
        })
        .collect()
}

fn owner_of(token: &TokenInfo, env: &Env, include_expired: Option<bool>) -> OwnerOfResponse {
    OwnerOfResponse {
        owner: token.owner.to_string(),
        approvals: visible_approvals(token, env, include_expired),
    }
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    if msg.name.is_empty()
        || msg.name.len() > MAX_NAME_LEN
        || msg.symbol.is_empty()
        || msg.symbol.len() > MAX_NAME_LEN
    {
        return Err(err("invalid collection"));
    }
    let minter = deps.api.addr_validate(&msg.minter)?;
    CONFIG.save(
        deps.storage,
        &ContractConfig {
            name: msg.name.clone(),
            symbol: msg.symbol.clone(),
        },
    )?;
    MINTER.save(deps.storage, &Some(minter))?;
    NUM_TOKENS.save(deps.storage, &0u64)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("name", msg.name)
        .add_attribute("symbol", msg.symbol)
        .add_attribute("minter", msg.minter)
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
    no_funds(&info)?;
    match msg {
        ExecuteMsg::Mint {
            token_id,
            owner,
            token_uri,
        } => execute_mint(deps, info, token_id, owner, token_uri),
        ExecuteMsg::TransferNft {
            recipient,
            token_id,
        } => execute_transfer(deps, env, info, recipient, token_id),
        ExecuteMsg::SendNft {
            contract,
            token_id,
            msg,
        } => execute_send(deps, env, info, contract, token_id, msg),
        ExecuteMsg::Approve {
            spender,
            token_id,
            expires,
        } => execute_approve(deps, env, info, spender, token_id, expires),
        ExecuteMsg::Revoke { spender, token_id } => execute_revoke(deps, info, spender, token_id),
        ExecuteMsg::ApproveAll { operator, expires } => {
            execute_approve_all(deps, info, operator, expires)
        }
        ExecuteMsg::RevokeAll { operator } => execute_revoke_all(deps, info, operator),
        ExecuteMsg::Burn { token_id } => execute_burn(deps, env, info, token_id),
        ExecuteMsg::UpdateMinter { minter } => execute_update_minter(deps, info, minter),
    }
}

fn execute_mint(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
    owner: String,
    token_uri: Option<String>,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    check_uri(&token_uri)?;
    let minter = MINTER
        .load(deps.storage)?
        .ok_or_else(|| err("not minter"))?;
    if info.sender != minter {
        return Err(err("not minter"));
    }
    if TOKENS.may_load(deps.storage, &token_id)?.is_some() {
        return Err(err("token exists"));
    }
    let owner = deps.api.addr_validate(&owner)?;
    let count = NUM_TOKENS.load(deps.storage)?;
    let next = count
        .checked_add(1)
        .ok_or_else(|| err("num tokens overflow"))?;
    TOKENS.save(
        deps.storage,
        &token_id,
        &TokenInfo {
            owner: owner.clone(),
            approvals: vec![],
            token_uri,
        },
    )?;
    OWNED.save(deps.storage, (&owner, &token_id), &Empty {})?;
    NUM_TOKENS.save(deps.storage, &next)?;
    Ok(Response::new()
        .add_attribute("action", "mint")
        .add_attribute("minter", info.sender)
        .add_attribute("owner", owner)
        .add_attribute("token_id", token_id))
}

fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    token_id: String,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    let token = load_token(deps.as_ref(), &token_id)?;
    can_transfer(deps.as_ref(), &env, &info, &token)?;
    let recipient_addr = deps.api.addr_validate(&recipient)?;
    let sender = info.sender.to_string();
    apply_transfer(deps, &token_id, token, recipient_addr)?;
    Ok(Response::new()
        .add_attribute("action", "transfer_nft")
        .add_attribute("sender", sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token_id", token_id))
}

fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    token_id: String,
    msg: Binary,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    let token = load_token(deps.as_ref(), &token_id)?;
    can_transfer(deps.as_ref(), &env, &info, &token)?;
    let recipient = deps.api.addr_validate(&contract)?;
    let owner = token.owner.clone();
    apply_transfer(deps, &token_id, token, recipient.clone())?;
    let hook = to_json_binary(&ReceiveNftHook {
        receive_nft: Cw721Hook {
            sender: owner.to_string(),
            token_id: token_id.clone(),
            msg,
        },
    })?;
    Ok(Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: recipient.to_string(),
            msg: hook,
            funds: vec![],
        })
        .add_attribute("action", "send_nft")
        .add_attribute("sender", owner)
        .add_attribute("recipient", recipient)
        .add_attribute("token_id", token_id))
}

#[cw_serde]
struct Cw721Hook {
    sender: String,
    token_id: String,
    msg: Binary,
}

#[cw_serde]
struct ReceiveNftHook {
    receive_nft: Cw721Hook,
}

fn execute_approve(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    spender: String,
    token_id: String,
    expires: Option<Expiration>,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    let mut token = load_token(deps.as_ref(), &token_id)?;
    if info.sender != token.owner {
        return Err(err("unauthorized"));
    }
    let spender_addr = deps.api.addr_validate(&spender)?;
    if spender_addr == token.owner {
        return Err(err("cannot approve owner"));
    }
    let expires = expires.unwrap_or(Expiration::Never {});
    if let Some(existing) = token
        .approvals
        .iter_mut()
        .find(|a| a.spender == spender_addr)
    {
        existing.expires = expires;
    } else {
        token.approvals.push(TokenApproval {
            spender: spender_addr,
            expires,
        });
    }
    // Drop expired approvals so storage stays bounded by the owner's own actions.
    token
        .approvals
        .retain(|a| !a.expires.is_expired(&env.block));
    TOKENS.save(deps.storage, &token_id, &token)?;
    Ok(Response::new()
        .add_attribute("action", "approve")
        .add_attribute("sender", info.sender)
        .add_attribute("spender", spender)
        .add_attribute("token_id", token_id))
}

fn execute_revoke(
    deps: DepsMut,
    info: MessageInfo,
    spender: String,
    token_id: String,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    let mut token = load_token(deps.as_ref(), &token_id)?;
    if info.sender != token.owner {
        return Err(err("unauthorized"));
    }
    let spender_addr = deps.api.addr_validate(&spender)?;
    token.approvals.retain(|a| a.spender != spender_addr);
    TOKENS.save(deps.storage, &token_id, &token)?;
    Ok(Response::new()
        .add_attribute("action", "revoke")
        .add_attribute("sender", info.sender)
        .add_attribute("spender", spender)
        .add_attribute("token_id", token_id))
}

fn execute_approve_all(
    deps: DepsMut,
    info: MessageInfo,
    operator: String,
    expires: Option<Expiration>,
) -> StdResult<Response> {
    let operator_addr = deps.api.addr_validate(&operator)?;
    if operator_addr == info.sender {
        return Err(err("cannot approve owner"));
    }
    let expires = expires.unwrap_or(Expiration::Never {});
    OPERATORS.save(deps.storage, (&info.sender, &operator_addr), &expires)?;
    Ok(Response::new()
        .add_attribute("action", "approve_all")
        .add_attribute("sender", info.sender)
        .add_attribute("operator", operator))
}

fn execute_revoke_all(deps: DepsMut, info: MessageInfo, operator: String) -> StdResult<Response> {
    let operator_addr = deps.api.addr_validate(&operator)?;
    OPERATORS.remove(deps.storage, (&info.sender, &operator_addr));
    Ok(Response::new()
        .add_attribute("action", "revoke_all")
        .add_attribute("sender", info.sender)
        .add_attribute("operator", operator))
}

fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: String,
) -> StdResult<Response> {
    check_token_id(&token_id)?;
    let token = load_token(deps.as_ref(), &token_id)?;
    can_transfer(deps.as_ref(), &env, &info, &token)?;
    OWNED.remove(deps.storage, (&token.owner, &token_id));
    TOKENS.remove(deps.storage, &token_id);
    let count = NUM_TOKENS.load(deps.storage)?;
    let next = count
        .checked_sub(1)
        .ok_or_else(|| err("num tokens underflow"))?;
    NUM_TOKENS.save(deps.storage, &next)?;
    Ok(Response::new()
        .add_attribute("action", "burn")
        .add_attribute("sender", info.sender)
        .add_attribute("token_id", token_id))
}

fn execute_update_minter(
    deps: DepsMut,
    info: MessageInfo,
    minter: Option<String>,
) -> StdResult<Response> {
    let current = MINTER
        .load(deps.storage)?
        .ok_or_else(|| err("not minter"))?;
    if info.sender != current {
        return Err(err("not minter"));
    }
    let stored = match minter {
        Some(m) => Some(deps.api.addr_validate(&m)?),
        None => None,
    };
    MINTER.save(deps.storage, &stored)?;
    Ok(Response::new()
        .add_attribute("action", "update_minter")
        .add_attribute("minter", stored.map(|a| a.to_string()).unwrap_or_default()))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::OwnerOf {
            token_id,
            include_expired,
        } => {
            let token = load_token(deps, &token_id)?;
            to_json_binary(&owner_of(&token, &env, include_expired))
        }
        QueryMsg::Approval {
            token_id,
            spender,
            include_expired,
        } => {
            let token = load_token(deps, &token_id)?;
            let spender_addr = deps.api.addr_validate(&spender)?;
            let include = include_expired.unwrap_or(false);
            let found = token
                .approvals
                .iter()
                .find(|a| a.spender == spender_addr)
                .ok_or_else(|| err("approval not found"))?;
            if !include && found.expires.is_expired(&env.block) {
                return Err(err("approval not found"));
            }
            to_json_binary(&ApprovalResponse {
                approval: Approval {
                    spender,
                    expires: found.expires,
                },
            })
        }
        QueryMsg::Approvals {
            token_id,
            include_expired,
        } => {
            let token = load_token(deps, &token_id)?;
            to_json_binary(&ApprovalsResponse {
                approvals: visible_approvals(&token, &env, include_expired),
            })
        }
        QueryMsg::AllOperators {
            owner,
            include_expired,
            start_after,
            limit,
        } => {
            let owner = deps.api.addr_validate(&owner)?;
            let include = include_expired.unwrap_or(false);
            let start_addr = match start_after {
                Some(op) => Some(deps.api.addr_validate(&op)?),
                None => None,
            };
            let start = start_addr.as_ref().map(Bound::exclusive);
            let mut operators = Vec::new();
            for item in OPERATORS.prefix(&owner).range(
                deps.storage,
                start,
                None,
                cosmwasm_std::Order::Ascending,
            ) {
                let (op, expires) = item?;
                if include || !expires.is_expired(&env.block) {
                    operators.push(Approval {
                        spender: op.to_string(),
                        expires,
                    });
                }
                if operators.len() >= page_limit(limit) {
                    break;
                }
            }
            to_json_binary(&OperatorsResponse { operators })
        }
        QueryMsg::NumTokens {} => to_json_binary(&NumTokensResponse {
            count: NUM_TOKENS.load(deps.storage)?,
        }),
        QueryMsg::ContractInfo {} => {
            let cfg = CONFIG.load(deps.storage)?;
            to_json_binary(&ContractInfoResponse {
                name: cfg.name,
                symbol: cfg.symbol,
            })
        }
        QueryMsg::NftInfo { token_id } => {
            let token = load_token(deps, &token_id)?;
            to_json_binary(&NftInfoResponse {
                token_uri: token.token_uri,
                extension: Empty {},
            })
        }
        QueryMsg::AllNftInfo {
            token_id,
            include_expired,
        } => {
            let token = load_token(deps, &token_id)?;
            to_json_binary(&AllNftInfoResponse {
                access: owner_of(&token, &env, include_expired),
                info: NftInfoResponse {
                    token_uri: token.token_uri,
                    extension: Empty {},
                },
            })
        }
        QueryMsg::Tokens {
            owner,
            start_after,
            limit,
        } => {
            let owner = deps.api.addr_validate(&owner)?;
            let start = start_after.as_deref().map(Bound::exclusive);
            let tokens = OWNED
                .prefix(&owner)
                .keys(deps.storage, start, None, cosmwasm_std::Order::Ascending)
                .take(page_limit(limit))
                .collect::<StdResult<Vec<_>>>()?;
            to_json_binary(&TokensResponse { tokens })
        }
        QueryMsg::AllTokens { start_after, limit } => {
            let start = start_after.as_deref().map(Bound::exclusive);
            let tokens = TOKENS
                .keys(deps.storage, start, None, cosmwasm_std::Order::Ascending)
                .take(page_limit(limit))
                .collect::<StdResult<Vec<_>>>()?;
            to_json_binary(&TokensResponse { tokens })
        }
        QueryMsg::Minter {} => {
            let minter = MINTER.load(deps.storage)?;
            to_json_binary(&MinterResponse {
                minter: minter.map(|a| a.to_string()),
            })
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
    use cosmwasm_std::{from_json, CosmosMsg};

    fn setup() -> (
        cosmwasm_std::OwnedDeps<
            cosmwasm_std::MemoryStorage,
            cosmwasm_std::testing::MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
        Addr,
        Addr,
    ) {
        let mut deps = mock_dependencies();
        let minter = deps.api.addr_make("minter");
        let owner = deps.api.addr_make("owner");
        instantiate(
            deps.as_mut(),
            mock_env(),
            message_info(&minter, &[]),
            InstantiateMsg {
                name: "Mesh Badge v1".into(),
                symbol: "MESH".into(),
                minter: minter.to_string(),
            },
        )
        .unwrap();
        (deps, minter, owner)
    }

    fn mint(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::MemoryStorage,
            cosmwasm_std::testing::MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
        minter: &Addr,
        owner: &Addr,
        token_id: &str,
    ) {
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(minter, &[]),
            ExecuteMsg::Mint {
                token_id: token_id.into(),
                owner: owner.to_string(),
                token_uri: Some("ipfs://badge".into()),
            },
        )
        .unwrap();
    }

    #[test]
    fn warning_text_is_the_required_sentence() {
        assert_eq!(
            NOT_AUDITED_WARNING,
            "Dieser Contract ist nicht auditiert. Einsatz auf eigenes Risiko. Keine Garantie für Fonds- oder NFT-Sicherheit. Testnet/kleine Mengen zuerst."
        );
    }

    #[test]
    fn mint_transfer_and_queries() {
        let (mut deps, minter, owner) = setup();
        let stranger = deps.api.addr_make("stranger");
        let err_mint = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&stranger, &[]),
            ExecuteMsg::Mint {
                token_id: "1".into(),
                owner: owner.to_string(),
                token_uri: None,
            },
        )
        .unwrap_err();
        assert!(err_mint.to_string().contains("not minter"));
        mint(&mut deps, &minter, &owner, "1");
        let dup = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&minter, &[]),
            ExecuteMsg::Mint {
                token_id: "1".into(),
                owner: owner.to_string(),
                token_uri: None,
            },
        )
        .unwrap_err();
        assert!(dup.to_string().contains("token exists"));

        let bin = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::OwnerOf {
                token_id: "1".into(),
                include_expired: None,
            },
        )
        .unwrap();
        let owner_of: OwnerOfResponse = from_json(bin).unwrap();
        assert_eq!(owner_of.owner, owner.to_string());
        let info: NftInfoResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::NftInfo {
                    token_id: "1".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(info.token_uri.unwrap(), "ipfs://badge");
        let count: NumTokensResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::NumTokens {}).unwrap()).unwrap();
        assert_eq!(count.count, 1);

        let recipient = deps.api.addr_make("recipient");
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &[]),
            ExecuteMsg::TransferNft {
                recipient: recipient.to_string(),
                token_id: "1".into(),
            },
        )
        .unwrap();
        let owner_of: OwnerOfResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::OwnerOf {
                    token_id: "1".into(),
                    include_expired: None,
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(owner_of.owner, recipient.to_string());
        let tokens: TokensResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Tokens {
                    owner: recipient.to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(tokens.tokens, vec!["1".to_string()]);
    }

    #[test]
    fn approve_transfer_and_expired_approval() {
        let (mut deps, minter, owner) = setup();
        mint(&mut deps, &minter, &owner, "7");
        let spender = deps.api.addr_make("spender");
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &[]),
            ExecuteMsg::Approve {
                spender: spender.to_string(),
                token_id: "7".into(),
                expires: Some(Expiration::Never {}),
            },
        )
        .unwrap();
        let recipient = deps.api.addr_make("recipient");
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&spender, &[]),
            ExecuteMsg::TransferNft {
                recipient: recipient.to_string(),
                token_id: "7".into(),
            },
        )
        .unwrap();
        let owner_of: OwnerOfResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::OwnerOf {
                    token_id: "7".into(),
                    include_expired: Some(true),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(owner_of.owner, recipient.to_string());
        assert!(owner_of.approvals.is_empty());

        mint(&mut deps, &minter, &owner, "8");
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &[]),
            ExecuteMsg::Approve {
                spender: spender.to_string(),
                token_id: "8".into(),
                expires: Some(Expiration::AtHeight(1)),
            },
        )
        .unwrap();
        let denied = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&spender, &[]),
            ExecuteMsg::TransferNft {
                recipient: recipient.to_string(),
                token_id: "8".into(),
            },
        )
        .unwrap_err();
        assert!(denied.to_string().contains("unauthorized"));
    }

    #[test]
    fn operator_can_transfer_and_burn_clears_supply() {
        let (mut deps, minter, owner) = setup();
        mint(&mut deps, &minter, &owner, "9");
        let operator = deps.api.addr_make("operator");
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &[]),
            ExecuteMsg::ApproveAll {
                operator: operator.to_string(),
                expires: None,
            },
        )
        .unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&operator, &[]),
            ExecuteMsg::Burn {
                token_id: "9".into(),
            },
        )
        .unwrap();
        let missing = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::OwnerOf {
                token_id: "9".into(),
                include_expired: None,
            },
        )
        .unwrap_err();
        assert!(missing.to_string().contains("token not found"));
        let count: NumTokensResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::NumTokens {}).unwrap()).unwrap();
        assert_eq!(count.count, 0);
        let denied = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&operator, &[]),
            ExecuteMsg::Burn {
                token_id: "9".into(),
            },
        )
        .unwrap_err();
        assert!(denied.to_string().contains("token not found"));
    }

    #[test]
    fn send_nft_names_owner_not_operator() {
        let (mut deps, minter, owner) = setup();
        mint(&mut deps, &minter, &owner, "badge-1");
        let operator = deps.api.addr_make("operator");
        let market = deps.api.addr_make("market");
        execute(
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &[]),
            ExecuteMsg::ApproveAll {
                operator: operator.to_string(),
                expires: None,
            },
        )
        .unwrap();
        let res = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&operator, &[]),
            ExecuteMsg::SendNft {
                contract: market.to_string(),
                token_id: "badge-1".into(),
                msg: Binary::from(
                    br#"{"list":{"price":{"denom":"aqcoin","amount":"1000000"}}}"#.to_vec(),
                ),
            },
        )
        .unwrap();
        assert_eq!(
            res.attributes
                .iter()
                .find(|a| a.key == "sender")
                .unwrap()
                .value,
            owner.to_string()
        );
        let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds,
        }) = &res.messages[0].msg
        else {
            panic!("expected wasm message");
        };
        assert_eq!(contract_addr, &market.to_string());
        assert!(funds.is_empty());
        let hook: ReceiveNftHook = from_json(msg).unwrap();
        assert_eq!(hook.receive_nft.sender, owner.to_string());
        assert_eq!(hook.receive_nft.token_id, "badge-1");
        let owner_of: OwnerOfResponse = from_json(
            query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::OwnerOf {
                    token_id: "badge-1".into(),
                    include_expired: None,
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(owner_of.owner, market.to_string());
    }

    #[test]
    fn rejects_funds_and_warns_on_instantiate() {
        let mut deps = mock_dependencies();
        let minter = deps.api.addr_make("minter");
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            message_info(&minter, &[]),
            InstantiateMsg {
                name: "Mesh Badge v1".into(),
                symbol: "MESH".into(),
                minter: minter.to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            res.attributes
                .iter()
                .find(|a| a.key == "not_audited")
                .unwrap()
                .value,
            "true"
        );
        let warning: WarningResponse =
            from_json(query(deps.as_ref(), mock_env(), QueryMsg::Warning {}).unwrap()).unwrap();
        assert!(warning.not_audited);
        assert_eq!(warning.warning, NOT_AUDITED_WARNING);
        let owner = deps.api.addr_make("owner");
        let paid = execute(
            deps.as_mut(),
            mock_env(),
            message_info(&minter, &cosmwasm_std::coins(1, "aqcoin")),
            ExecuteMsg::Mint {
                token_id: "1".into(),
                owner: owner.to_string(),
                token_uri: None,
            },
        )
        .unwrap_err();
        assert!(paid.to_string().contains("no funds expected"));
    }
}
