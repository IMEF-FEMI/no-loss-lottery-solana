use std::mem;

use crate::{VrfClient, STATE_SEED, LotteryStatus};
use crate::errors::ErrorCode;
use crate::{
    utils::{LOTTERY_INFO_STR, VAULT_SIGNER_STR},
    LotteryInfo, MAX_RESULT,
};
use anchor_lang::{prelude::*, solana_program};
use anchor_lang::solana_program::program::{invoke, invoke_signed};

use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, Transfer},
};
use port_variable_rate_lending_instructions::instruction::{
    deposit_reserve_liquidity, redeem_reserve_collateral, refresh_reserve,
};
use switchboard_v2::{SWITCHBOARD_PROGRAM_ID, VrfAccountData};

#[access_control(ctx.accounts.validate(&ctx, &params))]
pub fn initialize_lottery(
    ctx: Context<InitializeLottery>,
    params: InitializeLotteryParams,
) -> Result<()> {
    InitializeLottery::actuate(&ctx, &params)?;
    LotteryInfo::init(
        &mut ctx.accounts.lottery_acct,
        params.entry_fee,
        params.max_participants,
    )?;
    Ok(())
}

pub fn enter_lottery(ctx: Context<EnterLottery>) -> Result<()> {
    // transfer tokens from user to source_liquidity_vault
    anchor_spl::token::transfer(
        ctx.accounts.transfer_tokens_to_source_liquidity_vault(),
        ctx.accounts.lottery_acct.entry_fee,
    )
    .expect("transfer failed");
    LotteryInfo::add_participant(&mut ctx.accounts.lottery_acct, ctx.accounts.user.key())?;
    Ok(())
}
pub fn deposit<'a, 'b, 'c, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, DepositTokensToLendingPool<'info>>,
    vault_signer_bump: u8,
) -> Result<()> {
    let refresh_ix = refresh_reserve(
        ctx.accounts.lending_program.key(),
        ctx.accounts.reserve.key(),
        anchor_lang::solana_program::program_option::COption::Some(
            ctx.accounts.reserve_liquidity_oracle.key(),
        ),
    );
    let mut accounts = vec![
        ctx.accounts.lending_program.to_account_info(),
        ctx.accounts.reserve.to_account_info(),
        ctx.accounts.reserve_liquidity_oracle.to_account_info(),
        ctx.accounts.clock.to_account_info(),
    ];
    accounts.extend_from_slice(&ctx.remaining_accounts[..]);

    invoke(&refresh_ix, &accounts)?;

    let deposit_ix = deposit_reserve_liquidity(
        ctx.accounts.lending_program.key(),
        ctx.accounts.source_liquidity_vault.amount,
        ctx.accounts.source_liquidity_vault.key(),
        ctx.accounts.destination_collateral_vault.key(),
        ctx.accounts.reserve.key(),
        ctx.accounts.reserve_liquidity_supply.key(),
        ctx.accounts.reserve_collateral_mint.key(),
        ctx.accounts.lending_market.key(),
        ctx.accounts.authority.key(),
    );
    let mut accounts = vec![
        ctx.accounts.lending_program.to_account_info(),
        ctx.accounts.source_liquidity_vault.to_account_info(),
        ctx.accounts.destination_collateral_vault.to_account_info(),
        ctx.accounts.reserve.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.lending_market.to_account_info(),
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.clock.to_account_info(),
    ];
    // let mut accounts = ctx.accounts.to_account_infos();
    accounts.extend_from_slice(&ctx.remaining_accounts[..]);
    let pda_seeds = &[VAULT_SIGNER_STR.as_bytes(), &[vault_signer_bump]];
    // invoke(&deposit_ix, &account)?;
    invoke_signed(&deposit_ix, &accounts, &[pda_seeds.as_ref()])?;

    Ok(())
}

pub fn withdraw<'a, 'b, 'c, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, WithdrawTokensFromLendingPool<'info>>,
    vault_signer_bump: u8,
) -> Result<()> {

    let refresh_ix = refresh_reserve(
        ctx.accounts.lending_program.key(),
        ctx.accounts.reserve.key(),
        anchor_lang::solana_program::program_option::COption::Some(
            ctx.accounts.reserve_liquidity_oracle.key(),
        ),
    );
    let mut accounts = vec![
        ctx.accounts.lending_program.to_account_info(),
        ctx.accounts.reserve.to_account_info(),
        ctx.accounts.reserve_liquidity_oracle.to_account_info(),
        ctx.accounts.clock.to_account_info(),
    ];
    accounts.extend_from_slice(&ctx.remaining_accounts[..]);

    invoke(&refresh_ix, &accounts)?;

    let withdraw_ix = redeem_reserve_collateral(
        ctx.accounts.lending_program.key(),
        ctx.accounts.destination_collateral_vault.amount,
        ctx.accounts.destination_collateral_vault.key(),
        ctx.accounts.source_liquidity_vault.key(),
        ctx.accounts.reserve.key(),
        ctx.accounts.reserve_collateral_mint.key(),
        ctx.accounts.reserve_liquidity_supply.key(),
        ctx.accounts.lending_market.key(),
        ctx.accounts.authority.key(),
    );
    let mut accounts = vec![
        ctx.accounts.lending_program.to_account_info(),
        ctx.accounts.destination_collateral_vault.to_account_info(),
        ctx.accounts.source_liquidity_vault.to_account_info(),
        ctx.accounts.reserve.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.lending_market.to_account_info(),
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.clock.to_account_info(),
    ];
    // let mut accounts = ctx.accounts.to_account_infos();
    accounts.extend_from_slice(&ctx.remaining_accounts[..]);
    let pda_seeds = &[VAULT_SIGNER_STR.as_bytes(), &[vault_signer_bump]];
    // invoke(&withdraw_ix, &account)?;
    invoke_signed(&withdraw_ix, &accounts, &[pda_seeds.as_ref()])?;

    Ok(())
}

pub fn withdraw_user_tokens(
    ctx: Context<WithdrawUserTokens>
) -> Result<()> {
    require!(
        LotteryStatus::from(ctx.accounts.lottery_acct.status).unwrap() == LotteryStatus::Completed, 
        ErrorCode::LotteryStillOn,
    );
    let  winner = ctx.accounts.lottery_acct.winner.unwrap();
    let mut amount_to_pay = ctx.accounts.lottery_acct.entry_fee;
    if ctx.accounts.user.key() == winner {
        let amount_to_pay_non_winners = amount_to_pay * (ctx.accounts.lottery_acct.max_participants - 1);
         amount_to_pay = ctx.accounts.source_liquidity_vault.amount - amount_to_pay_non_winners; //includes extra made from lending investment

    }
    // transfer tokens back to user
    let bump = *ctx.bumps.get("vault_signer").unwrap();
    let pda_seeds = &[VAULT_SIGNER_STR.as_bytes(), &[bump]];

    anchor_spl::token::transfer(
        ctx.accounts.transfer_tokens_from_source_liquidity_vault().with_signer(&[pda_seeds.as_ref()]),
        amount_to_pay,
    )
    .expect("transfer failed");
    LotteryInfo::remove_participant(&mut ctx.accounts.lottery_acct, ctx.accounts.user.key())?;
    Ok(())
}

pub fn choose_winner(
    ctx: Context<ChooseWinner>,
) -> Result<()>{
    require!(
        LotteryStatus::from(ctx.accounts.lottery_acct.status).unwrap() == LotteryStatus::Started, 
        ErrorCode::WinnerAlreadySelected,
    );
    let state = ctx.accounts.state.load()?;
   
    let lottery_winner = ctx.accounts.lottery_acct.participants[state.result as usize];
    ctx.accounts.lottery_acct.winner = Some(lottery_winner);
    ctx.accounts.lottery_acct.status = LotteryStatus::Completed.to_code();
    Ok(())
}
pub fn close_accounts(
    _ctx: Context<CloseAccounts>
) -> Result<()> {

    Ok(())
}

#[derive(Accounts)]
pub struct ChooseWinner<'info> {
    #[account(
        mut,
        seeds = [LOTTERY_INFO_STR.as_bytes(),],
        bump,
    )]
    pub lottery_acct: Box<Account<'info, LotteryInfo>>,
    #[account(mut, 
        has_one = vrf @ ErrorCode::InvalidVrfAccount
    )]
    pub state: AccountLoader<'info, VrfClient>,
    #[account(
        constraint = 
            *vrf.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub vrf: AccountLoader<'info, VrfAccountData>,
}

#[derive(Accounts)]
pub struct WithdrawUserTokens<'info> {
    source_liquidity_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint=source_liquidity_mint,
        token::authority=user,
    )]
    user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint=source_liquidity_mint,
        associated_token::authority=vault_signer,
    )]
    source_liquidity_vault: Box<Account<'info, TokenAccount>>,
   
    /// CHECK: This is the vault signer Acct
    #[account(
            seeds = [VAULT_SIGNER_STR.as_bytes()],
            bump,
        )]
    vault_signer: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [LOTTERY_INFO_STR.as_bytes(),],
        bump,

    )]
    lottery_acct: Box<Account<'info, LotteryInfo>>,
    #[account(mut)]
    user: Signer<'info>,
    token_program: Program<'info, Token>,
    associated_token_program: Program<'info, AssociatedToken>,
    rent: Sysvar<'info, Rent>,
    system_program: Program<'info, System>,
}
impl<'info> WithdrawUserTokens<'info> {
    pub fn transfer_tokens_from_source_liquidity_vault(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let transfer_acct = Transfer {
            to: self.user_token_account.to_account_info().clone(),
            from: self.source_liquidity_vault.to_account_info().clone(),
            authority: self.vault_signer.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), transfer_acct)
    }
}
#[derive(Accounts)]
pub struct CloseAccounts<'info> {
    #[account(
        mut,
        seeds = [LOTTERY_INFO_STR.as_bytes(),],
        bump,
        close = user,
    )]
    lottery_acct: Box<Account<'info, LotteryInfo>>,
    #[account(
        mut,
        seeds = [
            STATE_SEED, 
            vrf.key().as_ref(),
            user.key().as_ref(),
        ],
        close = user,
        bump,
    )]
    state: AccountLoader<'info, VrfClient>,
    #[account(

        constraint = 
            *vrf.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount,
    )]
    vrf: AccountLoader<'info, VrfAccountData>,
    #[account(mut)]
    user: Signer<'info>,
}
#[derive(Accounts)]
pub struct EnterLottery<'info> {
    source_liquidity_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint=source_liquidity_mint,
        token::authority=user,
    )]
    user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint=source_liquidity_mint,
        associated_token::authority=vault_signer,
    )]
    source_liquidity_vault: Box<Account<'info, TokenAccount>>,
    destination_collateral_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint=destination_collateral_mint,
        associated_token::authority=vault_signer,
    )]
    destination_collateral_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is the vault signer Acct
    #[account(
            seeds = [VAULT_SIGNER_STR.as_bytes()],
            bump,
        )]
    vault_signer: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [LOTTERY_INFO_STR.as_bytes(),],
        bump,
    )]
    lottery_acct: Box<Account<'info, LotteryInfo>>,
    #[account(mut)]
    user: Signer<'info>,
    token_program: Program<'info, Token>,
    associated_token_program: Program<'info, AssociatedToken>,
    rent: Sysvar<'info, Rent>,
    system_program: Program<'info, System>,
}

impl<'info> EnterLottery<'info> {
    pub fn transfer_tokens_to_source_liquidity_vault(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let transfer_acct = Transfer {
            to: self.source_liquidity_vault.to_account_info().clone(),
            from: self.user_token_account.to_account_info().clone(),
            authority: self.user.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), transfer_acct)
    }
}

#[derive(Accounts)]
#[instruction(vault_signer_bump: u8)]
pub struct DepositTokensToLendingPool<'info> {
    /// CHECK:
    lending_program: AccountInfo<'info>,
    source_liquidity_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint=source_liquidity_mint,
        associated_token::authority=authority,
    )]
    source_liquidity_vault: Box<Account<'info, TokenAccount>>,
    destination_collateral_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint=destination_collateral_mint,
        associated_token::authority=authority,
    )]
    destination_collateral_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is the vault signer Acct
    #[account(
            seeds = [VAULT_SIGNER_STR.as_bytes()],
            bump = vault_signer_bump,
        )]
    authority: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve_liquidity_supply: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve_collateral_mint: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve_liquidity_oracle: AccountInfo<'info>,
    //CHECK:
    // reserve_liquidity_oracle: AccountInfo<
    /// CHECK:
    lending_market: AccountInfo<'info>,
    clock: Sysvar<'info, Clock>,
    // token_program: Program<'info, Token>,
    // rent: Sysvar<'info, Rent>,
    // system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(vault_signer_bump: u8)]
pub struct WithdrawTokensFromLendingPool<'info> {
    /// CHECK:
    lending_program: AccountInfo<'info>,
    source_liquidity_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint=source_liquidity_mint,
        associated_token::authority=authority,
    )]
    source_liquidity_vault: Box<Account<'info, TokenAccount>>,
    destination_collateral_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint=destination_collateral_mint,
        associated_token::authority=authority,
    )]
    destination_collateral_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is the vault signer Acct
    #[account(
            seeds = [VAULT_SIGNER_STR.as_bytes()],
            bump = vault_signer_bump,
        )]
    authority: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve_liquidity_supply: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve_collateral_mint: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    reserve_liquidity_oracle: AccountInfo<'info>,
    //CHECK:
    // reserve_liquidity_oracle: AccountInfo<
    /// CHECK:
    lending_market: AccountInfo<'info>,
    clock: Sysvar<'info, Clock>,
    // token_program: Program<'info, Token>,
    // rent: Sysvar<'info, Rent>,
    // system_program: Program<'info, System>,
}

pub mod utils {
    pub const VAULT_SIGNER_STR: &str = "vault_signer";
    pub const LOTTERY_INFO_STR: &str = "lottery_info";
}
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeLotteryParams {
    pub entry_fee: u64,
    pub max_participants: u64,
}
#[derive(Accounts)]
pub struct InitializeLottery<'info> {
    source_liquidity_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint=source_liquidity_mint,
        associated_token::authority=vault_signer,
    )]
    source_liquidity_vault: Box<Account<'info, TokenAccount>>,
    destination_collateral_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint=destination_collateral_mint,
        associated_token::authority=vault_signer,
    )]
    destination_collateral_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is the vault signer Acct
    #[account(
            seeds = [VAULT_SIGNER_STR.as_bytes()],
            bump,
        )]
    vault_signer: AccountInfo<'info>,
    #[account(
        init,
        space = 8 + LotteryInfo::MAX_SIZE ,
        payer = user,
        seeds = [LOTTERY_INFO_STR.as_bytes(),],
        bump,
    )]
    lottery_acct: Box<Account<'info, LotteryInfo>>,
    #[account(
        init,
        seeds = [
            STATE_SEED, 
            vrf.key().as_ref(),
            authority.key().as_ref(),
        ],
        payer = user,
        space = 8 + mem::size_of::<VrfClient>(),
        bump,
    )]
    state: AccountLoader<'info, VrfClient>,
    #[account(
        constraint = 
            *vrf.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    vrf: AccountLoader<'info, VrfAccountData>,
        /// CHECK:
    authority: AccountInfo<'info>,
    #[account(mut)]
    user: Signer<'info>,
    token_program: Program<'info, Token>,
    associated_token_program: Program<'info, AssociatedToken>,
    rent: Sysvar<'info, Rent>,
    #[account(address = solana_program::system_program::ID)]
    system_program: Program<'info, System>,
}

impl<'info> InitializeLottery<'info> {
    pub fn validate(&self, _ctx: &Context<Self>, params: &InitializeLotteryParams) -> Result<()> {
        msg!("Validate init");
        if params.max_participants > MAX_RESULT {
            return Err(error!(ErrorCode::MaxResultExceedsMaximum));
        }

        Ok(())
    }

    pub fn actuate(ctx: &Context<Self>, params: &InitializeLotteryParams) -> Result<()> {
        msg!("Actuate init");

        msg!("Checking VRF Account");
        let vrf = ctx.accounts.vrf.load()?;
        // client state needs to be authority in order to sign request randomness instruction
        if vrf.authority != ctx.accounts.state.key() {
            return Err(error!(ErrorCode::InvalidAuthorityError));
        }
        drop(vrf);

        msg!("Setting VrfClient state");
        let mut state = ctx.accounts.state.load_init()?;
        *state = VrfClient::default();
        state.bump = ctx.bumps.get("state").unwrap().clone();
        state.authority =  ctx.accounts.authority.key.clone();
        state.vrf = ctx.accounts.vrf.key();
        
        msg!("Setting VrfClient max_result");
        if params.max_participants == 0 {
            state.max_result = MAX_RESULT;
        } else {
            state.max_result = params.max_participants;
        }

        Ok(())
    }

}
