use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub ticket_price: u128,
    pub reward_percentage: u8,
    pub admin_percentage: u8,
}

#[cw_serde]
pub struct Entry {
    pub id: u64,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
}
#[cw_serde]
pub enum Status {
    ToDo,
    InProgress,
    Done,
    Cancelled,
}
#[cw_serde]
pub enum Priority {
    None,
    Low,
    Medium,
    High,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const ENTRY_SEQ: Item<u64> = Item::new("entry_seq");
// pub const TICKET_PRICE: Item<u64> = Item::new("ticket_price");
// pub const REWARD_PERCENTAGE: Item<u8> = Item::new("reward_percentage");
// pub const ADMIN_PERCENTAGE: Item<u8> = Item::new("admin_percentage");
pub const SHOOT_DEADLINE_MAPPER: Map<Addr, u64> = Map::new("shoot_deadline_mapper");

pub const LIST: Map<u64, Entry> = Map::new("list");
