use cosmwasm_std::{Addr};
use cw_storage_plus::{Item};


pub const OWNER: Item<Addr> = Item::new("owner");
pub const SIGNER: Item<Addr> = Item::new("signer");
