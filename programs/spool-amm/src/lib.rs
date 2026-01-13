use anchor_lang::prelude::*;
use anchor_spl::{
    token,
    token_interface::{self, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked},
};

declare_id!("EFuEiBtmr5tPy3iYnQVhMPRVW64R5E1GonrCit8hXa66");

#[program]
pub mod spool_amm {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    //create brain pool and the vault
    //create the mintfor the lp tokens
    //create ata creator for lp tokens
    //provide lp tokens
    //
}

#[account]
#[derive(InitSpace)]
pub struct LpPoolAccountShape {
    pub usdc_mint: Pubkey,
    pub wsol_mint: Pubkey,
    pub usdc_vault_address: Pubkey,
    pub sol_vault_address: Pubkey,
    pub lp_token_mint: Pubkey,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    //signer
    #[account(mut)]
    pub signer: Signer<'info>,

    //mint account for the tokens
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub wsol_mint: InterfaceAccount<'info, Mint>,

    //system program field
    pub system_program: Program<'info, System>,
    //account init
    #[account(init , payer = signer, space = 8+LpPoolAccountShape::INIT_SPACE, seeds = [b"pool_state", usdc_mint.key().as_ref(), wsol_mint.key().as_ref()], bump)]
    pub pool_stateaccount: Account<'info, LpPoolAccountShape>,

    //token program
    pub token_program: Interface<'info, TokenInterface>,
    //create usdc_vault
    #[account(init, payer = signer, token::mint= usdc_mint, token::authority = pool_stateaccount, token::token_program  = token_program, seeds = [b"usdc_vault",usdc_mint.key().as_ref()], bump)]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,

    //create sol_vault
    #[account(init, payer = signer, token::mint= wsol_mint, token::authority = pool_stateaccount, token::token_program  = token_program, seeds = [b"usdc_vault",wsol_mint.key().as_ref()], bump)]
    pub wsol_vault: InterfaceAccount<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct ProvideLp<'info> {
    //tranfer the money from
    pub signer: Signer<'info>,

    //user token account
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    pub user_sol_account: InterfaceAccount<'info, TokenAccount>,

    //vault accounts
    pub usdc_vault_account: InterfaceAccount<'info, TokenAccount>,
    pub sol_vault_account: InterfaceAccount<'info, TokenAccount>,

    //token_program
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Swap<'info> {}
