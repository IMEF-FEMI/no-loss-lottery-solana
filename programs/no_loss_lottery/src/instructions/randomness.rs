
use crate::{errors::ErrorCode, STATE_SEED, MAX_RESULT, VrfClient, RequestingRandomness, VrfClientResultUpdated, VrfClientInvoked,};

use anchor_lang::{prelude::*, solana_program};
use anchor_spl::token::TokenAccount;
use switchboard_v2::SWITCHBOARD_PROGRAM_ID;
pub use switchboard_v2::{VrfAccountData, VrfRequestRandomness, OracleQueueAccountData, PermissionAccountData, SbState};
use anchor_spl::token::Token;
use anchor_lang::solana_program::clock;
use std::mem;


#[access_control(ctx.accounts.validate(&ctx, &params))]
pub fn init_state(ctx: Context<InitState>, params: InitStateParams) -> Result<()> {
    InitState::actuate(&ctx, &params)
}

#[access_control(ctx.accounts.validate(&ctx))]
pub fn update_result(mut ctx: Context<UpdateResult>) -> Result<()> {
    UpdateResult::actuate( &mut ctx)
}

#[access_control(ctx.accounts.validate(&ctx, &params))]
pub fn request_result(ctx: Context<RequestResult>, params: RequestResultParams) -> Result<()> {
    RequestResult::actuate(&ctx, &params)
}


#[derive(Accounts)]
#[instruction(params: InitStateParams)]
pub struct InitState<'info> {
    #[account(
        init,
        seeds = [
            STATE_SEED, 
            vrf.key().as_ref(),
            authority.key().as_ref(),
        ],
        payer = payer,
        space = 8 + mem::size_of::<VrfClient>(),
        bump,
    )]
    pub state: AccountLoader<'info, VrfClient>,
    /// CHECK:
    pub authority: AccountInfo<'info>,
    /// CHECK:
    #[account(mut, signer)]
    /// CHECK:
    pub payer: AccountInfo<'info>,
    #[account(
        constraint = 
            *vrf.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub vrf: AccountLoader<'info, VrfAccountData>,
    #[account(address = solana_program::system_program::ID)]
    pub system_program: Program<'info, System>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct InitStateParams {
    pub max_result: u64,
}

impl InitState<'_> {
    pub fn validate(&self, _ctx: &Context<Self>, params: &InitStateParams) -> Result<()> {
        msg!("Validate init");
        if params.max_result > MAX_RESULT {
            return Err(error!(ErrorCode::MaxResultExceedsMaximum));
        }

        Ok(())
    }

    pub fn actuate(ctx: &Context<Self>, params: &InitStateParams) -> Result<()> {
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
        if params.max_result == 0 {
            state.max_result = MAX_RESULT;
        } else {
            state.max_result = params.max_result;
        }

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(params: RequestResultParams)] // rpc parameters hint
pub struct RequestResult<'info> {
    #[account(
        mut,
        seeds = [
            STATE_SEED, 
            vrf.key().as_ref(),
            authority.key().as_ref(),
        ],
        bump = state.load()?.bump,
        has_one = vrf @ ErrorCode::InvalidVrfAccount,
        has_one = authority @ ErrorCode::InvalidAuthorityError
    )]
    pub state: AccountLoader<'info, VrfClient>,
    /// CHECK:
    #[account(signer)] // client authority needs to sign
    pub authority: AccountInfo<'info>,

    // SWITCHBOARD ACCOUNTS
    #[account(mut,
        has_one = escrow,
        constraint = 
            *vrf.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub vrf: AccountLoader<'info, VrfAccountData>,
    /// CHECK
    #[account(mut, 
        has_one = data_buffer,
        constraint = 
            oracle_queue.load()?.authority == queue_authority.key()
            && *oracle_queue.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub oracle_queue: AccountLoader<'info, OracleQueueAccountData>,
    /// CHECK: Will be checked in the CPI instruction
    pub queue_authority: UncheckedAccount<'info>,
    /// CHECK
    #[account(mut, 
        constraint = 
            *data_buffer.owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub data_buffer: AccountInfo<'info>,
    /// CHECK
    #[account(mut, 
        constraint = 
            *permission.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub permission: AccountLoader<'info, PermissionAccountData>,
    #[account(mut, 
        constraint = 
            escrow.owner == program_state.key() 
            && escrow.mint == program_state.load()?.token_mint
    )]
    pub escrow: Account<'info, TokenAccount>,
    /// CHECK: Will be checked in the CPI instruction
    #[account(mut, 
        constraint = 
            *program_state.to_account_info().owner == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub program_state: AccountLoader<'info, SbState>,
    /// CHECK: 
    #[account(
        constraint = 
            switchboard_program.executable == true 
            && *switchboard_program.key == SWITCHBOARD_PROGRAM_ID @ ErrorCode::InvalidSwitchboardAccount
    )]
    pub switchboard_program: AccountInfo<'info>,

    // PAYER ACCOUNTS
    #[account(mut, 
        constraint = 
            payer_wallet.owner == payer_authority.key()
            && payer_wallet.mint == program_state.load()?.token_mint
    )]
    pub payer_wallet: Account<'info, TokenAccount>,
    /// CHECK:
    #[account(signer)]
    pub payer_authority: AccountInfo<'info>,

    // SYSTEM ACCOUNTS
    /// CHECK:
    #[account(address = solana_program::sysvar::recent_blockhashes::ID)]
    pub recent_blockhashes: AccountInfo<'info>,
    #[account(address = anchor_spl::token::ID)]
    pub token_program: Program<'info, Token>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct RequestResultParams {
    pub permission_bump: u8,
    pub switchboard_state_bump: u8,
}

impl RequestResult<'_> {
    pub fn validate(&self, _ctx: &Context<Self>, _params: &RequestResultParams) -> Result<()> {
        Ok(())
    }

    pub fn actuate(ctx: &Context<Self>, params: &RequestResultParams) -> Result<()> {
        let client_state = ctx.accounts.state.load()?;
        let bump = client_state.bump.clone();
        let max_result = client_state.max_result;
        drop(client_state);

        let switchboard_program = ctx.accounts.switchboard_program.to_account_info();

        let vrf_request_randomness = VrfRequestRandomness {
            authority: ctx.accounts.state.to_account_info(),
            vrf: ctx.accounts.vrf.to_account_info(),
            oracle_queue: ctx.accounts.oracle_queue.to_account_info(),
            queue_authority: ctx.accounts.queue_authority.to_account_info(),
            data_buffer: ctx.accounts.data_buffer.to_account_info(),
            permission: ctx.accounts.permission.to_account_info(),
            escrow: ctx.accounts.escrow.clone(),
            payer_wallet: ctx.accounts.payer_wallet.clone(),
            payer_authority: ctx.accounts.payer_authority.to_account_info(),
            recent_blockhashes: ctx.accounts.recent_blockhashes.to_account_info(),
            program_state: ctx.accounts.program_state.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        let vrf_key = ctx.accounts.vrf.key();
        let authority_key = ctx.accounts.authority.key.clone();

        msg!("bump: {}", bump);
        msg!("authority: {}", authority_key);
        msg!("vrf: {}", vrf_key);

        let state_seeds: &[&[&[u8]]] = &[&[
            &STATE_SEED,
            vrf_key.as_ref(),
            authority_key.as_ref(),
            &[bump],
        ]];
        msg!("requesting randomness");
        vrf_request_randomness.invoke_signed(
            switchboard_program,
            params.switchboard_state_bump,
            params.permission_bump,
            state_seeds,
        )?;

        let mut client_state = ctx.accounts.state.load_mut()?;
        client_state.result = 0;

        emit!(RequestingRandomness{
            vrf_client: ctx.accounts.state.key(),
            max_result: max_result,
            timestamp: clock::Clock::get().unwrap().unix_timestamp
        });

        msg!("randomness requested successfully");
        Ok(())
    }
}


#[derive(Accounts)]
pub struct UpdateResult<'info> {
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

impl UpdateResult<'_> {
    pub fn validate(&self, _ctx: &Context<Self>) -> Result<()> {
        // We should check VRF account passed is equal to the pubkey stored in our client state
        // But skipping so we can re-use this program instruction for CI testing
        Ok(())
    }

    pub fn actuate(ctx: &mut Context<Self>) -> Result<()> {
        let clock = clock::Clock::get().unwrap();

        emit!(VrfClientInvoked {
            vrf_client: ctx.accounts.state.key(),
            timestamp: clock.unix_timestamp,
        });

        let vrf = ctx.accounts.vrf.load()?;
        let result_buffer = vrf.get_result()?;
        if result_buffer == [0u8; 32] {
            msg!("vrf buffer empty");
            return Ok(());
        }

        let state = &mut ctx.accounts.state.load_mut()?;
        let max_result = state.max_result;
        if result_buffer == state.result_buffer {
            msg!("existing result_buffer");
            return Ok(());
        }

        msg!("Result buffer is {:?}", result_buffer);
        let value: &[u128] = bytemuck::cast_slice(&result_buffer[..]);
        msg!("u128 buffer {:?}", value);
        // let result = value[0] % max_result as u128 + 1;
        let result = value[0] % max_result as u128;
        msg!("Current VRF Value [1 - {}) = {}!", max_result, result);
        
      

        if state.result != result {
            state.result_buffer = result_buffer;
            state.result = result;
            state.last_timestamp = clock.unix_timestamp;

            emit!(VrfClientResultUpdated {
                vrf_client: ctx.accounts.state.key(),
                result: state.result,
                result_buffer: result_buffer,
                timestamp: clock.unix_timestamp,
            });
        }

        Ok(())
    }
}
