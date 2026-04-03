use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{
    errors::ConcentratedLiquidityError,
    state::{PoolState, Position},
};

#[derive(Accounts)]
#[instruction(tick_lower: i32, tick_upper: i32)]
pub struct CreatePosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = token_a_mint,
        has_one = token_b_mint,
        has_one = token_a_vault,
        has_one = token_b_vault,
    )]
    pub pool_state: Account<'info, PoolState>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    // PDA seeds: [b"position", position_mint.key().as_ref()]
    #[account(
        init,
        payer = owner,
        space = 8 + Position::INIT_SPACE,
        seeds = [
            b"position",
            position_mint.key().as_ref(),
        ],
        bump
    )]
    pub position: Account<'info, Position>,

    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = pool_state,
        mint::freeze_authority = pool_state,
    )]
    pub position_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = owner,
        associated_token::mint = position_mint,
        associated_token::authority = owner,
    )]
    pub owner_position_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_a_mint,
        token::authority = owner,
    )]
    pub owner_token_a: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_b_mint,
        token::authority = owner,
    )]
    pub owner_token_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_a_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_b_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<CreatePosition>,
    tick_lower: i32,
    tick_upper: i32,
    amount_a: u64,
    amount_b: u64,
) -> Result<()> {
    require!(tick_lower < tick_upper, ConcentratedLiquidityError::InvalidTickRange);
    require!(amount_a > 0 && amount_b > 0, ConcentratedLiquidityError::ZeroLiquidityDeposit);

    let cpi_accounts_a = Transfer {
        from: ctx.accounts.owner_token_a.to_account_info(),
        to: ctx.accounts.token_a_vault.to_account_info(),
        authority: ctx.accounts.owner.to_account_info(),
    };
    let cpi_accounts_b = Transfer {
        from: ctx.accounts.owner_token_b.to_account_info(),
        to: ctx.accounts.token_b_vault.to_account_info(),
        authority: ctx.accounts.owner.to_account_info(),
    };

    transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_a),
        amount_a,
    )?;
    transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_b),
        amount_b,
    )?;

    let pool_state_key = ctx.accounts.pool_state.key();
    let token_a_mint_key = ctx.accounts.token_a_mint.key();
    let token_b_mint_key = ctx.accounts.token_b_mint.key();
    let signer_seeds: &[&[u8]] = &[
        b"pool",
        token_a_mint_key.as_ref(),
        token_b_mint_key.as_ref(),
        &[ctx.accounts.pool_state.bump],
    ];
    let mint_to_accounts = MintTo {
        mint: ctx.accounts.position_mint.to_account_info(),
        to: ctx.accounts.owner_position_token_account.to_account_info(),
        authority: ctx.accounts.pool_state.to_account_info(),
    };
    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_to_accounts,
            &[signer_seeds],
        ),
        1,
    )?;

    let liquidity_amount = u128::from(amount_a.min(amount_b));

    let position = &mut ctx.accounts.position;
    position.bump = ctx.bumps.position;
    position.position_mint = ctx.accounts.position_mint.key();
    position.owner = ctx.accounts.owner.key();
    position.pool = pool_state_key;
    position.tick_lower = tick_lower;
    position.tick_upper = tick_upper;
    position.liquidity_amount = liquidity_amount;
    position.fees_a_owed = 0;
    position.fees_b_owed = 0;

    let pool_state = &mut ctx.accounts.pool_state;
    pool_state.total_liquidity = pool_state
        .total_liquidity
        .checked_add(liquidity_amount)
        .unwrap();

    Ok(())
}
