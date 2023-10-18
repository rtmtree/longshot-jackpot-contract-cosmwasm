use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Shoot {},
    GoalShot { player_address: Addr },
    SetTicketPrice { new_ticket_price: u128 },
    SetRewardPercentage { new_reward_percentage: u8 },
    SetAdminPercentage { new_admin_percentage: u8 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    QueryConfig {},
    #[returns(ShootDeadlineResponse)]
    QueryShootDeadline { address: Addr },
    #[returns(ContractBalanceResponse)]
    QueryBalance {},
}

// We define a custom struct for each query response
#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub ticket_price: u64,
    pub reward_percentage: u8,
    pub admin_percentage: u8,
}

#[cw_serde]
pub struct ShootDeadlineResponse {
    pub shoot_deadline: u64,
}

#[cw_serde]
pub struct ContractBalanceResponse {
    pub amount: u128,
}
