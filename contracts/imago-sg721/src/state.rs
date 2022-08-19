use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use sg721_imago::{CollectionInfo, RoyaltyInfo};

pub const COLLECTION_INFO: Item<CollectionInfo<RoyaltyInfo>> = Item::new("collection_info");

/// Set to true by the minter to indicate the minter creation process is complete
pub const READY: Item<bool> = Item::new("ready");
pub const FINALIZER: Item<Addr> = Item::new("finalizer");
pub const CODE_URI: Item<String> = Item::new("code_uri");