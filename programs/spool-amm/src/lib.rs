use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Token},
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

//from the token program
#[derive(Accounts)]
pub struct ProvideLp<'info> {
    //tranfer the money from
    pub signer: Signer<'info>,

    //mints for the vaults
    pub usdc_mint: InterfaceAccount<'info, TokenAccount>,
    pub sol_mint: InterfaceAccount<'info, TokenAccount>,

    //user token account
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    pub user_sol_account: InterfaceAccount<'info, TokenAccount>,

    //vault accounts
    pub usdc_vault_account: InterfaceAccount<'info, TokenAccount>,
    pub sol_vault_account: InterfaceAccount<'info, TokenAccount>,

    //token_program
    pub token_program: Interface<'info, TokenInterface>,
}

//swap struct

//lp token mint
#[derive(Accounts)]
pub struct LpMint<'info> {
    //signer for the account
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(init, payer = signer, mint::decimals = 9, mint::authority = signer.key(), mint::freeze_authority = signer.key())]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

//lp token ata account
#[derive(Accounts)]
pub struct CreateLpAta<'info> {
    //signer
    #[account(mut)]
    pub signer: Signer<'info>,

    //mint account
    pub lptokenmint: InterfaceAccount<'info, Mint>,

    //account for the value
    #[account(init, payer = signer, token::mint = lptokenmint, token::authority =  signer, token::token_program = token_program, seeds = [b"lptokenata", signer.key().as_ref()], bump)]
    pub lp_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

//lp token creating feature
#[derive(Accounts)]
pub struct Mintlptokens<'info> {
    //signer
    #[account(mut)]
    pub signer: Signer<'info>,

    //mint for lp tokens
    #[account(mut)]
    pub lptokenmint: InterfaceAccount<'info, Mint>,

    //user ata account
    #[account(mut)]
    pub lpata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> Mintlptokens<'info> {
    pub fn mint_tokens(&self, amount: u64) -> Result<()> {
        //figure the issue
        let cpi_accounts = MintTo {
            mint: self.lptokenmint.to_account_info(),
            to: self.lpata.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        //the cpi program
        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::mint_to(cpi_context, amount)?;
        Ok(())
    }
}
