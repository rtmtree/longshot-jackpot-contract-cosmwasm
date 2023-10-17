#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, Addr, Uint128, ensure
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use msg::ContractBalanceResponse;
use std::ops::Add;

mod error;
pub mod msg;
pub mod state;
use crate::error::ContractError;
use crate::msg::{EntryResponse, ExecuteMsg, InstantiateMsg, ListResponse, QueryMsg};
use crate::state::{Config, Entry, Priority, Status, CONFIG, ENTRY_SEQ, LIST, SHOOT_DEADLINE_MAPPER};

// version info for migration
const CONTRACT_NAME: &str = "crates.io:longshot_jackpot";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//consts
const SHOOT_DURATION: u64 = 90; // 90 seconds

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = msg
        .owner
        .and_then(|addr_string| deps.api.addr_validate(addr_string.as_str()).ok())
        .unwrap_or(info.sender);

    let config = Config {
        owner: owner.clone(),
        ticket_price: 0,
        reward_percentage: 80,
        admin_percentage: 4,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::NewEntry {
            description,
            priority,
        } => execute_create_new_entry(deps, info, description, priority),
        ExecuteMsg::UpdateEntry {
            id,
            description,
            status,
            priority,
        } => execute_update_entry(deps, info, id, description, status, priority),
        ExecuteMsg::DeleteEntry { id } => execute_delete_entry(deps, info, id),
        ExecuteMsg::SetTicketPrice { new_ticket_price } => execute_set_ticket_price(deps, info, new_ticket_price),
        ExecuteMsg::SetRewardPercentage { new_reward_percentage } => execute_set_reward_percentage(deps, info, new_reward_percentage),
        ExecuteMsg::SetAdminPercentage { new_admin_percentage } => execute_set_admin_percentage(deps, info, new_admin_percentage),        
        ExecuteMsg::Shoot {} => execute_shoot(deps, info, env),
    }
}

pub fn execute_shoot(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
) -> Result<Response, ContractError> {

    let player = info.sender;      

    let may_shoot_deadline_player = SHOOT_DEADLINE_MAPPER.may_load(deps.storage, player.clone())?;
    // Check if the player is already joined
    match may_shoot_deadline_player {
        Some(shoot_deadline_player) => {
            if shoot_deadline_player != 0 {
                //  Assert that the last shoot deadline is passed
                if shoot_deadline_player > env.block.time.seconds() {
                    return Err(ContractError::ShootDeadlineNotPassed {});
                }
            }
        }
        None => {}
    }

    let ticket_price = CONFIG.load(deps.storage)?.ticket_price;
    // println!("info.funds: {:?}", info.funds);
    ensure!( info.funds[0].amount >= Uint128::from(ticket_price), ContractError::InvalidFund {});

    // Set the shoot deadline for the player
    let cur_timestamp = env.block.time.seconds();
    let shoot_deadline = cur_timestamp.add(SHOOT_DURATION);

    SHOOT_DEADLINE_MAPPER.save(deps.storage, player.clone(), &shoot_deadline)?;

    Ok(Response::new()
        .add_attribute("method", "execute_shoot")
        .add_attribute("deadline", shoot_deadline.to_string())
        .add_attribute("timestamp", cur_timestamp.to_string()))
}

pub fn execute_set_admin_percentage(
    deps: DepsMut,
    info: MessageInfo,
    new_admin_percentage: u8,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }
    CONFIG.update(deps.storage,
        |mut state| -> Result<_, ContractError> {
            state.admin_percentage = new_admin_percentage;
            Ok(state)
        }
    )?;
    Ok(Response::new()
        .add_attribute("method", "execute_set_admin_percentage")
        .add_attribute("new_admin_percentage", new_admin_percentage.to_string()))
}

pub fn execute_set_reward_percentage(
    deps: DepsMut,
    info: MessageInfo,
    new_reward_percentage: u8,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }
    CONFIG.update(deps.storage,
        |mut state| -> Result<_, ContractError> {
            state.reward_percentage = new_reward_percentage;
            Ok(state)
        }
    )?;
    Ok(Response::new()
        .add_attribute("method", "execute_set_reward_percentage")
        .add_attribute("new_reward_percentage", new_reward_percentage.to_string()))
}

pub fn execute_set_ticket_price(
    deps: DepsMut,
    info: MessageInfo,
    new_ticket_price: u128,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }
    CONFIG.update(deps.storage,
        |mut state| -> Result<_, ContractError> {
            state.ticket_price = new_ticket_price;
            Ok(state)
        }
    )?;
    Ok(Response::new()
        .add_attribute("method", "execute_set_ticket_price")
        .add_attribute("new_ticket_price", new_ticket_price.to_string()))
}

pub fn execute_create_new_entry(
    deps: DepsMut,
    info: MessageInfo,
    description: String,
    priority: Option<Priority>,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }
    let id = ENTRY_SEQ.update::<_, cosmwasm_std::StdError>(deps.storage, |id| Ok(id.add(1)))?;
    let new_entry = Entry {
        id,
        description,
        priority: priority.unwrap_or(Priority::None),
        status: Status::ToDo,
    };
    LIST.save(deps.storage, id, &new_entry)?;
    Ok(Response::new()
        .add_attribute("method", "execute_create_new_entry")
        .add_attribute("new_entry_id", id.to_string()))
}

pub fn execute_update_entry(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
    description: Option<String>,
    status: Option<Status>,
    priority: Option<Priority>,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let entry = LIST.load(deps.storage, id)?;
    let updated_entry = Entry {
        id,
        description: description.unwrap_or(entry.description),
        status: status.unwrap_or(entry.status),
        priority: priority.unwrap_or(entry.priority),
    };
    LIST.save(deps.storage, id, &updated_entry)?;
    Ok(Response::new()
        .add_attribute("method", "execute_update_entry")
        .add_attribute("updated_entry_id", id.to_string()))
}

pub fn execute_delete_entry(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    LIST.remove(deps.storage, id);
    Ok(Response::new()
        .add_attribute("method", "execute_delete_entry")
        .add_attribute("deleted_entry_id", id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryEntry { id } => to_binary(&query_entry(deps, id)?),
        QueryMsg::QueryList { start_after, limit } => {
            to_binary(&query_list(deps, start_after, limit)?)
        },
        QueryMsg::QueryConfig { } => to_binary(&query_config(deps)?),
        QueryMsg::QueryShootDeadline { address } => to_binary(&query_shoot_deadline(deps, address)?),
        QueryMsg::QueryBalance { } => to_binary(&query_balance(deps, env)?),
    }
}

fn query_balance(deps: Deps,env: Env) -> StdResult<ContractBalanceResponse> {
    let balance = deps.querier.query_balance(&env.contract.address, &"untrn".to_string())?;
    Ok(ContractBalanceResponse {
        balance: balance.amount.u128(),
    })
}

fn query_config(deps: Deps) -> StdResult<Config> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

fn query_shoot_deadline(deps: Deps, address: Addr) -> StdResult<u64> {
    let shoot_deadline = SHOOT_DEADLINE_MAPPER.load(deps.storage, address)?;
    Ok(shoot_deadline)
}

fn query_entry(deps: Deps, id: u64) -> StdResult<EntryResponse> {
    let entry = LIST.load(deps.storage, id)?;
    Ok(EntryResponse {
        id: entry.id,
        description: entry.description,
        status: entry.status,
        priority: entry.priority,
    })
}

// Limits for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn query_list(deps: Deps, start_after: Option<u64>, limit: Option<u32>) -> StdResult<ListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);
    let entries: StdResult<Vec<_>> = LIST
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect();

    let result = ListResponse {
        entries: entries?.into_iter().map(|l| l.1).collect(),
    };
    Ok(result)
}


pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tes2 {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

#[cfg(test)]
mod tests {
    use crate::msg::ContractBalanceResponse;
    
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_dependencies_with_balances, mock_env, mock_info};
    use cosmwasm_std::{attr, from_binary, Addr, QuerierWrapper, Empty, BalanceResponse,
        QueryRequest,Coin, BankQuery};
    use std::vec::Vec;


    #[test]
    fn test_proper_initialization() {
        let mut deps = mock_dependencies();
        //no owner specified in the instantiation message
        let msg = InstantiateMsg { owner: None };
        let env = mock_env();
        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let state = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(
            state,
            Config {
                owner: Addr::unchecked("creator".to_string()),
                ticket_price: 0,
                reward_percentage: 80,
                admin_percentage: 4,
            }
        );
        //specifying an owner address in the instantiation message
        let msg = InstantiateMsg {
            owner: Some("specified_owner".to_string()),
        };

        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let state = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(
            state,
            Config {
                owner: Addr::unchecked("specified_owner".to_string()),
                ticket_price: 0,
                reward_percentage: 80,
                admin_percentage: 4,
            }
        );
    }

    #[test]
    fn test_set_ticket_price() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg { owner: None };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());
        
        let msg = ExecuteMsg::SetTicketPrice { new_ticket_price: 100 };

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_set_ticket_price"),
                attr("new_ticket_price", "100")
            ]
        );

        let res = query(deps.as_ref(), env.clone(), QueryMsg::QueryConfig {}).unwrap();
        let config: Config = from_binary(&res).unwrap();
        assert_eq!(
            Config {
                owner: Addr::unchecked("creator".to_string()),
                ticket_price: 100,
                reward_percentage: 80,
                admin_percentage: 4,
            },
            config
        );
    }

    #[test]
    fn test_set_percentage() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg { owner: None };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        let msg = ExecuteMsg::SetRewardPercentage { new_reward_percentage: 90 };

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_set_reward_percentage"),
                attr("new_reward_percentage", "90")
            ]
        );

        let res = query(deps.as_ref(), env.clone(), QueryMsg::QueryConfig {}).unwrap();
        let config: Config = from_binary(&res).unwrap();
        assert_eq!(
            Config {
                owner: Addr::unchecked("creator".to_string()),
                ticket_price: 0,
                reward_percentage: 90,
                admin_percentage: 4,
            },
            config
        );

        let msg = ExecuteMsg::SetAdminPercentage { new_admin_percentage: 10 };

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_set_admin_percentage"),
                attr("new_admin_percentage", "10")
            ]
        );

        let res = query(deps.as_ref(), env.clone(), QueryMsg::QueryConfig {}).unwrap();
        let config: Config = from_binary(&res).unwrap();
        assert_eq!(
            Config {
                owner: Addr::unchecked("creator".to_string()),
                ticket_price: 0,
                reward_percentage: 90,
                admin_percentage: 10,
            },
            config
        );
    }

    #[test]
    fn test_shoot_for_free_twice_success() {

        let env = mock_env();
        let mut deps = mock_dependencies_with_balances(&[
            (env.contract.address.as_str(), &[Coin::new(0, "untrn")]),
        ]);
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg { owner: None };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());
                
        let ticket_price = CONFIG.load(deps.as_ref().storage).unwrap().ticket_price;        
        let msg = ExecuteMsg::Shoot { };
        let info_with_funds = mock_info("creator", &[
            Coin {
                denom: "untrn".to_string(),
                amount: Uint128::from(ticket_price),
            }
        ]);
        let res = execute(deps.as_mut(), env.clone(), info_with_funds.clone(), msg).unwrap();

        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_shoot"),
                attr("deadline", env.block.time.seconds().add(SHOOT_DURATION).to_string()),
                attr("timestamp", env.block.time.seconds().to_string())
            ]
        );

        let res = query(deps.as_ref(), env.clone(), QueryMsg::QueryShootDeadline { address: Addr::unchecked("creator".to_string()) }).unwrap();
        let shoot_deadline: u64 = from_binary(&res).unwrap();
        assert_eq!(shoot_deadline, env.block.time.seconds().add(SHOOT_DURATION));

    }

    #[test]
    fn test_shoot_10ntrn_success() {

        let env = mock_env();
        let contract_address = env.contract.address.clone();
        let mut deps = mock_dependencies_with_balances(&[
            (env.contract.address.as_str(), &[Coin::new(1000, "untrn")]),
            ("player", &[Coin::new(100, "untrn")]),
        ]);

        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg { owner: Some("creator".to_string()) };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        //set ticket price
        let msg = ExecuteMsg::SetTicketPrice { new_ticket_price: 10 };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_set_ticket_price"),
                attr("new_ticket_price", "10")
            ]
        );

        let ticket_price = CONFIG.load(deps.as_ref().storage).unwrap().ticket_price;        
        let info_with_funds = mock_info("player", &[
            Coin {
                denom: "untrn".to_string(),
                amount: Uint128::from(ticket_price),
                // amount: Uint128::from(10000000000u128),
            }
        ]);
        // println!("sending ticket_price: {}", info_with_funds.funds[0].amount);
        
        // execute shoot
        let msg = ExecuteMsg::Shoot {};
        let res = execute(deps.as_mut(), env.clone(), info_with_funds.clone(), msg).unwrap();
        // check response
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_shoot"),
                attr("deadline", env.block.time.seconds().add(SHOOT_DURATION).to_string()),
                attr("timestamp", env.block.time.seconds().to_string())
            ]
        );
        // println!("res: {:?}", res);

        //check if deadline is set
        let res = query(deps.as_ref(), env.clone(), QueryMsg::QueryShootDeadline { address: Addr::unchecked("player".to_string()) }).unwrap();
        let shoot_deadline: u64 = from_binary(&res).unwrap();
        assert_eq!(shoot_deadline, env.block.time.seconds().add(SHOOT_DURATION));

        if false {
            // log the balances
            let msg = QueryRequest::Bank(BankQuery::Balance { address: env.contract.address.to_string(), denom: "untrn".to_string() });
            let res = deps.querier.handle_query(&msg).unwrap();
            let balance : BalanceResponse = from_binary(&res.unwrap()).unwrap();
            // println!("final contract bal: {}", balance.amount.amount);
            let msg = QueryRequest::Bank(BankQuery::Balance { address: "player".to_string(), denom: "untrn".to_string() });
            let res = deps.querier.handle_query(&msg).unwrap();
            let balance : BalanceResponse = from_binary(&res.unwrap()).unwrap();
            // println!("final player bal: {}", balance.amount.amount);
        }
    }
}
