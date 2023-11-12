use cosmwasm_std::{Addr, BankMsg, Binary, coin, Coin, coins, CosmosMsg, Decimal, Deps, DepsMut, Empty, Env, MessageInfo, Order, Reply, ReplyOn, StdError, StdResult, Timestamp, to_binary, Uint128, WasmMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::set_contract_version;
use cw721_base::{MintMsg, msg::ExecuteMsg as Cw721ExecuteMsg};
use cw_utils::{may_pay, maybe_addr, must_pay, nonpayable, parse_reply_instantiate_data};
use semver::Version;
use sg1::{FeeError};
use sg_std::{GENESIS_MINT_START_TIME, NATIVE_DENOM, StargazeMsgWrapper};
use url::Url;

use sg721_imago::msg::{CollectionInfoResponse, InstantiateMsg as Sg721InstantiateMsg};
use sg721_imago::msg::QueryMsg::CollectionInfo;
use whitelist::msg::{
    ConfigResponse as WhitelistConfigResponse, HasMemberResponse, QueryMsg as WhitelistQueryMsg,
};

use crate::error::ContractError;
use crate::msg::{ConfigResponse, DutchAuctionConfig, DutchAuctionPriceResponse, ExecuteMsg, InstantiateMsg, MintableNumTokensResponse, MintCountResponse, MintPriceResponse, QueryMsg, StartTimeResponse};
use crate::state::{
    Config, CONFIG, DutchAuctionConfig as DutchAuctionConfigState, MINTABLE_NUM_TOKENS, MINTABLE_TOKEN_IDS, MINTER_ADDRS, SG721_ADDRESS,
};

pub type Response = cosmwasm_std::Response<StargazeMsgWrapper>;
pub type SubMsg = cosmwasm_std::SubMsg<StargazeMsgWrapper>;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-minter-imago";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEV_ADDRESS: &str = "stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr";

const PW_HOSTNAME_SUFFIX: &str = "publicworks.art";

const INSTANTIATE_SG721_REPLY_ID: u64 = 1;

const MAX_DUTCH_AUCTION_DECLINE_DECAY: u64 = 1_000_000;

// governance parameters
const MAX_TOKEN_LIMIT: u32 = 10000;
const MAX_PER_ADDRESS_LIMIT: u32 = 100;
const MIN_MINT_PRICE: u128 = 0;
const AIRDROP_MINT_PRICE: u128 = 50_000_000;
// 100% airdrop fee goes to pw
const AIRDROP_MINT_FEE_PERCENT: u32 = 100;
// 4% mint fee goes to pw
const PW_MINT_FEE_PERCENT: u64 = 4;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Check the number of tokens is more than zero and less than the max limit
    if msg.num_tokens == 0 || msg.num_tokens > MAX_TOKEN_LIMIT {
        return Err(ContractError::InvalidNumTokens {
            min: 1,
            max: MAX_TOKEN_LIMIT,
        });
    }

    // Check per address limit is valid
    if msg.per_address_limit == 0 || msg.per_address_limit > MAX_PER_ADDRESS_LIMIT {
        return Err(ContractError::InvalidPerAddressLimit {
            max: MAX_PER_ADDRESS_LIMIT,
            min: 1,
            got: msg.per_address_limit,
        });
    }

    // Check that the price is in the correct denom ('ustars')
    if NATIVE_DENOM != msg.unit_price.denom {
        return Err(ContractError::InvalidDenom {
            expected: NATIVE_DENOM.to_string(),
            got: msg.unit_price.denom,
        });
    }

    let valid = validate_price(msg.unit_price.amount.u128());
    if valid.is_err() {
        return Err(valid.err().unwrap());
    }


    // If current time is beyond the provided start time return error
    if env.block.time > msg.start_time {
        return Err(ContractError::InvalidStartTime(
            msg.start_time,
            env.block.time,
        ));
    }

    // Validate address for the optional whitelist contract
    let whitelist_addr = msg
        .whitelist
        .and_then(|w| deps.api.addr_validate(w.as_str()).ok());

    let parsed_token_uri = Url::parse(&msg.base_token_uri)?;
    if parsed_token_uri.scheme() != "https" {
        return Err(ContractError::InvalidBaseTokenURI {});
    }
    let host_error = match parsed_token_uri.domain() {
        Some(d) => {
            if d.ends_with(PW_HOSTNAME_SUFFIX) {
                None
            } else {
                Some(Err(ContractError::InvalidBaseTokenURI {}))
            }
        }
        _ => Some(Err(ContractError::InvalidBaseTokenURI {}))
    };
    if let Some(..) = host_error {
        return host_error.unwrap();
    }
    let base_token_uri = msg.base_token_uri.clone();

    let parsed_code_uri = Url::parse(&msg.sg721_instantiate_msg.code_uri);
    if parsed_code_uri.is_err() {
        return Err(ContractError::InvalidCodeUri {});
    }
    if parsed_code_uri.unwrap().scheme() != "ipfs" {
        return Err(ContractError::InvalidCodeUri {});
    }

    //dutch auction config checks
    let is_dutch_auction = msg.dutch_auction_config.is_some();
    let dutch_auction_config = if is_dutch_auction {
        let dutch_auction_config = validate_dutch_auction(msg.start_time, msg.unit_price.amount.u128(),
                                                          msg.dutch_auction_config.clone().unwrap(),
        );
        if dutch_auction_config.is_err() {
            return Err(dutch_auction_config.err().unwrap());
        }
        Some(dutch_auction_config.unwrap())
    } else {
        None
    };


    let config = Config {
        admin: info.sender.clone(),
        base_token_uri,
        num_tokens: msg.num_tokens,
        sg721_code_id: msg.sg721_code_id,
        unit_price: msg.unit_price,
        per_address_limit: msg.per_address_limit,
        whitelist: whitelist_addr,
        start_time: msg.start_time,
        dutch_auction_config,
    };
    CONFIG.save(deps.storage, &config)?;
    MINTABLE_NUM_TOKENS.save(deps.storage, &msg.num_tokens)?;

    // Save mintable token ids map
    for token_id in 1..=msg.num_tokens {
        MINTABLE_TOKEN_IDS.save(deps.storage, token_id, &true)?;
    }

    // Submessage to instantiate sg721 contract
    let sub_msgs: Vec<SubMsg> = vec![SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: msg.sg721_code_id,
            msg: to_binary(&Sg721InstantiateMsg {
                name: msg.sg721_instantiate_msg.name,
                symbol: msg.sg721_instantiate_msg.symbol,
                minter: env.contract.address.to_string(),
                finalizer: msg.sg721_instantiate_msg.finalizer,
                code_uri: msg.sg721_instantiate_msg.code_uri.to_string(),
                collection_info: msg.sg721_instantiate_msg.collection_info,
            })?,
            funds: info.funds,
            admin: Some(info.sender.to_string()),
            label: String::from("Fixed price minter"),
        }
            .into(),
        id: INSTANTIATE_SG721_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", info.sender)
        .add_submessages(sub_msgs))
}

fn validate_dutch_auction(start_time: Timestamp, start_price: u128, config: DutchAuctionConfig) -> Result<DutchAuctionConfigState, ContractError> {
    let valid = validate_price(start_price);
    if valid.is_err() {
        return Err(valid.err().unwrap());
    }
    if config.end_time <= start_time {
        return Err(ContractError::InvalidEndTime {});
    }
    if config.resting_unit_price.clone().amount.u128() >= start_price {
        return Err(ContractError::InvalidRestingPrice {});
    }

    let duration = config.end_time.seconds() - start_time.seconds();
    if config.decline_period_seconds == 0 || config.decline_period_seconds > duration {
        return Err(ContractError::InvalidDeclinePeriodSeconds {});
    }

    if config.decline_decay > MAX_DUTCH_AUCTION_DECLINE_DECAY {
        return Err(ContractError::InvalidDutchAuctionDeclineDecay {});
    }

    return Ok(DutchAuctionConfigState {
        end_time: config.end_time,
        resting_unit_price: config.resting_unit_price,
        decline_decay: config.decline_decay,
        decline_period_seconds: config.decline_period_seconds,
    });
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint {} => execute_mint_sender(deps, env, info),
        ExecuteMsg::UpdateStartTime(time) => execute_update_start_time(deps, env, info, time),
        ExecuteMsg::UpdatePerAddressLimit { per_address_limit } => {
            execute_update_per_address_limit(deps, env, info, per_address_limit)
        }
        ExecuteMsg::MintTo { recipient } => execute_mint_to(deps, env, info, recipient),
        ExecuteMsg::SetWhitelist { whitelist } => {
            execute_set_whitelist(deps, env, info, &whitelist)
        }
        ExecuteMsg::BurnRemaining {} => execute_burn_remaining(deps, env, info),
        ExecuteMsg::UpdatePrice { unit_price } => execute_update_unit_price(deps, info, unit_price),
        ExecuteMsg::UpdateDutchAuction { start_time, unit_price, dutch_auction_config } => execute_update_dutch_auction(deps, info, start_time, unit_price, dutch_auction_config),
    }
}

pub fn execute_set_whitelist(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    whitelist: &str,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    if let Some(wl) = config.whitelist {
        let res: WhitelistConfigResponse = deps
            .querier
            .query_wasm_smart(wl, &WhitelistQueryMsg::Config {})?;

        if res.is_active {
            return Err(ContractError::WhitelistAlreadyStarted {});
        }
    }

    config.whitelist = Some(deps.api.addr_validate(whitelist)?);
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default()
        .add_attribute("action", "set_whitelist")
        .add_attribute("whitelist", whitelist.to_string()))
}

pub fn execute_mint_sender(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_sender";

    // If there is no active whitelist right now, check public mint
    // Check if after start_time
    if is_public_mint(deps.as_ref(), &info)? && (env.block.time < config.start_time) {
        return Err(ContractError::BeforeMintStartTime {});
    }

    // Check if already minted max per address limit
    let mint_count = mint_count(deps.as_ref(), &info)?;
    if mint_count >= config.per_address_limit {
        return Err(ContractError::MaxPerAddressLimitExceeded {});
    }

    _execute_mint(deps, env, info, action, false, None)
}


// Check if a whitelist exists and not ended
// Sender has to be whitelisted to mint
fn is_public_mint(deps: Deps, info: &MessageInfo) -> Result<bool, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // If there is no whitelist, there's only a public mint
    if config.whitelist.is_none() {
        return Ok(true);
    }

    let whitelist = config.whitelist.unwrap();

    let wl_config: WhitelistConfigResponse = deps
        .querier
        .query_wasm_smart(whitelist.clone(), &WhitelistQueryMsg::Config {})?;

    if !wl_config.is_active {
        return Ok(true);
    }

    let res: HasMemberResponse = deps.querier.query_wasm_smart(
        whitelist,
        &WhitelistQueryMsg::HasMember {
            member: info.sender.to_string(),
        },
    )?;
    if !res.has_member {
        return Err(ContractError::NotWhitelisted {
            addr: info.sender.to_string(),
        });
    }

    // Check wl per address limit
    let mint_count = mint_count(deps, info)?;
    if mint_count >= wl_config.per_address_limit {
        return Err(ContractError::MaxPerAddressLimitExceeded {});
    }

    Ok(false)
}

pub fn execute_mint_to(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let recipient = deps.api.addr_validate(&recipient)?;
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_to";

    // Check only admin
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }

    _execute_mint(deps, env, info, action, true, Some(recipient))
}

// Generalize checks and mint message creation
// mint -> _execute_mint(recipient: None, token_id: None)
// mint_to(recipient: "friend") -> _execute_mint(Some(recipient), token_id: None)
// mint_for(recipient: "friend2", token_id: 420) -> _execute_mint(recipient, token_id)
fn _execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    action: &str,
    is_admin: bool,
    recipient: Option<Addr>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let sg721_address = SG721_ADDRESS.load(deps.storage)?;

    let recipient_addr = match recipient {
        Some(some_recipient) => some_recipient,
        None => info.sender.clone(),
    };

    let mint_price: Coin = mint_price(deps.as_ref(), env.clone(), is_admin)?;
    // Exact payment only accepted
    let payment = may_pay(&info, &config.unit_price.denom)?;

    if config.dutch_auction_config.is_some() {
        // dutch auction allows overpaying. If overpaying, refund the difference.
        if payment < mint_price.amount {
            return Err(ContractError::IncorrectPaymentAmount(
                coin(payment.u128(), &config.unit_price.denom),
                mint_price,
            ));
        }
    } else {
        //exact payment only accepted for regular sale pricing auction
        if payment != mint_price.amount {
            return Err(ContractError::IncorrectPaymentAmount(
                coin(payment.u128(), &config.unit_price.denom),
                mint_price,
            ));
        }
    }

    let mut msgs: Vec<CosmosMsg<StargazeMsgWrapper>> = vec![];

    // Create network fee msgs
    let fee_percent = if is_admin {
        Decimal::percent(AIRDROP_MINT_FEE_PERCENT as u64)
    } else {
        Decimal::percent(PW_MINT_FEE_PERCENT as u64)
    };

    let addr = maybe_addr(deps.api, Some(DEV_ADDRESS.to_string()))?;

    let pw_fee = mint_price.amount * fee_percent;
    msgs.append(&mut pw_fee_msg(&info, pw_fee.u128(), addr.clone().unwrap())?);

    // Create refund fee msg if the sender overpaid for auction.
    if payment > mint_price.amount {
        let sender = deps.api.addr_validate(&info.sender.to_string())?;
        msgs.append(&mut refund_fee_msg((payment - mint_price.amount).u128(), sender));
    }

    let mintable_tokens_result: StdResult<Vec<u32>> = MINTABLE_TOKEN_IDS
        .keys(deps.storage, None, None, Order::Ascending)
        .take(1)
        .collect();
    let mintable_tokens = mintable_tokens_result?;
    if mintable_tokens.is_empty() {
        return Err(ContractError::SoldOut {});
    }
    let mintable_token_id = mintable_tokens[0];

    // Create mint msgs
    let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Empty> {
        token_id: mintable_token_id.to_string(),
        owner: recipient_addr.to_string(),
        token_uri: Some(format!("{}/{}", config.base_token_uri, mintable_token_id)),
        extension: Empty {},
    });
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: sg721_address.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    });
    msgs.append(&mut vec![msg]);

    // Remove mintable token id from map
    MINTABLE_TOKEN_IDS.remove(deps.storage, mintable_token_id);
    let mintable_num_tokens = MINTABLE_NUM_TOKENS.load(deps.storage)?;
    // Decrement mintable num tokens
    MINTABLE_NUM_TOKENS.save(deps.storage, &(mintable_num_tokens - 1))?;
    // Save the new mint count for the sender's address
    let new_mint_count = mint_count(deps.as_ref(), &info)? + 1;
    MINTER_ADDRS.save(deps.storage, info.clone().sender, &new_mint_count)?;

    let sg721_config: CollectionInfoResponse = deps
        .querier
        .query_wasm_smart(sg721_address, &CollectionInfo {})?;

    // payout to the royalty address if it exists for splits to work
    let payment_address = if sg721_config.royalty_info.is_some() {
        sg721_config.royalty_info.unwrap().payment_address
    } else {
        config.admin.to_string()
    };

    let seller_amount = if !is_admin {
        let amount = mint_price.amount - pw_fee;
        let msg = BankMsg::Send {
            to_address: payment_address,
            amount: vec![coin(amount.u128(), config.unit_price.denom)],
        };
        msgs.push(CosmosMsg::Bank(msg));
        amount
    } else {
        Uint128::zero()
    };

    Ok(Response::default()
        .add_attribute("action", action)
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient_addr)
        .add_attribute("token_id", mintable_token_id.to_string())
        .add_attribute("pw_fee", pw_fee)
        .add_attribute("mint_price", mint_price.amount)
        .add_attribute("seller_amount", seller_amount)
        .add_messages(msgs))
}

fn pw_fee_msg(
    info: &MessageInfo,
    fee: u128,
    developer: Addr,
) -> Result<Vec<CosmosMsg<StargazeMsgWrapper>>, FeeError> {
    let payment = must_pay(info, NATIVE_DENOM)?;
    if payment.u128() < fee {
        return Err(FeeError::InsufficientFee(fee, payment.u128()));
    };
    let mut msgs: Vec<CosmosMsg<StargazeMsgWrapper>> = vec![];
    let msg = BankMsg::Send {
        to_address: developer.to_string(),
        amount: coins(fee, NATIVE_DENOM),
    };
    msgs.push(CosmosMsg::Bank(msg));

    Ok(msgs)
}

fn refund_fee_msg(
    amount: u128,
    sender: Addr,
) -> Vec<CosmosMsg<StargazeMsgWrapper>> {
    let mut msgs: Vec<CosmosMsg<StargazeMsgWrapper>> = vec![];
    let msg = BankMsg::Send {
        to_address: sender.to_string(),
        amount: coins(amount, NATIVE_DENOM),
    };
    msgs.push(CosmosMsg::Bank(msg));

    msgs
}

pub fn execute_burn_remaining(
    deps: DepsMut,
    _: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    let mintable_count = MINTABLE_NUM_TOKENS.load(deps.storage)?;
    let num_tokens = config.num_tokens - mintable_count;

    // Check only admin
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }
    MINTABLE_NUM_TOKENS.save(deps.storage, &0)?;

    let mut token_ids = vec![];
    for mapping in MINTABLE_TOKEN_IDS.range(deps.storage, None, None, Order::Ascending) {
        let (token_id, _) = mapping?;
        token_ids.push(token_id);
    }
    for (_, token_id) in token_ids.iter().enumerate() {
        MINTABLE_TOKEN_IDS.remove(deps.storage, *token_id);
    }

    config.num_tokens = num_tokens;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default()
        .add_attribute("action", "burn_remaining")
        .add_attribute("sender", info.sender))
}

pub fn execute_update_start_time(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    start_time: Timestamp,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }
    // If current time is after the stored start time return error
    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    // If current time already passed the new start_time return error
    if env.block.time > start_time {
        return Err(ContractError::InvalidStartTime(start_time, env.block.time));
    }

    let genesis_start_time = Timestamp::from_nanos(GENESIS_MINT_START_TIME);
    // If the new start_time is before genesis start time return error
    if start_time < genesis_start_time {
        return Err(ContractError::BeforeGenesisTime {});
    }

    config.start_time = start_time;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_start_time")
        .add_attribute("sender", info.sender)
        .add_attribute("start_time", start_time.to_string()))
}

pub fn execute_update_per_address_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    per_address_limit: u32,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }
    if per_address_limit == 0 || per_address_limit > MAX_PER_ADDRESS_LIMIT {
        return Err(ContractError::InvalidPerAddressLimit {
            max: MAX_PER_ADDRESS_LIMIT,
            min: 1,
            got: per_address_limit,
        });
    }
    config.per_address_limit = per_address_limit;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_per_address_limit")
        .add_attribute("sender", info.sender)
        .add_attribute("limit", per_address_limit.to_string()))
}

fn validate_price(price: u128) -> Result<(), ContractError> {
    if price < MIN_MINT_PRICE {
        return Err(ContractError::InsufficientMintPrice {
            expected: MIN_MINT_PRICE,
            got: price,
        });
    }
    Ok(())
}

pub fn execute_update_unit_price(
    deps: DepsMut,
    info: MessageInfo,
    price: Coin,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }
    let valid = validate_price(price.amount.u128());
    if valid.is_err() {
        return Err(valid.err().unwrap());
    }

    //update the unit price and remove dutch auction settings
    config.unit_price = price.clone();
    config.dutch_auction_config = None;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_unit_price")
        .add_attribute("sender", info.sender)
        .add_attribute("unit_price", price.clone().to_string()))
}

pub fn execute_update_dutch_auction(
    deps: DepsMut,
    info: MessageInfo,
    start_time: Timestamp, unit_price: Coin,
    dutch_auction_config: DutchAuctionConfig,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }

    let dutch_auction_config = validate_dutch_auction(start_time, unit_price.amount.u128(), dutch_auction_config);
    if dutch_auction_config.is_err() {
        return Err(dutch_auction_config.err().unwrap());
    }
    config.start_time = start_time;
    config.unit_price = unit_price.clone();
    config.dutch_auction_config = Some(dutch_auction_config.unwrap());

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_dutch_auction")
        .add_attribute("sender", info.sender)
        .add_attribute("unit_price", unit_price.to_string()))
}

fn dutch_auction_price_response(env: Env, config: Config) -> Coin {
    let da_config = config.dutch_auction_config.unwrap();
    let end_time = da_config.end_time;
    let resting_unit_price = da_config.resting_unit_price;
    let decay = da_config.decline_decay;
    let dutch_auction_decline_period_seconds = da_config.decline_period_seconds;

    let da_price = dutch_auction_price_at_time(
        config.start_time.seconds(),
        end_time.seconds(),
        config.unit_price.amount,
        resting_unit_price.amount,
        env.block.time.seconds(),
        decay,
        dutch_auction_decline_period_seconds,
    );

    return coin(da_price.u128(), config.unit_price.denom);
}

// if admin_no_fee => no fee,
// else if in whitelist => whitelist price
// else if dutch auction => dutch auction price
// else => config unit price
pub fn mint_price(deps: Deps, env: Env, is_admin: bool) -> Result<Coin, StdError> {
    let config = CONFIG.load(deps.storage)?;

    if is_admin {
        return Ok(coin(AIRDROP_MINT_PRICE, config.unit_price.denom));
    }

    if config.whitelist.is_some() {
        let whitelist = config.whitelist.clone().unwrap();

        let wl_config: WhitelistConfigResponse = deps
            .querier
            .query_wasm_smart(whitelist, &WhitelistQueryMsg::Config {})?;

        if wl_config.is_active {
            return Ok(wl_config.unit_price);
        }
    }

    return if config.dutch_auction_config.is_some() {
        Ok(dutch_auction_price_response(env, config.clone()))
    } else {
        Ok(config.unit_price)
    }
}

fn mint_count(deps: Deps, info: &MessageInfo) -> Result<u32, StdError> {
    let mint_count = (MINTER_ADDRS
        .key(info.sender.clone())
        .may_load(deps.storage)?)
        .unwrap_or(0);
    Ok(mint_count)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::StartTime {} => to_binary(&query_start_time(deps)?),
        QueryMsg::MintableNumTokens {} => to_binary(&query_mintable_num_tokens(deps)?),
        QueryMsg::MintPrice {} => to_binary(&query_mint_price(deps, _env)?),
        QueryMsg::MintCount { address } => to_binary(&query_mint_count(deps, address)?),
    }
}


fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let sg721_address = SG721_ADDRESS.load(deps.storage)?;

    let dutch_auction_config = config.dutch_auction_config.map(|da_config| {
        DutchAuctionConfig {
            end_time: da_config.end_time,
            resting_unit_price: da_config.resting_unit_price,
            decline_decay: da_config.decline_decay,
            decline_period_seconds: da_config.decline_period_seconds,
        }
    });

    Ok(ConfigResponse {
        admin: config.admin.to_string(),
        base_token_uri: config.base_token_uri,
        sg721_address: sg721_address.to_string(),
        sg721_code_id: config.sg721_code_id,
        num_tokens: config.num_tokens,
        start_time: config.start_time,
        unit_price: config.unit_price,
        per_address_limit: config.per_address_limit,
        whitelist: config.whitelist.map(|w| w.to_string()),
        dutch_auction_config,
    })
}

fn query_mint_count(deps: Deps, address: String) -> StdResult<MintCountResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let mint_count = (MINTER_ADDRS.key(addr.clone()).may_load(deps.storage)?).unwrap_or(0);
    Ok(MintCountResponse {
        address: addr.to_string(),
        count: mint_count,
    })
}

fn query_start_time(deps: Deps) -> StdResult<StartTimeResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(StartTimeResponse {
        start_time: config.start_time.to_string(),
    })
}

fn query_mintable_num_tokens(deps: Deps) -> StdResult<MintableNumTokensResponse> {
    let count = MINTABLE_NUM_TOKENS.load(deps.storage)?;
    Ok(MintableNumTokensResponse { count })
}

fn query_mint_price(deps: Deps, env: Env) -> StdResult<MintPriceResponse> {
    let config = CONFIG.load(deps.storage)?;
    let current_price = mint_price(deps, env.clone(), false)?;
    let public_price = config.unit_price;
    let whitelist_price: Option<Coin> = if let Some(whitelist) = config.whitelist {
        let wl_config: WhitelistConfigResponse = deps
            .querier
            .query_wasm_smart(whitelist, &WhitelistQueryMsg::Config {})?;
        Some(wl_config.unit_price)
    } else {
        None
    };
    if config.dutch_auction_config.is_none() {
        return Ok(MintPriceResponse {
            current_price,
            public_price,
            whitelist_price,
            dutch_auction_price: None,
        });
    }
    let dutch_auction_config = config.dutch_auction_config.unwrap();
    let auction_rest_price = dutch_auction_config.resting_unit_price;
    let auction_end_time = dutch_auction_config.end_time.nanos().to_string();
    let auction_next_price_timestamp = Timestamp::from_seconds(dutch_auction_next_price_change_timestamp(
        config.start_time.seconds(),
        dutch_auction_config.end_time.seconds(),
        env.block.time.seconds(),
        dutch_auction_config.decline_period_seconds,
    )).nanos().to_string();

    let dutch_auction_price = DutchAuctionPriceResponse{
        next_price_timestamp: auction_next_price_timestamp,
        end_time: auction_end_time,
        rest_price: auction_rest_price,
    };

    Ok(MintPriceResponse {
        current_price,
        public_price,
        whitelist_price,
        dutch_auction_price: Some(dutch_auction_price),
    })
}

pub fn dutch_auction_next_price_change_timestamp(
    start_time: u64,
    end_time: u64,
    current_time: u64,
    decline_period: u64,
) -> u64 {
    if current_time < start_time {
        return start_time;
    }
    if current_time >= end_time {
        return end_time;
    }
    let current_bucket = (current_time - start_time) / decline_period;
    let next_bucket = current_bucket + 1;
    let time_at_next_bucket = start_time + (next_bucket * decline_period);

    time_at_next_bucket
}


pub fn dutch_auction_price_at_time(
    start_time_seconds: u64,
    end_time_seconds: u64,
    start_price: Uint128,
    end_price: Uint128,
    current_time_seconds: u64,
    decay: u64,
    decline_period_seconds: u64,
) -> Uint128 {
    if decay > MAX_DUTCH_AUCTION_DECLINE_DECAY {
        panic!("b must be between 0 and 1 as an integer with 6 decimal places");
    }
    if current_time_seconds <= start_time_seconds {
        return start_price;
    }
    if current_time_seconds >= end_time_seconds {
        return end_price;
    }

    let precision_18dp: u64 = 1_000_000_000_000_000_000;
    let precision_18dp_u128: u128 = precision_18dp as u128;
    let precision_6dp_u128: u128 = MAX_DUTCH_AUCTION_DECLINE_DECAY as u128;
    let coin_precision: u128 = 1_000_000 as u128;
    let decay_6dp_u128: u128 = decay as u128;

    let duration = (end_time_seconds - start_time_seconds) as u128;
    let current_bucket = ((current_time_seconds - start_time_seconds) / decline_period_seconds) as u128;
    let current_time_bucket = ((start_time_seconds as u128) + current_bucket * (decline_period_seconds as u128)) as u128;

    // the following code is a translation of Christophe Schlick’s falloff formula
    // f(x) = x / ((1 / b - 2) * (1 - x) + 1)
    // where b is the decay rate [0..1] and x is the time [0..1]
    // since this needs to divide a normalized float by a normalized float, we use 18dp for the
    // highest precision, 6 for the lowest (decay), and 12 for the intermediate
    // calculations (1/decay). Final output is rounded to 6 decimal places.
    let time_18dp = (current_time_bucket - (start_time_seconds as u128)) * precision_18dp_u128;
    let time_normalized_18dp = time_18dp / duration; // decimal in precision
    let decay_numerator_18dp = precision_18dp_u128;
    let two_12dp = 2i128 * (10i128.pow(12));
    let comp1_of_denom1_12dp = (decay_numerator_18dp / decay_6dp_u128) as i128;
    // can be negative
    let denom1_12dp: i128 = comp1_of_denom1_12dp - two_12dp;
    let denom2_6dp: i128 = ((precision_18dp_u128 - time_normalized_18dp) as i128) / (10u128.pow(12) as i128);
    let denomc_12dp = (denom1_12dp * denom2_6dp) / 10i128.pow(6);
    let ft_6dp = ((precision_6dp_u128 as i128) - (time_normalized_18dp as i128) / (denomc_12dp + 10i128.pow(12))) as u128;

    let price_diff = (start_price - end_price).u128();
    let price = (ft_6dp * price_diff + end_price.u128() * precision_6dp_u128) as u128;
    let price_in_coin_precision = price / precision_6dp_u128;
    let price_floored = price_in_coin_precision / coin_precision * coin_precision;
    let price_remainder = price_in_coin_precision % coin_precision;
    if price_remainder >= coin_precision / 2 {
        return Uint128::from(price_floored + coin_precision);
    }
    return Uint128::from(price_floored);
}


// Reply callback triggered from cw721 contract instantiation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != INSTANTIATE_SG721_REPLY_ID {
        return Err(ContractError::InvalidReplyID {});
    }

    let reply = parse_reply_instantiate_data(msg);
    match reply {
        Ok(res) => {
            SG721_ADDRESS.save(deps.storage, &Addr::unchecked(res.contract_address))?;
            Ok(Response::default().add_attribute("action", "instantiate_sg721_reply"))
        }
        Err(_) => Err(ContractError::InstantiateSg721Error {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: Empty) -> Result<Response, ContractError> {
    let current_version = cw2::get_contract_version(deps.storage)?;
    if current_version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Cannot upgrade to a different contract").into());
    }
    let version: Version = current_version
        .version
        .parse()
        .map_err(|_| StdError::generic_err("Invalid contract version"))?;
    let new_version: Version = CONTRACT_VERSION
        .parse()
        .map_err(|_| StdError::generic_err("Invalid contract version"))?;

    if version > new_version {
        return Err(StdError::generic_err("Cannot upgrade to a previous contract version").into());
    }
    // if same version return
    if version == new_version {
        return Ok(Response::new());
    }

    //add migrate the config

    // set new contract version
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new())
}
