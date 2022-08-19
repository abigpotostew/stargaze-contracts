use cosmwasm_std::{Addr, coin, coins, Decimal, Timestamp, Uint128};
use cosmwasm_std::{Api, Coin};
use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
use cw721::{Cw721QueryMsg, OwnerOfResponse};
use cw_multi_test::{BankSudo, Contract, ContractWrapper, Executor, SudoMsg};
use sg_multi_test::StargazeApp;
use sg_std::{GENESIS_MINT_START_TIME, NATIVE_DENOM, StargazeMsgWrapper};

use sg721_imago::msg::{CodeUriResponse, InstantiateMsg as Sg721InstantiateMsg, QueryMsg as Sg721ImagoQueryMsg, RoyaltyInfoResponse};
use sg721_imago::state::CollectionInfo;

use crate::contract::instantiate;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MintableNumTokensResponse, MintCountResponse,
    QueryMsg, StartTimeResponse,
};

const CREATION_FEE: u128 = 1_000_000_000;
const INITIAL_BALANCE: u128 = 2_000_000_000;

const UNIT_PRICE: u128 = 100_000_000;
const MINT_FEE: u128 = 10_000_000;
const PW_MINT_FEE: u128 = 15_000_000;
const DEV_FEE: u128 = 1_000_000;
const MAX_TOKEN_LIMIT: u32 = 10000;
const WHITELIST_AMOUNT: u128 = 66_000_000;
const WL_PER_ADDRESS_LIMIT: u32 = 1;
const ADMIN_MINT_PRICE: u128 = 15_000_000;

fn custom_mock_app() -> StargazeApp {
    StargazeApp::default()
}

pub fn contract_whitelist() -> Box<dyn Contract<StargazeMsgWrapper>> {
    let contract = ContractWrapper::new(
        whitelist::contract::execute,
        whitelist::contract::instantiate,
        whitelist::contract::query,
    );
    Box::new(contract)
}

pub fn contract_minter() -> Box<dyn Contract<StargazeMsgWrapper>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
        .with_reply(crate::contract::reply);
    Box::new(contract)
}

pub fn contract_sg721() -> Box<dyn Contract<StargazeMsgWrapper>> {
    let contract = ContractWrapper::new(
        sg721_imago::contract::execute,
        sg721_imago::contract::instantiate,
        sg721_imago::contract::query,
    );
    Box::new(contract)
}


// Upload contract code and instantiate minter contract
fn setup_minter_contract(
    router: &mut StargazeApp,
    creator: &Addr,
    num_tokens: u32,
) -> (Addr, ConfigResponse) {
    // Upload contract code
    let sg721_code_id = router.store_code(contract_sg721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    // Instantiate minter contract
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: creator.to_string(),
            finalizer: creator.to_string(),
            code_uri: "ipfs://test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: creator.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: creator.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    let minter_addr = router
        .instantiate_contract(
            minter_code_id,
            // failing here
            creator.clone(),
            &msg,
            &creation_fee,
            "Minter Imago",
            None,
        )
        .unwrap();

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    (minter_addr, config)
}

// Add a creator account with initial balances
fn setup_accounts(router: &mut StargazeApp) -> (Addr, Addr) {
    let buyer = Addr::unchecked("buyer");
    let creator = Addr::unchecked("creator");
    // 3,000 tokens
    let creator_funds = coins(INITIAL_BALANCE + CREATION_FEE, NATIVE_DENOM);
    // 2,000 tokens
    let buyer_funds = coins(INITIAL_BALANCE, NATIVE_DENOM);
    router
        .sudo(SudoMsg::Bank({
            BankSudo::Mint {
                to_address: creator.to_string(),
                amount: creator_funds.clone(),
            }
        }))
        .map_err(|err| println!("{:?}", err))
        .ok();

    router
        .sudo(SudoMsg::Bank({
            BankSudo::Mint {
                to_address: buyer.to_string(),
                amount: buyer_funds.clone(),
            }
        }))
        .map_err(|err| println!("{:?}", err))
        .ok();

    // Check native balances
    let creator_native_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(creator_native_balances, creator_funds);

    // Check native balances
    let buyer_native_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(buyer_native_balances, buyer_funds);

    (creator, buyer)
}

// Set blockchain time to after mint by default
fn setup_block_time(router: &mut StargazeApp, nanos: u64) {
    let mut block = router.block_info();
    block.time = Timestamp::from_nanos(nanos);
    router.set_block(block);
}

// Deal with zero and non-zero coin amounts for msgs
fn coins_for_msg(msg_coin: Coin) -> Vec<Coin> {
    if msg_coin.amount > Uint128::zero() {
        vec![msg_coin]
    } else {
        vec![]
    }
}

#[test]
fn initialization() {
    let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

    // Check valid addr
    let addr = "earth1";
    let res = deps.api.addr_validate(&(*addr));
    assert!(res.is_ok());

    // 0 per address limit returns error
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 0,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1234".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "ipfs://test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Invalid base token uri returns error
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "a".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Invalid base token uri returns error -- not https protocol
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "a".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "http://metadata.publicworks.art/2".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Invalid base toke uri-- not public works url
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.aart/1".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Invalid code token uri returns error
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "uri_missing_protocol".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Invalid denom returns error
    let wrong_denom = "uosmo";
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, wrong_denom),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "ipfs://test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Insufficient mint price returns error
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(1, NATIVE_DENOM),
        num_tokens: 100,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "ipfs://test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Over max token limit
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: (MAX_TOKEN_LIMIT + 1),
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "ipfs://test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Under min token limit
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        num_tokens: 0,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id: 1,
        sg721_instantiate_msg: Sg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            finalizer: info.sender.to_string(),
            code_uri: "ipfs://test_code_url".to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        },
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
}

#[test]
fn happy_path() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, GENESIS_MINT_START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 2;
    let (minter_addr, config) = setup_minter_contract(&mut router, &creator, num_tokens);

    // Default start time genesis mint time
    let res: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
        .unwrap();
    assert_eq!(
        res.start_time,
        Timestamp::from_nanos(GENESIS_MINT_START_TIME).to_string()
    );

    setup_block_time(&mut router, GENESIS_MINT_START_TIME + 1);

    // Fail with incorrect tokens
    let mint_msg = ExecuteMsg::Mint {};
    let err = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE + 100, NATIVE_DENOM),
    );
    assert!(err.is_err());

    // Succeeds if funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Balances are correct
    // The creator should get the unit price - mint fee for the mint above
    let creator_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(creator_balances, coins(INITIAL_BALANCE, NATIVE_DENOM));
    // The buyer's tokens should reduce by unit price
    let buyer_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(
        buyer_balances,
        coins(INITIAL_BALANCE - UNIT_PRICE, NATIVE_DENOM)
    );

    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.address, buyer.to_string());

    // Check NFT is transferred
    let query_owner_msg = Cw721QueryMsg::OwnerOf {
        token_id: String::from("1"),
        include_expired: None,
    };
    let res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(config.sg721_address.clone(), &query_owner_msg)
        .unwrap();
    assert_eq!(res.owner, buyer.to_string());

    // Buyer can't call MintTo
    let mint_to_msg = ExecuteMsg::MintTo {
        recipient: buyer.to_string(),
    };
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_to_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_err());

    // Creator mints an extra NFT for the buyer (who is a friend)
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &mint_to_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_ok());

    // Mint count is not increased if admin mints for the user
    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.address, buyer.to_string());

    // Minter contract should have a balance
    let minter_balance = router
        .wrap()
        .query_all_balances(minter_addr.clone())
        .unwrap();
    assert_eq!(1, minter_balance.len());
    assert_eq!(minter_balance[0].amount.u128(), UNIT_PRICE - MINT_FEE - PW_MINT_FEE); // not sure why it's less than this?

    // Minter contract should have a balance
    let pw_balance = router
        .wrap()
        .query_all_balances(minter_addr.clone())
        .unwrap();
    assert_eq!(1, minter_balance.len());
    assert_eq!(minter_balance[0].amount.u128(), UNIT_PRICE - MINT_FEE );//- PW_MINT_FEE

    // Check that NFT is transferred
    let query_owner_msg = Cw721QueryMsg::OwnerOf {
        token_id: String::from("1"),
        include_expired: None,
    };
    let res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(config.sg721_address.clone(), &query_owner_msg)
        .unwrap();
    assert_eq!(res.owner, buyer.to_string());

    // Errors if sold out
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer,
        minter_addr.clone(),
        &mint_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(UNIT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_err());

    // Creator can't use MintTo if sold out
    let res = router.execute_contract(
        creator,
        minter_addr,
        &mint_to_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_err());

    // Check code URI
    let res: CodeUriResponse = router
        .wrap()
        .query_wasm_smart(
            config.sg721_address.clone(),
            &Sg721ImagoQueryMsg::CodeUri {},
        )
        .unwrap();
    assert_eq!(res.code_uri, "ipfs://test_code_url");
}


#[test]
fn burn_remaining() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, GENESIS_MINT_START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 2;
    let (minter_addr, _) = setup_minter_contract(&mut router, &creator, num_tokens);

    // Default start time genesis mint time
    let res: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
        .unwrap();
    assert_eq!(
        res.start_time,
        Timestamp::from_nanos(GENESIS_MINT_START_TIME).to_string()
    );

    setup_block_time(&mut router, GENESIS_MINT_START_TIME + 1);

    // Fail with incorrect tokens
    let mint_msg = ExecuteMsg::Mint {};
    let err = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE + 100, NATIVE_DENOM),
    );
    assert!(err.is_err());

    // Succeeds if funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());


    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.address, buyer.to_string());

    // Errors if burn remaining as buyer
    let burn_msg = ExecuteMsg::BurnRemaining {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &burn_msg,
        &[],
    );
    assert!(res.is_err());

    // check num mintable tokens is unchanged
    let res: MintableNumTokensResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintableNumTokens {},
        )
        .unwrap();
    assert_eq!(res.count, 1);

    // Allow burn remaining as creator
    let mint_msg = ExecuteMsg::BurnRemaining {};
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &mint_msg,
        &[],
    );
    assert!(res.is_ok());

    // check num mintable tokens is zero
    let res: MintableNumTokensResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintableNumTokens {},
        )
        .unwrap();
    assert_eq!(res.count, 0);
}
