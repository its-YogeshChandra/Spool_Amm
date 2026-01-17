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
    //adding lp mint logic
    //signer for the account

    //the authority of this mint should be the contract
    #[account(init, payer = signer, mint::decimals = 9, mint::authority = pool_stateaccount, mint::freeze_authority = signer.key())]
    pub mint: InterfaceAccount<'info, Mint>,
}

#[error_code]
pub enum ProvideLpErrors {
    #[msg("multiplication error")]
    MultiplicationError,

    #[msg("liquidity too low")]
    LiquidityTooLow,
}

//from the token program
#[derive(Accounts)]
pub struct ProvideLp<'info> {
    //tranfer the money from
    #[account(mut)]
    pub signer: Signer<'info>,

    //mints for the vaults
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub wsol_mint: InterfaceAccount<'info, Mint>,

    //user token account
    #[account(mut,token::mint = usdc_mint, token::authority= signer )]
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint= wsol_mint, token::authority = signer)]
    pub user_wsol_account: InterfaceAccount<'info, TokenAccount>,

    //vault accounts
    pub usdc_vault_account: InterfaceAccount<'info, TokenAccount>,
    pub wsol_vault_account: InterfaceAccount<'info, TokenAccount>,

    //token_program
    pub token_program: Interface<'info, TokenInterface>,

    // ---------minting lp token logic --------
    //token account creation----
    //mint of the lp
    pub lptokenmint: InterfaceAccount<'info, Mint>,

    //account creation
    #[account(init, payer = signer, token::mint = lptokenmint, token::authority =  signer, token::token_program = token_program, seeds = [b"lptokenata", signer.key().as_ref()], bump)]
    pub lp_ata: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,

    //for minting lp tokens -------
    //pool state acount for getting seeds
    //mining authority
    pub mint_authority: Account<'info, LpPoolAccountShape>,

    //user ata account
    #[account(mut,token::authority= signer, token::mint = lptokenmint)]
    pub lpata: InterfaceAccount<'info, TokenAccount>,
}

impl<'info> ProvideLp<'info> {
    //providing lp mainly has signing function
    fn token_transfer(&self, wsol_amount: u64, usdc_amount: u64) -> Result<()> {
        //calculate the lp token amount need to provide
        let lp_amount = self.lptoken_amount(usdc_amount, wsol_amount).unwrap();

        //tranfer function for usdc and sol
        self.tranfer_usdc(usdc_amount)?;
        self.tranfer_wsol(wsol_amount)?;

        //min lp token function
        self.mint_lptokens(lp_amount as u64)?;
        Ok(())
    }

    fn tranfer_usdc(&self, amount: u64) -> Result<()> {
        let decimals = self.usdc_mint.decimals;
        let cpi_accounts = TransferChecked {
            mint: self.usdc_mint.to_account_info(),
            from: self.user_usdc_account.to_account_info(),
            to: self.usdc_vault_account.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);

        //tranfer token
        token_interface::transfer_checked(cpi_context, amount, decimals)?;
        Ok(())
    }

    fn tranfer_wsol(&self, amount: u64) -> Result<()> {
        let decimals = self.wsol_mint.decimals;
        let cpi_accounts = TransferChecked {
            mint: self.wsol_mint.to_account_info(),
            from: self.user_wsol_account.to_account_info(),
            to: self.wsol_vault_account.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);

        //tranfer token
        token_interface::transfer_checked(cpi_context, amount, decimals)?;
        Ok(())
    }

    fn lptoken_amount(&self, usdc_amount: u64, wsol_amount: u64) -> Result<u64> {
        //burn constant to prevent inflation attack
        const MINIMUM_LIQUIDITY: u64 = 1000;

        //lp share on the basis of total lp supply
        let total_supply = self.lptokenmint.supply;

        //the lp amount
        let lp_amount: u64;

        if total_supply == 0 {
            let (lp, _) = self
                .first_time_amount(usdc_amount, wsol_amount, MINIMUM_LIQUIDITY)
                .unwrap();
            lp_amount = lp;
        } else {
            let (lp, _) = self.normal_amount(usdc_amount, wsol_amount).unwrap();
            lp_amount = lp
        }

        Ok(lp_amount)
    }

    fn first_time_amount(
        &self,
        usdc_amount: u64,
        wsol_amount: u64,
        minimum_liquidity: u64,
    ) -> Result<(u64, u64)> {
        let product = (usdc_amount as u128)
            .checked_mul(wsol_amount as u128)
            .ok_or(ProvideLpErrors::MultiplicationError)?;

        let liquidity = (product as f64).sqrt() as u64;

        //deposit should satisfy the amount
        if liquidity <= minimum_liquidity {
            return err!(ProvideLpErrors::LiquidityTooLow);
        }

        return Ok((
            liquidity.checked_sub(minimum_liquidity).unwrap(),
            minimum_liquidity,
        ));
    }

    fn normal_amount(&self, usdc_amount: u64, wsol_amount: u64) -> Result<(u64, u64)> {
        let total_supply = self.lptokenmint.supply;

        //share on the basis of usdc
        let share_usdc = (usdc_amount as u128)
            .checked_mul(total_supply as u128)
            .ok_or(ProvideLpErrors::MultiplicationError)?
            .checked_div(self.usdc_vault_account.amount as u128)
            .ok_or(ProvideLpErrors::MultiplicationError)?;

        //share on the basis of wsol
        let share_wsol = (wsol_amount as u128)
            .checked_mul(total_supply as u128)
            .ok_or(ProvideLpErrors::MultiplicationError)?
            .checked_div(self.wsol_vault_account.amount as u128)
            .ok_or(ProvideLpErrors::MultiplicationError)?;

        //take the smaller share from both the values
        let liquidity = std::cmp::min(share_usdc, share_wsol) as u64;

        //return
        return Ok((liquidity, 0));
    }

    fn mint_lptokens(&self, amount: u64) -> Result<()> {
        let cpi_accounts = MintTo {
            mint: self.lptokenmint.to_account_info(),
            to: self.lp_ata.to_account_info(),
            authority: self.mint_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        let usdc_mint = self.usdc_mint.key();
        let wsol_mint = self.wsol_mint.key();
        let bump = self.mint_authority.bump;
        let seeds = [
            b"pool_state",
            usdc_mint.as_ref(),
            wsol_mint.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        token_interface::mint_to(cpi_context, amount)?;
        Ok(())
    }
}

//
// //lp token mint
// #[derive(Accounts)]
// pub struct LpMint<'info> {
//     //signer for the account
//     #[account(mut)]
//     pub signer: Signer<'info>,
//
//     //the authority of this mint should be the contract
//     #[account(init, payer = signer, mint::decimals = 9, mint::authority = signer.key(), mint::freeze_authority = signer.key())]
//     pub mint: InterfaceAccount<'info, Mint>,
//     pub token_program: Interface<'info, TokenInterface>,
//     pub system_program: Program<'info, System>,
// }
//
// //lp token ata account
// #[derive(Accounts)]
// pub struct CreateLpAta<'info> {
//     //signer
//     #[account(mut)]
//     pub signer: Signer<'info>,
//
//     //mint account
//     pub lptokenmint: InterfaceAccount<'info, Mint>,
//
//     //account for the value
//     #[account(init, payer = signer, token::mint = lptokenmint, token::authority =  signer, token::token_program = token_program, seeds = [b"lptokenata", signer.key().as_ref()], bump)]
//     pub lp_ata: InterfaceAccount<'info, TokenAccount>,
//     pub token_program: Interface<'info, TokenInterface>,
//     pub system_program: Program<'info, System>,
// }
//
// //lp token creating feature
// #[derive(Accounts)]
// pub struct Mintlptokens<'info> {
//     //signer
//     #[account(mut)]
//     pub signer: Signer<'info>,
//
//     //mint for lp tokens
//     #[account(mut)]
//     pub lptokenmint: InterfaceAccount<'info, Mint>,
//
//     //user ata account
//     #[account(mut,token::authority= signer, token::mint = lptokenmint)]
//     pub lpata: InterfaceAccount<'info, TokenAccount>,
//
//     pub token_program: Interface<'info, TokenInterface>,
// }
//
// impl<'info> Mintlptokens<'info> {
//     pub fn mint_tokens(&self, amount: u64) -> Result<()> {
//         let cpi_accounts = MintTo {
//             mint: self.lptokenmint.to_account_info(),
//             to: self.lpata.to_account_info(),
//             authority: self.signer.to_account_info(),
//         };
//
//         //the cpi program
//         let cpi_program = self.token_program.to_account_info();
//         let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
//         token_interface::mint_to(cpi_context, amount)?;
//         Ok(())
//     }
// }

//struct for swap
#[derive(Accounts)]
pub struct SwapTokens<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    //user accounts
    #[account(mut, token::authority = signer)]
    pub user_input_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::authority = signer)]
    pub user_output_account: InterfaceAccount<'info, TokenAccount>,

    //vaults for the transaction
    #[account(mut, token::authority = pool_stateaccount)]
    pub input_vault_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::authority = pool_stateaccount)]
    pub output_vault_accont: InterfaceAccount<'info, TokenAccount>,

    //pool state for the vault
    pub pool_stateaccount: Account<'info, LpPoolAccountShape>,
}

//impl  for swap
impl<'info> SwapTokens<'info> {
    pub fn checks() {}

    pub fn deductfee() {}

    pub fn swaptokens() {}

    pub fn distributefee() {}

    fn tranferinput() {}
    fn transferoutput() {}
}
