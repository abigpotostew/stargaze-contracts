#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult};
use cw2::set_contract_version;

use sg_std::burn_and_distribute_fee;
use sg_std::StargazeMsgWrapper;

use crate::ContractError;
use cw721::ContractInfoResponse;
use cw721_base::ContractError as BaseError;
use cw721_base::TokenInfo as Cw721TokenInfo;
use url::Url;

use crate::msg::{
    CollectionInfoResponse, ExecuteMsg, InstantiateMsg, QueryMsg, RoyaltyInfoResponse,
};
use crate::state::{CollectionInfo, RoyaltyInfo, COLLECTION_INFO};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-721";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CREATION_FEE: u128 = 1_000_000_000;
const MAX_DESCRIPTION_LENGTH: u32 = 512;

type Response = cosmwasm_std::Response<StargazeMsgWrapper>;
pub type Sg721Contract<'a> = cw721_base::Cw721Contract<'a, Empty, StargazeMsgWrapper>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let fee_msgs = burn_and_distribute_fee(&info, CREATION_FEE)?;

    // cw721 instantiation
    let info = ContractInfoResponse {
        name: msg.name,
        symbol: msg.symbol,
    };
    Sg721Contract::default()
        .contract_info
        .save(deps.storage, &info)?;

    let minter = deps.api.addr_validate(&msg.minter)?;
    Sg721Contract::default()
        .minter
        .save(deps.storage, &minter)?;

    // sg721 instantiation
    if msg.collection_info.description.len() > MAX_DESCRIPTION_LENGTH as usize {
        return Err(ContractError::DescriptionTooLong {});
    }

    let image = Url::parse(&msg.collection_info.image)?;

    if let Some(ref external_link) = msg.collection_info.external_link {
        Url::parse(external_link)?;
    }

    let royalty_info: Option<RoyaltyInfo> = match msg.collection_info.royalty_info {
        Some(royalty_info) => Some(RoyaltyInfo {
            payment_address: deps.api.addr_validate(&royalty_info.payment_address)?,
            share: royalty_info.share_validate()?,
        }),
        None => None,
    };

    deps.api.addr_validate(&msg.collection_info.creator)?;

    let collection_info = CollectionInfo {
        creator: msg.collection_info.creator,
        description: msg.collection_info.description,
        image: msg.collection_info.image,
        external_link: msg.collection_info.external_link,
        royalty_info,
    };

    COLLECTION_INFO.save(deps.storage, &collection_info)?;

    Ok(Response::default()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("image", image.to_string())
        .add_messages(fee_msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, BaseError> {
    match msg {
        ExecuteMsg::UpdateTokenURIs { base_token_uri } => {
            execute_update_token_uris(deps, env, info, base_token_uri)
        }
        //TODO change back to Sg721Contract
        // _ => Sg721Contract::default().execute(deps, env, info, msg),
        _ => Ok(Response::default()),
    }
}

fn execute_update_token_uris(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    base_token_uri: String,
) -> Result<Response, BaseError> {
    let sg721_contract = Sg721Contract::default();
    let minter = sg721_contract.minter.load(deps.storage)?;
    // TODO add frozen state
    // let frozen = FROZEN.load(deps.storage)?;
    if info.sender != minter {
        return Err(BaseError::Unauthorized {});
    }

    // TODO add frozen state check
    // if frozen {
    //     Err(ContractError::Frozen {});
    // }

    let token_id = "1".to_string();

    sg721_contract
        .tokens
        .update(deps.storage, &token_id, |token| match token {
            Some(mut token_info) => {
                token_info.token_uri = Some(format!("{}/{}", base_token_uri, token_id));
                token_info.extension = Empty {};
                Ok(token_info)
            }
            None => return Err(ContractError::TokenNotFound { got: token_id }),
        });
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::CollectionInfo {} => to_binary(&query_config(deps)?),
        _ => Sg721Contract::default().query(deps, env, msg.into()),
    }
}

fn query_config(deps: Deps) -> StdResult<CollectionInfoResponse> {
    let info = COLLECTION_INFO.load(deps.storage)?;

    let royalty_info_res: Option<RoyaltyInfoResponse> = match info.royalty_info {
        Some(royalty_info) => Some(RoyaltyInfoResponse {
            payment_address: royalty_info.payment_address.to_string(),
            share: royalty_info.share,
        }),
        None => None,
    };

    Ok(CollectionInfoResponse {
        creator: info.creator,
        description: info.description,
        image: info.image,
        external_link: info.external_link,
        royalty_info: royalty_info_res,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::state::CollectionInfo;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Decimal};
    use sg_std::NATIVE_DENOM;

    #[test]
    fn proper_initialization_no_royalties() {
        let mut deps = mock_dependencies();
        let collection = String::from("collection0");

        let msg = InstantiateMsg {
            name: collection,
            symbol: String::from("BOBO"),
            minter: String::from("minter"),
            collection_info: CollectionInfo {
                creator: String::from("creator"),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: None,
            },
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.messages.len());

        // let's query the collection info
        let res = query(deps.as_ref(), mock_env(), QueryMsg::CollectionInfo {}).unwrap();
        let value: CollectionInfoResponse = from_binary(&res).unwrap();
        assert_eq!("https://example.com/image.png", value.image);
        assert_eq!("Stargaze Monkeys", value.description);
        assert_eq!(
            "https://example.com/external.html",
            value.external_link.unwrap()
        );
        assert_eq!(None, value.royalty_info);
    }

    #[test]
    fn proper_initialization_with_royalties() {
        let mut deps = mock_dependencies();
        let creator = String::from("creator");
        let collection = String::from("collection0");

        let msg = InstantiateMsg {
            name: collection,
            symbol: String::from("BOBO"),
            minter: String::from("minter"),
            collection_info: CollectionInfo {
                creator: String::from("creator"),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: creator.clone(),
                    share: Decimal::percent(10),
                }),
            },
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.messages.len());

        // let's query the collection info
        let res = query(deps.as_ref(), mock_env(), QueryMsg::CollectionInfo {}).unwrap();
        let value: CollectionInfoResponse = from_binary(&res).unwrap();
        assert_eq!(
            Some(RoyaltyInfoResponse {
                payment_address: creator,
                share: Decimal::percent(10),
            }),
            value.royalty_info
        );
    }
}
