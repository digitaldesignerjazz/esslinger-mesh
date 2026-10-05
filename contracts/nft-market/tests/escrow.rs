//! cw-multi-test escrow scenarios: mint, transfer, list, buy, cancel.
use cosmwasm_std::{coins, to_json_binary, Addr, Coin};
use cw_multi_test::{App, ContractWrapper, Executor};
use esslinger_mesh_cw721 as nft;
use esslinger_mesh_nft_market as market;

const XCOIN: &str = "ibc/249B1BA7E248694683603187D43174EA80EC25BA1E36149D7980133DCA99C22C";

struct World {
    app: App,
    admin: Addr,
    minter: Addr,
    seller: Addr,
    buyer: Addr,
    buyer2: Addr,
    operator: Addr,
    fee: Addr,
    stranger: Addr,
    nft: Addr,
    market: Addr,
}

fn world() -> World {
    let api = cosmwasm_std::testing::MockApi::default();
    let admin = api.addr_make("admin");
    let minter = api.addr_make("minter");
    let seller = api.addr_make("seller");
    let buyer = api.addr_make("buyer");
    let buyer2 = api.addr_make("buyer2");
    let operator = api.addr_make("operator");
    let fee = api.addr_make("fee");
    let stranger = api.addr_make("stranger");
    let purse = vec![
        Coin::new(1_000_000u128, "aqcoin"),
        Coin::new(1_000_000u128, XCOIN),
    ];
    let mut app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &buyer, purse.clone())
            .unwrap();
        router
            .bank
            .init_balance(storage, &buyer2, purse.clone())
            .unwrap();
        router
            .bank
            .init_balance(storage, &seller, coins(1u128, "aqstake"))
            .unwrap();
    });
    let nft_code = app.store_code(Box::new(ContractWrapper::new(
        nft::execute,
        nft::instantiate,
        nft::query,
    )));
    let market_code = app.store_code(Box::new(ContractWrapper::new(
        market::execute,
        market::instantiate,
        market::query,
    )));
    let nft_addr = app
        .instantiate_contract(
            nft_code,
            minter.clone(),
            &nft::InstantiateMsg {
                name: "Mesh Badge v1".into(),
                symbol: "MESH".into(),
                minter: minter.to_string(),
            },
            &[],
            "esslinger-mesh-cw721",
            Some(admin.to_string()),
        )
        .unwrap();
    let market_addr = app
        .instantiate_contract(
            market_code,
            admin.clone(),
            &market::InstantiateMsg {
                admin: admin.to_string(),
                fee_bps: 30,
                fee_recipient: fee.to_string(),
                allowed_denoms: vec!["aqcoin".into(), XCOIN.into()],
                nft_contracts: vec![nft_addr.to_string()],
            },
            &[],
            "esslinger-mesh-nft-market",
            Some(admin.to_string()),
        )
        .unwrap();
    World {
        app,
        admin,
        minter,
        seller,
        buyer,
        buyer2,
        operator,
        fee,
        stranger,
        nft: nft_addr,
        market: market_addr,
    }
}

fn mint(w: &mut World, token_id: &str) {
    w.app
        .execute_contract(
            w.minter.clone(),
            w.nft.clone(),
            &nft::ExecuteMsg::Mint {
                token_id: token_id.into(),
                owner: w.seller.to_string(),
                token_uri: Some("ipfs://mesh-badge".into()),
            },
            &[],
        )
        .unwrap();
}

fn owner_of(w: &World, token_id: &str) -> String {
    let resp: nft::OwnerOfResponse = w
        .app
        .wrap()
        .query_wasm_smart(
            w.nft.to_string(),
            &nft::QueryMsg::OwnerOf {
                token_id: token_id.into(),
                include_expired: None,
            },
        )
        .unwrap();
    resp.owner
}

fn listing(w: &World, token_id: &str) -> Option<market::ListingInfo> {
    let resp: market::ListingResponse = w
        .app
        .wrap()
        .query_wasm_smart(
            w.market.to_string(),
            &market::QueryMsg::Listing {
                nft_contract: w.nft.to_string(),
                token_id: token_id.into(),
            },
        )
        .unwrap();
    resp.listing
}

fn shows(err: impl std::fmt::Debug, needle: &str) {
    let text = format!("{err:?}");
    assert!(text.contains(needle), "{text}");
}

fn bal(w: &World, addr: &Addr, denom: &str) -> u128 {
    w.app
        .wrap()
        .query_balance(addr, denom)
        .unwrap()
        .amount
        .u128()
}

macro_rules! send_list_as {
    ($w:ident, $who:ident, $id:expr, $price:expr) => {{
        let from = $w.$who.clone();
        send_list(&mut $w, from, $id, $price);
    }};
}

fn send_list(w: &mut World, from: Addr, token_id: &str, price: Coin) {
    let msg = to_json_binary(&market::HookMsg::List { price }).unwrap();
    w.app
        .execute_contract(
            from,
            w.nft.clone(),
            &nft::ExecuteMsg::SendNft {
                contract: w.market.to_string(),
                token_id: token_id.into(),
                msg,
            },
            &[],
        )
        .unwrap();
}

#[test]
fn send_list_buy_pays_seller_and_fee_and_empties_escrow() {
    let mut w = world();
    mint(&mut w, "badge-1");
    assert_eq!(owner_of(&w, "badge-1"), w.seller.to_string());
    w.app
        .execute_contract(
            w.seller.clone(),
            w.nft.clone(),
            &nft::ExecuteMsg::TransferNft {
                recipient: w.operator.to_string(),
                token_id: "badge-1".into(),
            },
            &[],
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-1"), w.operator.to_string());
    w.app
        .execute_contract(
            w.operator.clone(),
            w.nft.clone(),
            &nft::ExecuteMsg::TransferNft {
                recipient: w.seller.to_string(),
                token_id: "badge-1".into(),
            },
            &[],
        )
        .unwrap();

    send_list_as!(w, seller, "badge-1", Coin::new(10_000u128, "aqcoin"));
    assert_eq!(owner_of(&w, "badge-1"), w.market.to_string());
    let listed = listing(&w, "badge-1").unwrap();
    assert_eq!(listed.seller, w.seller.to_string());
    assert_eq!(listed.price, Coin::new(10_000u128, "aqcoin"));

    w.app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-1".into(),
            },
            &coins(10_000u128, "aqcoin"),
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-1"), w.buyer.to_string());
    assert!(listing(&w, "badge-1").is_none());
    assert_eq!(bal(&w, &w.seller, "aqcoin"), 9970);
    assert_eq!(bal(&w, &w.fee, "aqcoin"), 30);
    assert_eq!(bal(&w, &w.buyer, "aqcoin"), 990_000);
    assert_eq!(bal(&w, &w.market, "aqcoin"), 0);

    let double = w
        .app
        .execute_contract(
            w.buyer2.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-1".into(),
            },
            &coins(10_000u128, "aqcoin"),
        )
        .unwrap_err();
    shows(double, "listing not found");
    assert_eq!(bal(&w, &w.buyer2, "aqcoin"), 1_000_000);
    assert_eq!(owner_of(&w, "badge-1"), w.buyer.to_string());
}

#[test]
fn cancel_returns_nft_to_seller() {
    let mut w = world();
    mint(&mut w, "badge-2");
    send_list_as!(w, seller, "badge-2", Coin::new(10_000u128, "aqcoin"));
    let denied = w
        .app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Cancel {
                nft_contract: w.nft.to_string(),
                token_id: "badge-2".into(),
            },
            &[],
        )
        .unwrap_err();
    shows(denied, "only seller can cancel");
    w.app
        .execute_contract(
            w.seller.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Cancel {
                nft_contract: w.nft.to_string(),
                token_id: "badge-2".into(),
            },
            &[],
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-2"), w.seller.to_string());
    assert!(listing(&w, "badge-2").is_none());
    assert_eq!(bal(&w, &w.buyer, "aqcoin"), 1_000_000);
}

#[test]
fn approve_list_buy_and_rejects_unapproved_or_third_party() {
    let mut w = world();
    mint(&mut w, "badge-3");
    let unapproved = w
        .app
        .execute_contract(
            w.seller.clone(),
            w.market.clone(),
            &market::ExecuteMsg::List {
                nft_contract: w.nft.to_string(),
                token_id: "badge-3".into(),
                price: Coin::new(4_000u128, "aqcoin"),
            },
            &[],
        )
        .unwrap_err();
    shows(unapproved, "unauthorized");
    assert_eq!(owner_of(&w, "badge-3"), w.seller.to_string());
    assert!(listing(&w, "badge-3").is_none());

    w.app
        .execute_contract(
            w.seller.clone(),
            w.nft.clone(),
            &nft::ExecuteMsg::Approve {
                spender: w.market.to_string(),
                token_id: "badge-3".into(),
                expires: Some(nft::Expiration::Never {}),
            },
            &[],
        )
        .unwrap();
    let hijack = w
        .app
        .execute_contract(
            w.stranger.clone(),
            w.market.clone(),
            &market::ExecuteMsg::List {
                nft_contract: w.nft.to_string(),
                token_id: "badge-3".into(),
                price: Coin::new(4_000u128, "aqcoin"),
            },
            &[],
        )
        .unwrap_err();
    shows(hijack, "only owner can list");
    assert!(listing(&w, "badge-3").is_none());

    w.app
        .execute_contract(
            w.seller.clone(),
            w.market.clone(),
            &market::ExecuteMsg::List {
                nft_contract: w.nft.to_string(),
                token_id: "badge-3".into(),
                price: Coin::new(4_000u128, "aqcoin"),
            },
            &[],
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-3"), w.market.to_string());
    w.app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-3".into(),
            },
            &coins(4_000u128, "aqcoin"),
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-3"), w.buyer.to_string());
    assert_eq!(bal(&w, &w.seller, "aqcoin"), 3_988);
    assert_eq!(bal(&w, &w.fee, "aqcoin"), 12);
    assert_eq!(bal(&w, &w.market, "aqcoin"), 0);
}

#[test]
fn operator_send_pays_the_owner() {
    let mut w = world();
    mint(&mut w, "badge-4");
    w.app
        .execute_contract(
            w.seller.clone(),
            w.nft.clone(),
            &nft::ExecuteMsg::ApproveAll {
                operator: w.operator.to_string(),
                expires: None,
            },
            &[],
        )
        .unwrap();
    send_list_as!(w, operator, "badge-4", Coin::new(10_000u128, "aqcoin"));
    assert_eq!(listing(&w, "badge-4").unwrap().seller, w.seller.to_string());
    w.app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-4".into(),
            },
            &coins(10_000u128, "aqcoin"),
        )
        .unwrap();
    assert_eq!(bal(&w, &w.seller, "aqcoin"), 9970);
    assert_eq!(bal(&w, &w.operator, "aqcoin"), 0);
}

#[test]
fn xcoin_buy_and_wrong_denom_rolls_back() {
    let mut w = world();
    mint(&mut w, "badge-5");
    let bad = w.app.execute_contract(
        w.seller.clone(),
        w.nft.clone(),
        &nft::ExecuteMsg::SendNft {
            contract: w.market.to_string(),
            token_id: "badge-5".into(),
            msg: to_json_binary(&market::HookMsg::List {
                price: Coin::new(10_000u128, "ustake"),
            })
            .unwrap(),
        },
        &[],
    );
    shows(bad.unwrap_err(), "unexpected denom");
    assert_eq!(owner_of(&w, "badge-5"), w.seller.to_string());

    send_list_as!(w, seller, "badge-5", Coin::new(10_000u128, XCOIN));
    let wrong = w
        .app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-5".into(),
            },
            &coins(10_000u128, "aqcoin"),
        )
        .unwrap_err();
    shows(wrong, "unexpected denom");
    assert_eq!(owner_of(&w, "badge-5"), w.market.to_string());
    w.app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-5".into(),
            },
            &coins(10_000u128, XCOIN),
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-5"), w.buyer.to_string());
    assert_eq!(bal(&w, &w.seller, XCOIN), 9970);
    assert_eq!(bal(&w, &w.fee, XCOIN), 30);
    assert_eq!(bal(&w, &w.market, XCOIN), 0);
    assert_eq!(bal(&w, &w.buyer, "aqcoin"), 1_000_000);
}

#[test]
fn pause_blocks_buy_cancel_still_returns_nft() {
    let mut w = world();
    mint(&mut w, "badge-6");
    send_list_as!(w, seller, "badge-6", Coin::new(10_000u128, "aqcoin"));
    w.app
        .execute_contract(
            w.admin.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Pause {},
            &[],
        )
        .unwrap();
    let paused = w
        .app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-6".into(),
            },
            &coins(10_000u128, "aqcoin"),
        )
        .unwrap_err();
    shows(paused, "paused");
    w.app
        .execute_contract(
            w.seller.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Cancel {
                nft_contract: w.nft.to_string(),
                token_id: "badge-6".into(),
            },
            &[],
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-6"), w.seller.to_string());
    assert_eq!(bal(&w, &w.buyer, "aqcoin"), 1_000_000);
}

#[test]
fn rescue_unlisted_transfer_and_refuses_a_live_listing() {
    let mut w = world();
    mint(&mut w, "badge-7");
    w.app
        .execute_contract(
            w.seller.clone(),
            w.nft.clone(),
            &nft::ExecuteMsg::TransferNft {
                recipient: w.market.to_string(),
                token_id: "badge-7".into(),
            },
            &[],
        )
        .unwrap();
    assert!(listing(&w, "badge-7").is_none());
    assert_eq!(owner_of(&w, "badge-7"), w.market.to_string());
    let denied = w
        .app
        .execute_contract(
            w.stranger.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Rescue {
                nft_contract: w.nft.to_string(),
                token_id: "badge-7".into(),
                recipient: w.stranger.to_string(),
            },
            &[],
        )
        .unwrap_err();
    shows(denied, "not admin");
    w.app
        .execute_contract(
            w.admin.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Rescue {
                nft_contract: w.nft.to_string(),
                token_id: "badge-7".into(),
                recipient: w.seller.to_string(),
            },
            &[],
        )
        .unwrap();
    assert_eq!(owner_of(&w, "badge-7"), w.seller.to_string());

    send_list_as!(w, seller, "badge-7", Coin::new(1u128, "aqcoin"));
    let blocked = w
        .app
        .execute_contract(
            w.admin.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Rescue {
                nft_contract: w.nft.to_string(),
                token_id: "badge-7".into(),
                recipient: w.admin.to_string(),
            },
            &[],
        )
        .unwrap_err();
    shows(blocked, "listing exists");
    assert_eq!(owner_of(&w, "badge-7"), w.market.to_string());
}

#[test]
fn buy_without_funds_keeps_the_listing() {
    let mut w = world();
    mint(&mut w, "badge-8");
    send_list_as!(w, seller, "badge-8", Coin::new(10_000u128, "aqcoin"));
    let err = w
        .app
        .execute_contract(
            w.buyer.clone(),
            w.market.clone(),
            &market::ExecuteMsg::Buy {
                nft_contract: w.nft.to_string(),
                token_id: "badge-8".into(),
            },
            &[],
        )
        .unwrap_err();
    shows(err, "send exactly the listing price");
    assert_eq!(owner_of(&w, "badge-8"), w.market.to_string());
    assert!(listing(&w, "badge-8").is_some());
}
