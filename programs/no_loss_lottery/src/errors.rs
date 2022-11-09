use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    // 0
    /// Invalid instruction data passed in.
    #[msg("Participant already added")]
    ParticipantAlreadyAdded,
    #[msg("Participant Not found or has already withdrawn")]
    ParticipantNotFound,
    #[msg("List Full: Participant can't be added")]
    ListFull,
    #[msg("Not a valid Switchboard account")]
    InvalidSwitchboardAccount,
    #[msg("The max result must not exceed u64")]
    MaxResultExceedsMaximum,
    #[msg("Current round result is empty")]
    EmptyCurrentRoundResult,
    #[msg("Invalid authority account provided.")]
    InvalidAuthorityError,
    #[msg("Invalid VRF account provided.")]
    InvalidVrfAccount,
    #[msg("Invalid Lottery status")]
    InvalidStatus,
    #[msg("Winner has been already selected")]
    WinnerAlreadySelected,
    #[msg("Lottery still on and winner has not been selected yet")]
    LotteryStillOn,
}