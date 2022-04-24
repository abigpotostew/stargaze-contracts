use cosmwasm_std::{
     CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Order, Binary, ReplyOn, StdResult, Timestamp, to_binary, WasmMsg,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::set_contract_version;
use sg_std::StargazeMsgWrapper;

use sg721_imago::msg::ExecuteMsg as Imago721ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use crate::state::{OWNER, SIGNER};

pub type Response = cosmwasm_std::Response<StargazeMsgWrapper>;
pub type SubMsg = cosmwasm_std::SubMsg<StargazeMsgWrapper>;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:imago-finalizer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_SG721_REPLY_ID: u64 = 1;

// governance parameters
const MAX_TOKEN_LIMIT: u32 = 10000;
const MAX_PER_ADDRESS_LIMIT: u32 = 50;
const MIN_MINT_PRICE: u128 = 50_000_000;
const AIRDROP_MINT_PRICE: u128 = 15_000_000;
const MINT_FEE_PERCENT: u32 = 10;
// 100% airdrop fee goes to fair burn
const AIRDROP_MINT_FEE_PERCENT: u32 = 100;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = deps.api.addr_validate(&msg.owner)?;
    OWNER.save(deps.storage, &owner);

    let signer = deps.api.addr_validate(&msg.signer)?;
    SIGNER.save(deps.storage, &signer);


    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", info.sender)
        .add_attribute("finalize_owner", msg.owner)
        .add_attribute("finalize_signer", msg.signer))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::TransferOwnership { to } => execute_transfer_ownership(deps, info, to),
        ExecuteMsg::ChangeSigner { to } => execute_change_signer(deps, info, to),
        ExecuteMsg::Finalize { contract, token_uri, token_id } => execute_finalize(deps, info, contract.to_string(), token_id, token_uri),
    }
}

pub fn execute_finalize(
    deps: DepsMut,
    info: MessageInfo,
    contract: String, token_id: String, token_uri: String,
) -> Result<Response, ContractError> {
    let signer = SIGNER.load(deps.storage)?;
    if signer != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an signer".to_owned(),
        ));
    };
    let finalize_msg = Imago721ExecuteMsg::FinalizeTokenUri {
        token_id:token_id.to_string(),
        token_uri:token_uri.to_string(),
    };
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract.to_string(),
        msg: to_binary(&finalize_msg)?,
        funds: vec![],
    });

    Ok(Response::default()
        .add_attribute("action", "finalize")
        .add_attribute("finalize_contract", contract)
        .add_attribute("finalize_token_id", token_id)
        .add_attribute("finalize_token_uri", token_uri)
        .add_message(msg))
}


pub fn execute_transfer_ownership(
    deps: DepsMut,
    info: MessageInfo,
    to: String,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if owner != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not owner".to_owned(),
        ));
    };

    let new_owner = deps.api.addr_validate(&to)?;
    OWNER.save(deps.storage, &new_owner);

    Ok(Response::default()
        .add_attribute("action", "finalize_transfer_ownership")
        .add_attribute("finalize_transfer_ownership", &to)
    )
}

pub fn execute_change_signer(
    deps: DepsMut,
    info: MessageInfo,
    to: String,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if owner != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not owner".to_owned(),
        ));
    };

    let new_signer = deps.api.addr_validate(&to)?;
    SIGNER.save(deps.storage, &new_signer);

    Ok(Response::default()
        .add_attribute("action", "finalize_transfer_ownership")
        .add_attribute("finalize_change_signer", &to)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let signer = SIGNER.load(deps.storage)?;
    let owner = OWNER.load(deps.storage)?;

    Ok(ConfigResponse {
        signer: signer.to_string(),
        owner: owner.to_string(),
    })
}
