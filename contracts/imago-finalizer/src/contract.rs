use cosmwasm_std::{Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, StdResult, to_binary, WasmMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::set_contract_version;
use sg_std::StargazeMsgWrapper;

use sg721_imago::msg::ExecuteMsg as Imago721ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use crate::state::{OWNER, SIGNERS};

pub type Response = cosmwasm_std::Response<StargazeMsgWrapper>;
pub type SubMsg = cosmwasm_std::SubMsg<StargazeMsgWrapper>;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:imago-finalizer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = deps.api.addr_validate(&msg.owner)?;
    OWNER.save(deps.storage, &owner)?;

    let signer = deps.api.addr_validate(&msg.signer)?;
    SIGNERS.save(deps.storage, signer, &true)?;


    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", info.sender)
        .add_attribute("finalize_owner", msg.owner)
        .add_attribute("finalize_change_signer_to", msg.signer)
        .add_attribute("finalize_change_signer_enabled", &true.to_string()))
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
        ExecuteMsg::ChangeSigner { to, enabled } => execute_change_signer(deps, info, to, enabled),
        ExecuteMsg::Finalize { contract, token_uri, token_id } => execute_finalize(deps, info, contract, token_id, token_uri),
    }
}

pub fn execute_finalize(
    deps: DepsMut,
    info: MessageInfo,
    contract: String, token_id: String, token_uri: String,
) -> Result<Response, ContractError> {
    let valid_signer = SIGNERS.load(deps.storage, info.sender)?;
    if !valid_signer {
        return Err(ContractError::Unauthorized(
            "Sender is not an signer".to_owned(),
        ));
    };
    let finalize_msg = Imago721ExecuteMsg::FinalizeTokenUri {
        token_id: token_id.to_string(),
        token_uri: token_uri.to_string(),
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
    OWNER.save(deps.storage, &new_owner)?;

    Ok(Response::default()
        .add_attribute("action", "finalize_transfer_ownership")
        .add_attribute("finalize_transfer_ownership", &to)
    )
}

pub fn execute_change_signer(
    deps: DepsMut,
    info: MessageInfo,
    to: String,
    enabled: bool,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if owner != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not owner".to_owned(),
        ));
    };

    let validated_addr = deps.api.addr_validate(&to)?;
    SIGNERS.save(deps.storage, validated_addr, &enabled)?;

    Ok(Response::default()
        .add_attribute("action", "finalize_change_signer")
        .add_attribute("finalize_change_signer_to", &to)
        .add_attribute("finalize_change_signer_enabled", &enabled.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let signers = get_signers(deps);
    let owner = OWNER.load(deps.storage)?;

    Ok(ConfigResponse {
        signers,
        owner: owner.to_string(),
    })
}

fn get_signers(
    deps: Deps,
) -> Vec<String> {
    let signers = SIGNERS.range(deps.storage, Option::None, Option::None, Order::Ascending);

    signers
        .filter(|s| (s.as_ref().unwrap().1))
        .map(|s| s.unwrap().0.to_string())
        .collect()
}
