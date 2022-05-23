use cosmwasm_std::{Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult, to_binary};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::set_contract_version;
use cw721::ContractInfoResponse;
use sg1::checked_fair_burn;
use sg_std::StargazeMsgWrapper;
use url::Url;
use cw_utils::{maybe_addr};

use crate::ContractError;
use crate::ContractError::Unauthorized;
use crate::msg::{
    CollectionInfoResponse, ExecuteMsg, InstantiateMsg, QueryMsg, RoyaltyInfoResponse,
};
use crate::state::{COLLECTION_INFO, CollectionInfo, FINALIZER, RoyaltyInfo, TOKEN_FINALIZED};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-721-imago";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CREATION_FEE: u128 = 1_000_000_000;
const MAX_DESCRIPTION_LENGTH: u32 = 512;

pub const DEV_ADDRESS:&str= "stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr";

type Response = cosmwasm_std::Response<StargazeMsgWrapper>;
pub type Sg721ImagoContract<'a> = cw721_base::Cw721Contract<'a, Empty, StargazeMsgWrapper>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let addr = maybe_addr(deps.api, Some(DEV_ADDRESS.to_string()));
    let fee_msgs = checked_fair_burn(&info, CREATION_FEE, addr?)?;

    // cw721 instantiation
    let info = ContractInfoResponse {
        name: msg.name,
        symbol: msg.symbol,
    };
    Sg721ImagoContract::default()
        .contract_info
        .save(deps.storage, &info)?;

    let minter = deps.api.addr_validate(&msg.minter)?;
    Sg721ImagoContract::default()
        .minter
        .save(deps.storage, &minter)?;

    // imago
    let finalizer = deps.api.addr_validate(&msg.finalizer)?;
    FINALIZER.save(deps.storage, &finalizer)?;

    // sg721 instantiation
    if msg.collection_info.description.len() > MAX_DESCRIPTION_LENGTH as usize {
        return Err(ContractError::DescriptionTooLong {});
    }

    let image = Url::parse(&msg.collection_info.image)?;


    if let Some(ref external_link) = msg.collection_info.external_link {
        Url::parse(external_link)?;
    }

    Url::parse(&msg.collection_info.code_uri)?;
    // todo validate it is ipfs

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
        code_uri: msg.collection_info.code_uri,
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

fn finalize_token_uri(deps: DepsMut,
                      _env: Env,
                      info: MessageInfo,
                      token_id: String,
                      token_uri: String,
) -> Result<Response, ContractError> {
    //todo
    let finalizer = FINALIZER.load(deps.storage)?;

    if info.sender != finalizer {
        return Err(Unauthorized {});
    }

    let token_finalized = (TOKEN_FINALIZED
        .key(token_id.clone())
        .may_load(deps.storage)?)
        .unwrap_or(false);

    if token_finalized {
        return Err(ContractError::Finalized {});
    }

    Sg721ImagoContract::default()
        .tokens
        .update(deps.storage, &token_id, |token| match token {
            Some(mut token_info) => {
                token_info.token_uri = Some(token_uri);
                token_info.extension = Empty {};
                Ok(token_info)
            }
            None => Err(ContractError::TokenNotFound {
                got: token_id.to_string(),
            }),
        })?;

    return Ok(Response::new()
        .add_attribute("action", "finalize"));
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::FinalizeTokenUri { token_id, token_uri } => finalize_token_uri(deps, env, info, token_id, token_uri),
        _ => Sg721ImagoContract::default()
            .execute(deps, env, info, msg.into())
            .map_err(ContractError::from),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::CollectionInfo {} => to_binary(&query_config(deps)?),
        _ => Sg721ImagoContract::default().query(deps, env, msg.into()),
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
        code_uri: info.code_uri,
        external_link: info.external_link,
        royalty_info: royalty_info_res,
    })
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{coins, Decimal, from_binary};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw721::NftInfoResponse;
    use cw721_base::MintMsg;
    use sg_std::NATIVE_DENOM;

    use crate::state::CollectionInfo;

    use super::*;

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
                code_uri: "ipfs://abc123".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: None,
            },
            finalizer: "finalizer_address".to_string(),
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
        assert_eq!(
            "ipfs://abc123",
            value.code_uri
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
                code_uri: "ipfs://abc123".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: creator.clone(),
                    share: Decimal::percent(10),
                }),
            },
            finalizer: "finalizer_address".to_string(),
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

    #[test]
    fn finalization() {
        let mut deps = mock_dependencies();
        let creator = String::from("creator");
        let collection = String::from("collection0");
        const MINTER: &str = "minter";

        let msg = InstantiateMsg {
            name: collection,
            symbol: String::from("BOBO"),
            minter: String::from("minter"),
            collection_info: CollectionInfo {
                creator: String::from("creator"),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                code_uri: "ipfs://abc123".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: creator.clone(),
                    share: Decimal::percent(10),
                }),
            },
            finalizer: "finalizer_address".to_string(),
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(2, res.messages.len());

        // mint nft
        let token_id = "1".to_string();
        let token_uri = "https://imago.com".to_string();

        let exec_mint_msg = ExecuteMsg::Mint(MintMsg::<Empty> {
            token_id: token_id.clone(),
            owner: String::from("medusa"),
            token_uri: Some(token_uri.clone()),
            extension: Empty {},
        });

        let allowed = mock_info(MINTER, &[]);
        let _ = Sg721ImagoContract::default()
            .execute(deps.as_mut(), mock_env(), allowed.clone(), exec_mint_msg.into())
            .unwrap();

        let query_msg: QueryMsg = QueryMsg::NftInfo {
            token_id: (&token_id).to_string(),
        };

        // confirm response is the same
        let res: NftInfoResponse<Empty> =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
        assert_eq!(res.token_uri, Some(token_uri));

        // update base token uri
        let new_token_uri: String = "ipfs://abc123newuri".to_string();
        let finalize_token_uri_msg = ExecuteMsg::FinalizeTokenUri {
            token_uri: new_token_uri.clone(),
            token_id,
        };
        let _ = execute(
            deps.as_mut(),
            mock_env(),
            allowed.clone(),
            finalize_token_uri_msg,
        )
            .unwrap();

        let res: NftInfoResponse<Empty> =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();

        assert_eq!(
            res.token_uri,
            Some(format!("{}", new_token_uri))
        );
    }
}
