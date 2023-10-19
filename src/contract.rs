#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    ensure, entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use cw2::set_contract_version;
use cw_asset::Asset;
use std::ops::Add;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ContractBalanceResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    ShootDeadlineResponse,
};
use crate::state::{Config, CONFIG, MAIN_DENOM, SHOOT_DEADLINE_MAPPER};

// version info for migration
const CONTRACT_NAME: &str = "crates.io:longshot_jackpot";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// constants
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
        shoot_duration: 90,
    };

    CONFIG.save(deps.storage, &config)?;

    // main denom, depends on the chain
    // "uosmo" for osmosis,
    // "untrn" for neutron,
    // "usei" for sei,
    // "uluna" for terra,
    MAIN_DENOM.save(deps.storage, &msg.main_denom)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", owner)
        .add_attribute("main_denom", msg.main_denom))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SetTicketPrice { new_ticket_price } => {
            execute_set_ticket_price(deps, info, new_ticket_price)
        }
        ExecuteMsg::SetRewardPercentage {
            new_reward_percentage,
        } => execute_set_reward_percentage(deps, info, new_reward_percentage),
        ExecuteMsg::SetAdminPercentage {
            new_admin_percentage,
        } => execute_set_admin_percentage(deps, info, new_admin_percentage),
        ExecuteMsg::Shoot {} => execute_shoot(deps, info, env),
        ExecuteMsg::GoalShot { player_address } => {
            execute_goal_shot(deps, info, env, player_address)
        }
    }
}

pub fn execute_shoot(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
) -> Result<Response, ContractError> {
    let player = info.sender;

    // === Uncomment this to let user wait til the deadline pass before reshoot ===
    // Check if the player is already joined
    // let may_shoot_deadline_player = SHOOT_DEADLINE_MAPPER.may_load(deps.storage, player.clone())?;
    // match may_shoot_deadline_player {
    //     Some(shoot_deadline_player) => {
    //         if shoot_deadline_player != 0 {
    //             //  Assert that the last shoot deadline is passed
    //             if shoot_deadline_player > env.block.time.seconds() {
    //                 return Err(ContractError::ShootDeadlineNotPassed {});
    //             }
    //         }
    //     }
    //     None => {}
    // }
    // ============================================================================

    // Check if the player has enough funds to shoot
    ensure!(!info.funds.is_empty(), ContractError::InvalidFund {});
    let cur_ticket_price = CONFIG.load(deps.storage)?.ticket_price;
    let main_denom = MAIN_DENOM.load(deps.storage)?;
    ensure!(
        info.funds[0].denom == main_denom.to_string()
            && info.funds[0].amount.u128() == cur_ticket_price,
        ContractError::InvalidPriceIndex0 {
            expected_denom: main_denom.to_string(),
            expected_amount: cur_ticket_price,
            actual_denom: info.funds[0].denom.clone(),
            actual_amount: info.funds[0].amount.u128(),
        }
    );

    // Set the shoot deadline for the player
    let cur_timestamp = env.block.time.seconds();
    let shoot_deadline = cur_timestamp.add(SHOOT_DURATION);
    SHOOT_DEADLINE_MAPPER.save(deps.storage, player.clone(), &shoot_deadline)?;

    Ok(Response::new()
        .add_attribute("method", "execute_shoot")
        .add_attribute("shoot_deadline", shoot_deadline.to_string()))
}

pub fn execute_goal_shot(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    player_address: Addr,
) -> Result<Response, ContractError> {
    let owner = CONFIG.load(deps.storage)?.owner;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    // Assert that player joined the game
    let may_shoot_deadline_player =
        SHOOT_DEADLINE_MAPPER.may_load(deps.storage, player_address.clone())?;
    match may_shoot_deadline_player {
        Some(shoot_deadline_player) => {
            // Assert that the shoot deadline is not passed
            if env.block.time.seconds() >= shoot_deadline_player {
                return Err(ContractError::ShootDeadlinePassed {});
            }
        }
        None => {
            return Err(ContractError::PlayerNotJoined {});
        }
    }

    // Get how much reward the player should get
    let config = CONFIG.load(deps.storage)?;
    let main_denom = MAIN_DENOM.load(deps.storage)?;
    let reward_percentage = config.reward_percentage;
    let admin_percentage = config.admin_percentage;
    let contract_balance = deps
        .querier
        .query_balance(&env.contract.address, &main_denom.to_string())?
        .amount
        .u128();
    let reward_amount = contract_balance * reward_percentage as u128 / 100;
    let admin_amount = contract_balance * admin_percentage as u128 / 100;

    // Init response
    let res = Response::new()
        .add_attribute("method", "goal_shot")
        .add_attribute("pre_balance", contract_balance.to_string());

    let mut attrs = vec![];
    let mut msgs = vec![];
    // Transfer reward to the admin
    if admin_amount > 0 {
        let admin = config.owner;
        let asset = Asset::native(main_denom.clone(), admin_amount);
        attrs.push(("reward_transfer_to_admin", admin_amount.to_string()));
        msgs.push(asset.transfer_msg(admin)?);
    }

    if reward_amount > 0 {
        let asset = Asset::native(main_denom, reward_amount);
        attrs.push(("reward_transfer", reward_amount.to_string()));
        msgs.push(asset.transfer_msg(player_address)?);
    }

    Ok(res.add_attributes(attrs).add_messages(msgs))
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
    CONFIG.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.admin_percentage = new_admin_percentage;
        Ok(state)
    })?;
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
    CONFIG.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.reward_percentage = new_reward_percentage;
        Ok(state)
    })?;
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
    CONFIG.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.ticket_price = new_ticket_price;
        Ok(state)
    })?;
    Ok(Response::new()
        .add_attribute("method", "execute_set_ticket_price")
        .add_attribute("new_ticket_price", new_ticket_price.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryConfig {} => to_binary(&query_config(deps)?),
        QueryMsg::QueryShootDeadline { address } => {
            to_binary(&query_shoot_deadline(deps, address)?)
        }
        QueryMsg::QueryBalance {} => to_binary(&query_balance(deps, env)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        ticket_price: config.ticket_price,
        reward_percentage: config.reward_percentage,
        admin_percentage: config.admin_percentage,
        shoot_duration: config.shoot_duration,
    })
}

fn query_shoot_deadline(deps: Deps, address: Addr) -> StdResult<ShootDeadlineResponse> {
    let shoot_deadline = SHOOT_DEADLINE_MAPPER.load(deps.storage, address)?;
    Ok(ShootDeadlineResponse {
        shoot_deadline: shoot_deadline,
    })
}

fn query_balance(deps: Deps, env: Env) -> StdResult<ContractBalanceResponse> {
    let main_denom = MAIN_DENOM.load(deps.storage)?;
    let balance = deps
        .querier
        .query_balance(&env.contract.address, &main_denom.to_string())?;
    Ok(ContractBalanceResponse {
        amount: balance.amount.u128(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balances, mock_env, mock_info,
    };
    use cosmwasm_std::{attr, from_binary, Addr, Coin, Timestamp, Uint128};

    #[test]
    fn test_proper_initialization() {
        let mut deps = mock_dependencies();
        //no owner specified in the instantiation message
        let msg = InstantiateMsg {
            owner: None,
            main_denom: "usei".to_string(),
        };
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
                shoot_duration: 90
            }
        );
        //specifying an owner address in the instantiation message
        let msg = InstantiateMsg {
            owner: Some("specified_owner".to_string()),
            main_denom: "usei".to_string(),
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
                shoot_duration: 90
            }
        );
    }

    #[test]
    fn test_set_ticket_price() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            owner: None,
            main_denom: "usei".to_string(),
        };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        let msg = ExecuteMsg::SetTicketPrice {
            new_ticket_price: 100,
        };

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
                shoot_duration: 90
            },
            config
        );
    }

    #[test]
    fn test_set_percentage() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            owner: None,
            main_denom: "usei".to_string(),
        };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        let msg = ExecuteMsg::SetRewardPercentage {
            new_reward_percentage: 90,
        };

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
                shoot_duration: 90
            },
            config
        );

        let msg = ExecuteMsg::SetAdminPercentage {
            new_admin_percentage: 10,
        };

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
                shoot_duration: 90
            },
            config
        );
    }

    #[test]
    fn test_shoot_for_free_twice_success() {
        let env = mock_env();
        let main_denom = "usei";
        let mut deps = mock_dependencies_with_balances(&[(
            env.contract.address.as_str(),
            &[Coin::new(0, main_denom)],
        )]);
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            owner: None,
            main_denom: main_denom.to_string(),
        };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        let ticket_price = CONFIG.load(deps.as_ref().storage).unwrap().ticket_price;
        let msg = ExecuteMsg::Shoot {};
        let info_with_funds = mock_info(
            "creator",
            &[Coin {
                denom: main_denom.to_string(),
                amount: Uint128::from(ticket_price),
            }],
        );
        let res = execute(deps.as_mut(), env.clone(), info_with_funds.clone(), msg).unwrap();

        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_shoot"),
                attr(
                    "shoot_deadline",
                    env.block.time.seconds().add(SHOOT_DURATION).to_string()
                ),
            ]
        );

        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::QueryShootDeadline {
                address: Addr::unchecked("creator".to_string()),
            },
        )
        .unwrap();
        let shoot_deadline: u64 = from_binary::<ShootDeadlineResponse>(&res)
            .unwrap()
            .shoot_deadline;
        assert_eq!(shoot_deadline, env.block.time.seconds().add(SHOOT_DURATION));
    }

    #[test]
    fn test_shoot_10ntrn_success() {
        let env = mock_env();
        let main_denom = "usei";
        let mut deps = mock_dependencies_with_balances(&[
            (
                env.contract.address.as_str(),
                &[Coin::new(1000, main_denom)],
            ),
            ("player", &[Coin::new(100, main_denom)]),
        ]);

        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            owner: Some("creator".to_string()),
            main_denom: main_denom.to_string(),
        };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // set ticket price
        let msg = ExecuteMsg::SetTicketPrice {
            new_ticket_price: 10,
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_set_ticket_price"),
                attr("new_ticket_price", "10")
            ]
        );

        let ticket_price = CONFIG.load(deps.as_ref().storage).unwrap().ticket_price;
        let info_with_funds = mock_info(
            "player",
            &[Coin {
                denom: main_denom.to_string(),
                amount: Uint128::from(ticket_price),
            }],
        );

        // execute shoot
        let msg = ExecuteMsg::Shoot {};
        let res = execute(deps.as_mut(), env.clone(), info_with_funds.clone(), msg).unwrap();
        // check response
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_shoot"),
                attr(
                    "shoot_deadline",
                    env.block.time.seconds().add(SHOOT_DURATION).to_string()
                )
            ]
        );

        // check if deadline is set
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::QueryShootDeadline {
                address: Addr::unchecked("player".to_string()),
            },
        )
        .unwrap();
        let shoot_deadline: u64 = from_binary::<ShootDeadlineResponse>(&res)
            .unwrap()
            .shoot_deadline;
        assert_eq!(shoot_deadline, env.block.time.seconds().add(SHOOT_DURATION));
    }

    #[test]
    fn test_shoot_10ntrn_and_goal_shot_success() {
        let mut env = mock_env();
        let main_denom = "usei";
        let mut deps = mock_dependencies_with_balances(&[
            (env.contract.address.as_str(), &[Coin::new(100, main_denom)]),
            ("player", &[Coin::new(100, main_denom)]),
        ]);

        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            owner: Some("creator".to_string()),
            main_denom: main_denom.to_string(),
        };

        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());

        // set ticket price
        let msg = ExecuteMsg::SetTicketPrice {
            new_ticket_price: 100,
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_set_ticket_price"),
                attr("new_ticket_price", "100")
            ]
        );

        let ticket_price = CONFIG.load(deps.as_ref().storage).unwrap().ticket_price;
        let info_with_funds = mock_info(
            "player",
            &[Coin {
                denom: main_denom.to_string(),
                amount: Uint128::from(ticket_price),
            }],
        );

        // execute shoot
        let msg = ExecuteMsg::Shoot {};
        let res = execute(deps.as_mut(), env.clone(), info_with_funds.clone(), msg).unwrap();
        // check response
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "execute_shoot"),
                attr(
                    "shoot_deadline",
                    env.block.time.seconds().add(SHOOT_DURATION).to_string()
                )
            ]
        );

        // check if deadline is set
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::QueryShootDeadline {
                address: Addr::unchecked("player".to_string()),
            },
        )
        .unwrap();
        let shoot_deadline: u64 = from_binary::<ShootDeadlineResponse>(&res)
            .unwrap()
            .shoot_deadline;
        assert_eq!(shoot_deadline, env.block.time.seconds().add(SHOOT_DURATION));

        env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 50);

        // calculate reward should be 80% of the contract balance
        let res = query(deps.as_ref(), env.clone(), QueryMsg::QueryBalance {}).unwrap();
        let contract_balance: ContractBalanceResponse = from_binary(&res).unwrap();
        let reward_amount = contract_balance.amount * 80 / 100;
        let admin_amount = contract_balance.amount * 4 / 100;

        // goal shot
        let msg = ExecuteMsg::GoalShot {
            player_address: Addr::unchecked("player".to_string()),
        };
        let info = mock_info("creator", &[]);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // check response
        assert_eq!(
            res.attributes,
            vec![
                attr("method", "goal_shot"),
                attr("pre_balance", contract_balance.amount.to_string()),
                attr("reward_transfer_to_admin", admin_amount.to_string()),
                attr("reward_transfer", reward_amount.to_string()),
            ]
        );
    }
}
