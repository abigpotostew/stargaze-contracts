use cosmwasm_std::{Coin, StdError, Timestamp};
use cw_utils::PaymentError;
use sg1::FeeError;
use thiserror::Error;
use url::ParseError;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Invalid reply ID")]
    InvalidReplyID {},

    #[error("Not enough funds sent")]
    NotEnoughFunds {},

    #[error("TooManyCoins")]
    TooManyCoins {},

    #[error("IncorrectPaymentAmount {0} != {1}")]
    IncorrectPaymentAmount(Coin, Coin),

    #[error("InvalidNumTokens {max}, min: 1")]
    InvalidNumTokens { max: u32, min: u32 },

    #[error("Sold out")]
    SoldOut {},

    #[error("InvalidDenom {expected} got {got}")]
    InvalidDenom { expected: String, got: String },

    #[error("Minimum network mint price {expected} got {got}")]
    InsufficientMintPrice { expected: u128, got: u128 },

    #[error("Invalid address {addr}")]
    InvalidAddress { addr: String },

    #[error("Invalid token id")]
    InvalidTokenId {},

    #[error("AlreadyStarted")]
    AlreadyStarted {},

    #[error("BeforeGenesisTime")]
    BeforeGenesisTime {},

    #[error("WhitelistAlreadyStarted")]
    WhitelistAlreadyStarted {},

    #[error("InvalidStartTime {0} < {1}")]
    InvalidStartTime(Timestamp, Timestamp),

    #[error("End time must be after start time")]
    InvalidEndTime {},

    #[error("Resting price must be less than unit price")]
    InvalidRestingPrice {},

    #[error("Decline period must be greater than 0 and less than the auction duration")]
    InvalidDeclinePeriodSeconds {},

    #[error("Dutch auction decline decay must be less than 1000000")]
    InvalidDutchAuctionDeclineDecay {},

    #[error("Instantiate sg721 error")]
    InstantiateSg721Error {},

    #[error("Invalid base token URI (must be publicworks.art url)")]
    InvalidBaseTokenURI {},

    #[error("address not on whitelist: {addr}")]
    NotWhitelisted { addr: String },

    #[error("Minting has not started yet")]
    BeforeMintStartTime {},

    #[error("Invalid minting limit per address. max: {max}, min: 1, got: {got}")]
    InvalidPerAddressLimit { max: u32, min: u32, got: u32 },

    #[error("Max minting limit per address exceeded")]
    MaxPerAddressLimitExceeded {},

    #[error("Token id: {token_id} already sold")]
    TokenIdAlreadySold { token_id: u32 },

    #[error("ZeroBalance")]
    ZeroBalance {},

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    Fee(#[from] FeeError),

    #[error("InvalidCodeUri")]
    InvalidCodeUri {},
}

impl From<ParseError> for ContractError {
    fn from(_err: ParseError) -> ContractError {
        ContractError::InvalidBaseTokenURI {}
    }
}
