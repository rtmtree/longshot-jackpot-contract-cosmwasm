use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Invalid fund")]
    InvalidFund {},

    #[error("Insufficient balance")]
    InsufficientBalance {},

    #[error("Invalid price")]
    InvalidPriceIndex0 {
        expected_amount: u128,
        expected_denom: String,
        actual_amount: u128,
        actual_denom: String,
    },

    #[error("Deadline not passed")]
    ShootDeadlineNotPassed {},

    #[error("Deadline passed")]
    ShootDeadlinePassed {},

    #[error("Player not joined")]
    PlayerNotJoined {},
}
