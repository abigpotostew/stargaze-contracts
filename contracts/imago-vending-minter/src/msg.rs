use cosmwasm_std::{Coin, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sg721_imago::{CollectionInfo, RoyaltyInfoResponse};
use vending_factory::{msg::VendingMinterCreateMsg, state::VendingMinterParams};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ImagoVendingMinterInitMsgExtension {
    pub base_token_uri: String,
    pub start_time: Timestamp,
    pub num_tokens: u32,
    pub unit_price: Coin,
    pub per_address_limit: u32,
    pub whitelist: Option<String>,
    pub finalizer: String,
    pub code_uri: String,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CreateMinterMsg<T> {
    pub init_msg: T,
    pub collection_params: CollectionParams,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollectionParams {
    /// The collection code id
    pub code_id: u64,
    pub name: String,
    pub symbol: String,
    pub info: CollectionInfo<RoyaltyInfoResponse>,
}
pub type ImagoVendingMinterCreateMsg = CreateMinterMsg<ImagoVendingMinterInitMsgExtension>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub create_msg: VendingMinterCreateMsg,
    pub params: VendingMinterParams,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Mint {},
    SetWhitelist { whitelist: String },
    UpdateStartTime(Timestamp),
    UpdatePerAddressLimit { per_address_limit: u32 },
    MintTo { recipient: String },
    MintFor { token_id: u32, recipient: String },
    Withdraw {},
    BurnRemaining {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    MintableNumTokens {},
    StartTime {},
    MintPrice {},
    MintCount { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub admin: String,
    pub base_token_uri: String,
    pub num_tokens: u32,
    pub per_address_limit: u32,
    pub sg721_address: String,
    pub sg721_code_id: u64,
    pub start_time: Timestamp,
    pub unit_price: Coin,
    pub whitelist: Option<String>,
    // pub factory: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MintableNumTokensResponse {
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StartTimeResponse {
    pub start_time: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MintPriceResponse {
    pub public_price: Coin,
    pub whitelist_price: Option<Coin>,
    pub current_price: Coin,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MintCountResponse {
    pub address: String,
    pub count: u32,
}