use cosmwasm_std::{Addr, coin, coins, Decimal, Timestamp, Uint128};
use cosmwasm_std::{Api, Coin};
use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
use cw721::{Cw721QueryMsg, OwnerOfResponse};
use cw_multi_test::{BankSudo, Contract, ContractWrapper, Executor, SudoMsg};
use sg_multi_test::StargazeApp;
use sg_std::{GENESIS_MINT_START_TIME, NATIVE_DENOM, StargazeMsgWrapper};

use sg721_imago::msg::{CodeUriResponse, InstantiateMsg as Sg721InstantiateMsg, QueryMsg as Sg721ImagoQueryMsg, RoyaltyInfoResponse};
use sg721_imago::state::CollectionInfo;

use crate::contract::{declining_dutch_auction, discrete_gda, dutch_auction_linear_next_price_change_timestamp, dutch_auction_price_linear_decline, instantiate};
use crate::msg::{ConfigResponse, DutchAuctionConfig, ExecuteMsg, InstantiateMsg, MintableNumTokensResponse, MintCountResponse, MintPriceResponse, QueryMsg, StartTimeResponse};

const CREATION_FEE: u128 = 1_000_000_000;
const INITIAL_BALANCE: u128 = 2_000_000_000;

const UNIT_PRICE: u128 = 100_000_000;
const PW_CREATE_FEE: u128 = 100_000_000;
const MAX_TOKEN_LIMIT: u32 = 10000;
const ADMIN_MINT_PRICE: u128 = 15_000_000;

fn custom_mock_app() -> StargazeApp {
    StargazeApp::default()
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
        dutch_auction_config:None,
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

fn setup_minter_contract_dutch_auction(
    router: &mut StargazeApp,
    creator: &Addr,
    num_tokens: u32,
    end_time: u64,
    unit_price:u128,
    resting_unit_price:u128,
) -> (Addr, ConfigResponse) {
    // Upload contract code
    let sg721_code_id = router.store_code(contract_sg721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    // Instantiate minter contract
    let msg = InstantiateMsg {
        unit_price: coin(unit_price, NATIVE_DENOM),
        num_tokens,
        start_time: Timestamp::from_nanos(GENESIS_MINT_START_TIME),
        per_address_limit: 5,
        whitelist: None,
        base_token_uri: "https://metadata.publicworks.art/1".to_string(),
        sg721_code_id,
        dutch_auction_config:Some(DutchAuctionConfig{
            end_time: (Timestamp::from_nanos(end_time)),
            resting_unit_price: (coin(resting_unit_price, NATIVE_DENOM)),
            decline_period_seconds: 300,
            decline_coefficient: 850000,
        }),

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

// Set blockchain time to after mint by default
fn setup_block_time_height(router: &mut StargazeApp, nanos: u64, height: u64) {
    let mut block = router.block_info();
    block.time = Timestamp::from_nanos(nanos);
    block.height = height;
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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
        dutch_auction_config:None,
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


// #[test]
// fn dutch_auction_linear() {
//
//     let start = Timestamp::from_seconds(1675486368).seconds();
//     let end = Timestamp::from_seconds(start + 7 * 24 * 60 * 60).seconds();
//
//     let start_price = Uint128::from(100000000000u128);
//     let end_price = Uint128::from(1000000000u128);
//
//     let auction_duration = end - start;
//     let five_minutes_seconds = 5 * 60;
//     let price_diff = start_price.u128() - end_price.u128();
//     let price_drop_per_period = price_diff/(auction_duration as u128 / five_minutes_seconds);
//
//
//
//     //before it starts
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, start - 1),
//         start_price
//     );
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, start),
//         start_price
//     );
//     //after it ends, it stays at resting price
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, end),
//         end_price
//     );
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, end + 1000),
//         end_price
//     );
//
//     // during declining period price gradually decreases linearly
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, start + 299),
//         start_price
//     );
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, start + 300),
//         start_price - Uint128::from(price_drop_per_period)
//     );
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, start + 599),
//         start_price - Uint128::from(price_drop_per_period)
//     );
//     assert_eq!(
//         dutch_auction_price_linear_decline(start, end, start_price, end_price, start + 600),
//         start_price - Uint128::from(price_drop_per_period * 2)
//     );
//
//
//
//
//     // check the timestamp of the next price change
//     assert_eq!(
//         dutch_auction_linear_next_price_change_timestamp(start, end, start - 10000),
//         start
//     );
//     assert_eq!(
//         dutch_auction_linear_next_price_change_timestamp(start, end, start),
//         start + 300
//     );
//     assert_eq!(
//         dutch_auction_linear_next_price_change_timestamp(start, end, start + 1),
//         start + 300
//     );
//     assert_eq!(
//         dutch_auction_linear_next_price_change_timestamp(start, end, end),
//         end
//     );
//     assert_eq!(
//         dutch_auction_linear_next_price_change_timestamp(start, end, end-1),
//         end
//     );
//     assert_eq!(
//         dutch_auction_linear_next_price_change_timestamp(start, end, end+1),
//         end
//     );
// }

// #[test]
// fn dutch_auction_gda() {
//     let start = Timestamp::from_seconds(1675486368).seconds();
//     let end = Timestamp::from_seconds(start + 7 * 24 * 60 * 60).seconds();
//
//     let duration = end - start;
//     let start_price = Uint128::from(100000000000u128);
//     let end_price = Uint128::from(1000000000u128);
//
//     // let auction_duration = end - start;
//     // let five_minutes_seconds = 5 * 60;
//     // let price_diff = start_price.u128() - end_price.u128();
//     // let price_drop_per_period = price_diff/(auction_duration as u128 / five_minutes_seconds);
//
//     assert_eq!(
//         discrete_gda(1, 0, start, end, start_price, end_price, start),
//         start_price
//     );
//     assert_eq!(
//         discrete_gda(1, 1, start, end, start_price, end_price, start+1),
//         Uint128::from(67051188842u128)
//     );
//     assert_eq!(
//         discrete_gda(1, 1, start, end, start_price, end_price, start+duration-1),
//         end_price,
//     );
// }

#[test]
fn dutch_auction_decline_linear() {
    let start = Timestamp::from_seconds(167540000).seconds();
    let end = Timestamp::from_seconds(start + 7 * 24 * 60 * 60).seconds();

    let duration = end - start;
    let start_price = Uint128::from(100000000000u128);
    let end_price = Uint128::from(1000000000u128);

    let auction_duration = end - start;
    let five_minutes_seconds = 5 * 60;
    let price_diff = start_price.u128() - end_price.u128();
    let price_drop_per_period = price_diff/(auction_duration as u128 / five_minutes_seconds);

    // linear decay
    let b = 0.5;

    //before it starts
    const decline_period: u64 = 300;
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start - 1, b, decline_period),
        start_price
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start, b, decline_period),
        start_price
    );
    //after it ends, it stays at resting price
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, end, b, decline_period),
        end_price
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, end + 1000, b, decline_period),
        end_price
    );

    // during declining period price gradually decreases linearly
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 299, b, decline_period),
        start_price
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + decline_period, b, decline_period),
        start_price - Uint128::from(price_drop_per_period)
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 599, b, decline_period),
        start_price - Uint128::from(price_drop_per_period)
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 600, b, decline_period),
        start_price - Uint128::from(price_drop_per_period * 2)
    );


    // check the timestamp of the next price change
    assert_eq!(
        dutch_auction_linear_next_price_change_timestamp(start, end, start - 10000, decline_period),
        start
    );
    assert_eq!(
        dutch_auction_linear_next_price_change_timestamp(start, end, start, decline_period),
        start + decline_period
    );
    assert_eq!(
        dutch_auction_linear_next_price_change_timestamp(start, end, start + 1, decline_period),
        start + decline_period
    );
    assert_eq!(
        dutch_auction_linear_next_price_change_timestamp(start, end, end, decline_period),
        end
    );
    assert_eq!(
        dutch_auction_linear_next_price_change_timestamp(start, end, end-1, decline_period),
        end
    );
    assert_eq!(
        dutch_auction_linear_next_price_change_timestamp(start, end, end+1, decline_period),
        end
    );
}

#[test]
fn dutch_auction_decline_exp() {
    let start = Timestamp::from_seconds(167540000).seconds();
    let end = Timestamp::from_seconds(start + 60 * 30).seconds();

    let start_price = Uint128::from(100000000000u128);
    let end_price = Uint128::from(1000000000u128);

    const b:f64 = 0.85;
    const decline_period_seconds: u64 = 300;

    //before it starts
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start - 1, b, decline_period_seconds),
        start_price
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start, b, decline_period_seconds),
        start_price
    );
    //after it ends, it stays at resting price
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, end, b, decline_period_seconds),
        end_price
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, end + 1000, b, decline_period_seconds),
        end_price
    );

    // during declining period price gradually decreases linearly
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 299, b, decline_period_seconds),
        start_price
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + decline_period_seconds, b, decline_period_seconds),
        Uint128::from(47406250000u128)
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 599, b, decline_period_seconds),
        Uint128::from(47406250000u128)
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 600, b, decline_period_seconds),
        Uint128::from(26826086956u128)
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 900, b, decline_period_seconds),
        Uint128::from(15850000000u128)
    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 1200, b, decline_period_seconds),
        Uint128::from(9027027027u128)

    );
    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 1500, b, decline_period_seconds),
        Uint128::from(4374999999u128)
    );

    assert_eq!(
        declining_dutch_auction(start, end, start_price, end_price, start + 1800, b, decline_period_seconds),
        Uint128::from(1000000000u128)
    );
}

    #[test]
fn happy_path() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, GENESIS_MINT_START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 2;

    // Get dev address balance Before any actions
    let pw_balance_before = router
        .wrap()
        .query_all_balances("stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr".to_string())
        .unwrap();
    assert_eq!(0, pw_balance_before.len());

    let (minter_addr, config) = setup_minter_contract(&mut router, &creator, num_tokens);


    // Get dev address balance Before any actions
    let pw_balance_after_mint = router
        .wrap()
        .query_all_balances("stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr".to_string())
        .unwrap();
    assert_eq!(1, pw_balance_after_mint.len());
    assert_eq!(pw_balance_after_mint[0].amount.u128(), PW_CREATE_FEE);

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
    assert_eq!(creator_balances, coins(INITIAL_BALANCE + 75_000_000, NATIVE_DENOM));
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
    if res.is_err() {
        println!("{}", res.as_ref().err().unwrap().to_string())
    }
    // not sure why this is panicking.
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

    //creator should have balance
    let pw_balance_creator = router
        .wrap()
        .query_all_balances(creator.to_string())
        .unwrap();
    assert_eq!(1, pw_balance_creator.len());
    assert_eq!(pw_balance_creator[0].amount.u128(), 2_060_000_000); //fair burn plus PW fees

    // Minter contract should not have a balance
    let minter_balance = router
        .wrap()
        .query_all_balances(minter_addr.clone())
        .unwrap();
    assert_eq!(0, minter_balance.len());

    // Dev address should have a balance
    let pw_balance2 = router
        .wrap()
        .query_all_balances("stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr".to_string())
        .unwrap();
    assert_eq!(1, pw_balance2.len());
    assert_eq!(pw_balance2[0].amount.u128(), 117_500_000); //fair burn plus PW fees

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

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config.num_tokens, 1);
}

#[test]
fn happy_path_dutch_auction() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, GENESIS_MINT_START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 10;

    // Get dev address balance Before any actions
    let pw_balance_before = router
        .wrap()
        .query_all_balances("stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr".to_string())
        .unwrap();
    assert_eq!(0, pw_balance_before.len());

    let one_hour_nanos = 3600 * 1_000_000_000;
    let five_minutes_nanos = 5 * 60 * 1_000_000_000;
    let end_time = GENESIS_MINT_START_TIME + one_hour_nanos;
    let unit_price = 100_000_000u128; //100 stars
    let resting_price = 10_000_000; // 10 stars
    let price_diff = unit_price - resting_price;
    let price_drop_per_period = price_diff/(one_hour_nanos as u128 /five_minutes_nanos);

    let (minter_addr, _) = setup_minter_contract_dutch_auction(&mut router, &creator, num_tokens, end_time, unit_price, resting_price);
    let mut buyer_spent = 0u128;

    // // Get dev address balance Before any actions
    // let pw_balance_after_mint = router
    //     .wrap()
    //     .query_all_balances("stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr".to_string())
    //     .unwrap();
    // assert_eq!(1, pw_balance_after_mint.len());
    // assert_eq!(pw_balance_after_mint[0].amount.u128(), PW_CREATE_FEE);

    // Default start time genesis mint time
    let res: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
        .unwrap();
    assert_eq!(
        res.start_time,
        Timestamp::from_nanos(GENESIS_MINT_START_TIME).to_string()
    );

    let one_minute_nanos = 60 * 1_000_000_000;
    setup_block_time_height(&mut router, GENESIS_MINT_START_TIME +one_minute_nanos, 2);

    // Fail with incorrect tokens
    let mint_msg = ExecuteMsg::Mint {};
    let err = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(unit_price - 1, NATIVE_DENOM),
    );
    assert!(err.is_err());

    // read the price parameters
    let res: MintPriceResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintPrice {},
        )
        .unwrap();
    assert_eq!(res.current_price.amount.u128(), unit_price);
    assert_eq!(res.public_price.amount.u128(), unit_price);
    assert_eq!(u64::from_str_radix(res.auction_end_time.unwrap().as_str(), 10).unwrap(), end_time);
    let next_price_time = GENESIS_MINT_START_TIME+ 5*60*1000*1000*1000;
    assert_eq!(u64::from_str_radix(res.auction_next_price_timestamp.unwrap().as_str(), 10).unwrap(), next_price_time);

    // Succeeds if funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(unit_price, NATIVE_DENOM),
    );
    assert!(res.is_ok());
    buyer_spent += unit_price;

    setup_block_time_height(&mut router, GENESIS_MINT_START_TIME + one_minute_nanos*2, 3);

    // Succeeds if too many funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(unit_price+1, NATIVE_DENOM),
    );
    if let Err(ref e) = res {
        println!("Error: {}", e);
        assert!(res.is_ok());
    }
    buyer_spent += unit_price;

    // Balances are correct
    // The creator should get the unit price - mint fee for the mint above
    let creator_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(creator_balances, coins(INITIAL_BALANCE + 176_000_000, NATIVE_DENOM));
    // The buyer's tokens should reduce by unit price
    let buyer_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(
        buyer_balances,
        coins(INITIAL_BALANCE - buyer_spent, NATIVE_DENOM)
    );

    let six_minutes_nanos = 6 * 60 * 1_000_000_000;
    // Mint after the price has dropped
    setup_block_time_height(&mut router, GENESIS_MINT_START_TIME + six_minutes_nanos, 4);

    // Succeeds if too many funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(unit_price+1, NATIVE_DENOM),
    );
    if let Err(ref e) = res {
        println!("Error: {}", e);
        assert!(res.is_ok());
    }
    // dutch auction price after 6 minutes
    buyer_spent += unit_price - price_drop_per_period;

    // Balances are correct
    // The creator should get the unit price - mint fee for the mint above
    let creator_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(creator_balances, coins(INITIAL_BALANCE + 257_400_000, NATIVE_DENOM));
    // The buyer's tokens should reduce by unit price
    let buyer_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(
        buyer_balances,
        coins(INITIAL_BALANCE - buyer_spent, NATIVE_DENOM)
    );

    let res: MintPriceResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintPrice {},
        )
        .unwrap();
    assert_eq!(res.current_price.amount.u128(), unit_price - price_drop_per_period);

    setup_block_time_height(&mut router, GENESIS_MINT_START_TIME + one_hour_nanos - 1, 5);
    // read the price parameters just before price drops
    let res: MintPriceResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintPrice {},
        )
        .unwrap();
    assert_eq!(res.current_price.amount.u128(), resting_price + price_drop_per_period);
    assert_eq!(res.public_price.amount.u128(), unit_price);
    assert_eq!(u64::from_str_radix(res.auction_end_time.unwrap().as_str(), 10).unwrap(), end_time);
    let next_price_time = end_time;
    assert_eq!(u64::from_str_radix(res.auction_next_price_timestamp.unwrap().as_str(), 10).unwrap(), next_price_time);


    // failed to mint just before price drops
    let mint_msg = ExecuteMsg::Mint {};
    router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(resting_price + price_drop_per_period - 1, NATIVE_DENOM),
    ).unwrap();
    assert!(err.is_err());

    // mint just before price drops
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins( resting_price + price_drop_per_period, NATIVE_DENOM),
    );
    if let Err(ref e) = res {
        println!("Error: {}", e);
        assert!(res.is_ok());
    }
    buyer_spent += resting_price + price_drop_per_period;

    setup_block_time_height(&mut router, GENESIS_MINT_START_TIME + one_hour_nanos, 6);
    // mint at resting price
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins( resting_price , NATIVE_DENOM),
    );
    if let Err(ref e) = res {
        println!("Error: {}", e);
        assert!(res.is_ok());
    }
    buyer_spent += resting_price;

    let buyer_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(
        buyer_balances,
        coins(INITIAL_BALANCE - buyer_spent, NATIVE_DENOM)
    );
}
