use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

#[account]
pub struct LotteryInfo {
    pub winner: Option<Pubkey>,
    pub entry_fee: u64,
    pub participants: Vec<Pubkey>,
    pub max_participants: u64,
    pub status: u8,
}

impl LotteryInfo {
    pub const MAX_SIZE: usize = 1 + 32 //winner
    + 8
    + 4 +( 32 * 5)
    + 8
    + 1;

    pub fn init(&mut self, entry_fee: u64, max_participants: u64) -> Result<()> {
        self.entry_fee = entry_fee;
        self.winner = None;
        self.max_participants = max_participants;
        self.status = LotteryStatus::Started.to_code();
        Ok(())
    }

    pub fn add_participant(&mut self, new_participant: Pubkey) -> Result<()> {
        let index = self
            .participants
            .iter()
            .position(|participant| *participant == new_participant);

        require!(index == None, ErrorCode::ParticipantAlreadyAdded);
        require!(
            self.participants.len() < self.max_participants.try_into().unwrap(),
            ErrorCode::ListFull,
        );
        self.participants.push(new_participant);

        Ok(())
    }
    pub fn remove_participant(&mut self, current_participant: Pubkey) -> Result<()> {
        let index = self
            .participants
            .iter()
            .position(|participant| *participant == current_participant);

        require!(index != None, ErrorCode::ParticipantNotFound);

        self.participants.remove(index.unwrap());

        Ok(())
    }
}

#[derive(PartialEq, Eq,)]
pub enum LotteryStatus {
    //initial stage
    Started,
    //Winner has been selected
    Completed,
}

impl LotteryStatus {
    pub fn to_code(&self) -> u8 {
        match self {
            LotteryStatus::Started => 0,
            LotteryStatus::Completed => 1,
        }
    }

    pub fn from(val: u8) -> std::result::Result<LotteryStatus, ErrorCode> {
        match val {
            0 => Ok(LotteryStatus::Started),
            1 => Ok(LotteryStatus::Completed),
            _ => Err(ErrorCode::InvalidStatus.into()),
        }
    }
}
