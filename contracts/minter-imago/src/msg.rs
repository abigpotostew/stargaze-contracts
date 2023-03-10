use cosmwasm_std::{Coin, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use sg721_imago::msg::InstantiateMsg as Sg721ImagoInstantiateMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub base_token_uri: String,
    pub num_tokens: u32,
    pub sg721_code_id: u64,
    pub sg721_instantiate_msg: Sg721ImagoInstantiateMsg,
    pub start_time: Timestamp,
    pub per_address_limit: u32,
    pub unit_price: Coin,
    pub whitelist: Option<String>,
    pub dutch_auction_config: Option<DutchAuctionConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DutchAuctionConfig {
    pub end_time: Timestamp,
    pub resting_unit_price: Coin,
    pub decline_period_seconds: u64,
    pub decline_decay: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Mint {},
    SetWhitelist { whitelist: String },
    UpdateStartTime(Timestamp),
    UpdatePerAddressLimit { per_address_limit: u32 },
    MintTo { recipient: String },
    BurnRemaining {},
    UpdatePrice { unit_price: Coin },
    UpdateDutchAuction { dutch_auction_config:DutchAuctionConfig, unit_price:Coin, start_time:Timestamp},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    MintableNumTokens {},
    StartTime {},
    MintPrice {},
    MintCount { address: String },
    // DutchAuctionInfo {},
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
    pub dutch_auction_config: Option<DutchAuctionConfig>,
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

    pub auction_rest_price: Option<Coin>,
    pub auction_end_time: Option<String>,
    pub auction_next_price_timestamp: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MintCountResponse {
    pub address: String,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
