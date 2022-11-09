pub mod errors;
pub mod instructions;
pub mod state;

pub use instructions::*;
pub use state::*;

use anchor_lang::prelude::*;

// use lottery::LotteryInfo;

declare_id!("GFVxvAzesa7PdEhndi4YZLQGKT7Hdgc6RLyZSCFNaMP6");

const MAX_RESULT: u64 = u64::MAX;

const STATE_SEED: &[u8] = b"STATE";

#[program]
pub mod no_loss_lottery {

    use super::*;

    pub fn initialize_lottery(
        ctx: Context<InitializeLottery>,
        params: InitializeLotteryParams,
    ) -> Result<()> {
        instructions::lottery::initialize_lottery(ctx, params)
    }
    pub fn enter_lottery(ctx: Context<EnterLottery>) -> Result<()> {
        instructions::lottery::enter_lottery(ctx)
    }

    pub fn withdraw<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, WithdrawTokensFromLendingPool<'info>>,
        vault_signer_bump: u8,
    ) -> Result<()> {
        instructions::lottery::withdraw(ctx, vault_signer_bump)
    }
    pub fn deposit<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, DepositTokensToLendingPool<'info>>,
        vault_signer_bump: u8,
    ) -> Result<()> {
        instructions::lottery::deposit(ctx, vault_signer_bump)
    }

    pub fn choose_winner(ctx: Context<ChooseWinner>) -> Result<()> {
        instructions::lottery::choose_winner(ctx)
    }
    pub fn withdraw_user_tokens(ctx: Context<WithdrawUserTokens>) -> Result<()> {
        instructions::lottery::withdraw_user_tokens(ctx)
    }
    #[access_control(ctx.accounts.validate(&ctx))]
    pub fn update_result(ctx: Context<UpdateResult>) -> Result<()> {
        instructions::randomness::update_result(ctx)
    }

    #[access_control(ctx.accounts.validate(&ctx, &params))]
    pub fn request_result(ctx: Context<RequestResult>, params: RequestResultParams) -> Result<()> {
        instructions::randomness::request_result(ctx, params)
    }

    pub fn close_accounts(ctx: Context<CloseAccounts>) -> Result<()> {
        instructions::lottery::close_accounts(ctx)
    }
}
