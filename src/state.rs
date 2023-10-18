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

pub const CONFIG: Item<Config> = Item::new("config");
pub const SHOOT_DEADLINE_MAPPER: Map<Addr, u64> = Map::new("shoot_deadline_mapper");
