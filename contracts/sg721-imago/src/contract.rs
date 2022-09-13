use cosmwasm_std::{Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult, to_binary};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::set_contract_version;
use cw721::ContractInfoResponse;
use cw_utils::maybe_addr;
use sg1::checked_fair_burn;
use sg_std::StargazeMsgWrapper;
use url::Url;

use crate::ContractError;
use crate::ContractError::Unauthorized;
use crate::msg::{CodeUriResponse, CollectionInfoResponse, ExecuteMsg, InstantiateMsg, QueryMsg, RoyaltyInfoResponse};
use crate::state::{CODE_URI, COLLECTION_INFO, CollectionInfo, FINALIZER, RoyaltyInfo};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-721-imago";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CREATION_FEE: u128 = 1_000_000_000;
const MAX_DESCRIPTION_LENGTH: u32 = 512;

pub const DEV_ADDRESS: &str = "stars1zmqesn4d0gjwhcp2f0j3ptc2agqjcqmuadl6cr";

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

    let maybe_err = validate_code_uri(msg.code_uri.clone());
    if let Some(..) = maybe_err {
        return Err(maybe_err.unwrap());
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
    CODE_URI.save(deps.storage, &msg.code_uri)?;

    Ok(Response::default()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("image", image.to_string())
        .add_messages(fee_msgs))
}

fn validate_code_uri(uri: String) -> Option<ContractError> {
    let parsed_token_uri = Url::parse(&uri);
    if parsed_token_uri.is_err() {
        return Some(ContractError::InvalidCodeUri {});
    }
    if parsed_token_uri.unwrap().scheme() != "ipfs" {
        return Some(ContractError::InvalidCodeUri {});
    }
    None
}

fn finalize_token_uri(deps: DepsMut,
                      _env: Env,
                      info: MessageInfo,
                      token_id: String,
                      token_uri: String,
) -> Result<Response, ContractError> {
    let finalizer = FINALIZER.load(deps.storage)?;

    if info.sender != finalizer {
        return Err(Unauthorized {});
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

    Ok(Response::new()
        .add_attribute("action", "finalize"))
}

fn execute_set_code_uri(deps: DepsMut,
                        _env: Env,
                        info: MessageInfo,
                        uri: String,
) -> Result<Response, ContractError> {
    let config = COLLECTION_INFO.load(deps.storage)?;

    if info.sender != config.creator {
        return Err(Unauthorized {});
    }

    let maybe_err = validate_code_uri(uri.clone());
    if let Some(..) = maybe_err {
        return Err(maybe_err.unwrap());
    }

    CODE_URI.save(deps.storage, &uri)?;

    Ok(Response::new()
        .add_attribute("action", "set_code_uri"))
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
        ExecuteMsg::SetCodeUri { uri } => execute_set_code_uri(deps, env, info, uri),
        _ => Sg721ImagoContract::default()
            .execute(deps, env, info, msg.into())
            .map_err(ContractError::from),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::CollectionInfo {} => to_binary(&query_config(deps)?),
        QueryMsg::CodeUri {} => to_binary(&query_code_uri(deps)?),
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
        external_link: info.external_link,
        royalty_info: royalty_info_res,
    })
}

fn query_code_uri(deps: Deps) -> StdResult<CodeUriResponse> {
    let code_uri = CODE_URI.load(deps.storage)?;

    Ok(CodeUriResponse {
        code_uri,
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
            code_uri: "ipfs://abc123".to_string(),
            collection_info: CollectionInfo {
                creator: String::from("creator"),
                description: String::from("Stargaze Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: None,
            },
            finalizer: "finalizer_address".to_string(),
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages, and fairburn
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(3, res.messages.len());

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
            code_uri: "ipfs://abc123".to_string(),
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
            finalizer: "finalizer_address".to_string(),
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages
        // it also has the dev burn message now
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(3, res.messages.len());

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
        let finalizer = String::from("finalizer_address");
        let collection = String::from("collection0");
        const MINTER: &str = "minter";

        let msg = InstantiateMsg {
            name: collection,
            symbol: String::from("BOBO"),
            minter: String::from("minter"),
            code_uri: "ipfs://abc123".to_string(),
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
            finalizer: finalizer.to_string(),
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages and fair burn
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(3, res.messages.len());

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
        let finalizer_allowed = mock_info(&finalizer, &[]);
        let _ = execute(
            deps.as_mut(),
            mock_env(),
            finalizer_allowed.clone(),
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

    #[test]
    fn set_code_uri() {
        let mut deps = mock_dependencies();
        let creator = String::from("creator");
        let finalizer = String::from("finalizer_address");
        let collection = String::from("collection0");

        let original_code_uri = "ipfs://abc123".to_string();
        let msg = InstantiateMsg {
            name: collection,
            symbol: String::from("BOBO"),
            minter: String::from("minter"),
            code_uri: original_code_uri.clone(),
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
            finalizer: finalizer.to_string(),
        };
        let info = mock_info("creator", &coins(CREATION_FEE, NATIVE_DENOM));

        // make sure instantiate has the burn messages and fair burn
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(3, res.messages.len());

        let query_msg: QueryMsg = QueryMsg::CodeUri {};

        // confirm response is the same
        let res: CodeUriResponse =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
        assert_eq!(res.code_uri, original_code_uri.clone());

        let new_code_uri = "ipfs://xyz987".to_string();

        let exec_seturi_msg = ExecuteMsg::SetCodeUri {
            uri: new_code_uri.clone(),
        };

        let allowed = mock_info(&creator.clone(), &[]);
        let _ = execute(deps.as_mut(), mock_env(), allowed.clone(), exec_seturi_msg)
            .unwrap();

        let query_msg: QueryMsg = QueryMsg::CodeUri {};

        // confirm response is the same
        let res: CodeUriResponse =
            from_binary(&query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap()).unwrap();
        assert_eq!(res.code_uri, new_code_uri.clone());


        //
    }
}
