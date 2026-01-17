use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Token},
    token_interface::{self, Burn, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked},
};

declare_id!("EFuEiBtmr5tPy3iYnQVhMPRVW64R5E1GonrCit8hXa66");

#[program]
pub mod spool_amm {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        //populate the pool_state_account
        let pool = &mut ctx.accounts.pool_stateaccount;
        pool.bump = ctx.bumps.pool_stateaccount;
        pool.usdc_mint = ctx.accounts.usdc_mint.key();
        pool.wsol_mint = ctx.accounts.wsol_mint.key();
        pool.usdc_vault_address = ctx.accounts.usdc_vault.key();
        pool.wsol_vault_address = ctx.accounts.wsol_vault.key();
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    //funtion to swap tokens
    pub fn swap(ctx: Context<SwapTokens>, amount_toswap: u64) -> Result<()> {
        ctx.accounts.main_swap_function(amount_toswap)?;
        msg!("swap is working");
        Ok(())
    }

    //function to remove lp
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, burnamount: u64) -> Result<()> {
        //call the main function
        ctx.accounts.remove_lp_main(burnamount)?;
        msg!("liquidty removed");
        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct LpPoolAccountShape {
    pub usdc_mint: Pubkey,
    pub wsol_mint: Pubkey,
    pub usdc_vault_address: Pubkey,
    pub wsol_vault_address: Pubkey,
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

//struct for swap
#[derive(Accounts)]
pub struct SwapTokens<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    //mints for the tokens
    pub input_mint: InterfaceAccount<'info, Mint>,
    pub output_mint: InterfaceAccount<'info, Mint>,

    //user accounts
    #[account(mut, token::authority = signer)]
    pub user_input_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::authority = signer)]
    pub user_output_account: InterfaceAccount<'info, TokenAccount>,

    //vaults for the transaction
    #[account(mut, token::authority = pool_stateaccount)]
    pub input_vault_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::authority = pool_stateaccount)]
    pub output_vault_account: InterfaceAccount<'info, TokenAccount>,

    //pool state for the vault
    pub pool_stateaccount: Account<'info, LpPoolAccountShape>,

    //token program
    pub token_program: Interface<'info, TokenInterface>,
}

//error enum for the swaptokens
#[error_code]
pub enum SwapTokenErrors {
    #[msg("swap amount is more then available balance")]
    AmountError,

    #[msg("input vault is incorrect")]
    InputVaultError,

    #[msg("output vault is incorrect")]
    OutputVaultError,

    #[msg("swap error")]
    SwapError,
}

//impl  for swap
impl<'info> SwapTokens<'info> {
    pub fn main_swap_function(&self, amount_toswap: u64) -> Result<()> {
        //run the checks
        self.checks(amount_toswap)?;

        //deduct fee
        let input_amount = self.deductfee(amount_toswap);

        //calculate output amount
        let output_amount = self.output_amount_calculation(input_amount)?;

        //call the swap function
        self.swaptokens(input_amount, output_amount as u64)?;
        Ok(())
    }

    pub fn checks(&self, amount_toswap: u64) -> Result<()> {
        //check for the amount
        if self.user_input_account.amount < amount_toswap {
            //throw the error
            return err!(SwapTokenErrors::AmountError);
        }

        //check for the input and output vault
        if self.input_vault_account.key() == self.pool_stateaccount.usdc_vault_address {
            //then output vault should be the wsol vault
            if self.output_vault_account.key() != self.pool_stateaccount.wsol_vault_address {
                return err!(SwapTokenErrors::OutputVaultError);
            } else if self.input_vault_account.key() == self.pool_stateaccount.wsol_vault_address {
                //then output vault should be the usdc vault
                if self.output_vault_account.key() != self.pool_stateaccount.usdc_vault_address {
                    return err!(SwapTokenErrors::OutputVaultError);
                }
            } else {
                //throw the eror for input vault
                return err!(SwapTokenErrors::InputVaultError);
            }
        }

        Ok(())
    }

    //function to deduct fee
    pub fn deductfee(&self, amouunt_in: u64) -> u64 {
        //constants for the fee
        const FEE_NUMERATOR: u128 = 30;
        const FEE_DENOMINATOR: u128 = 1000;

        let amount_needed = amouunt_in as u128;

        //calculate the fee
        let fee = (amount_needed * FEE_NUMERATOR) / FEE_DENOMINATOR;

        //return input_amount - fee
        (amount_needed - fee) as u64
    }

    pub fn swaptokens(&self, input_amount: u64, output_amount: u64) -> Result<()> {
        self.transferinput(input_amount)?;
        self.transferoutput(output_amount)?;
        Ok(())
    }

    pub fn output_amount_calculation(&self, input_amount: u64) -> Result<u64> {
        let input_vaultamount = self.input_vault_account.amount as u128;
        let output_vaultamount = self.output_vault_account.amount as u128;

        //product before swap
        let product_before_swap = (input_vaultamount * output_vaultamount) as u128;

        //formula to calculate amount
        let outputamount =
            (input_vaultamount * output_vaultamount) / (input_vaultamount + input_amount as u128);

        //check if the product before and after is same
        let input_vault_afterswap = input_vaultamount + input_amount as u128;
        let output_vault_afterswap = output_vaultamount - outputamount;

        let product_after_swap = (input_vault_afterswap * output_vault_afterswap) as u128;

        if product_after_swap != product_before_swap {
            return err!(SwapTokenErrors::SwapError);
        }

        Ok(outputamount as u64)
    }

    fn transferinput(&self, amount_toswap: u64) -> Result<()> {
        let decimals = self.input_mint.decimals;
        //tranfer from user to input vault
        let cpi_accounts = TransferChecked {
            mint: self.input_mint.to_account_info(),
            from: self.user_input_account.to_account_info(),
            to: self.input_vault_account.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, amount_toswap, decimals)?;
        Ok(())
    }
    fn transferoutput(&self, amount_transfer: u64) -> Result<()> {
        let usdc_mint = self.pool_stateaccount.usdc_mint;
        let wsol_mint = self.pool_stateaccount.wsol_mint;
        let decimals = self.output_mint.decimals;
        //tranfer from user to input vault
        let cpi_accounts = TransferChecked {
            mint: self.output_mint.to_account_info(),
            from: self.output_vault_account.to_account_info(),
            to: self.user_output_account.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let seeds = [
            b"pool_state",
            usdc_mint.as_ref(),
            wsol_mint.as_ref(),
            &[self.pool_stateaccount.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        token_interface::transfer_checked(cpi_context, amount_transfer, decimals)?;
        Ok(())
    }
}

//for withdrawing liquidity;
#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    //signer
    pub signer: Signer<'info>,

    //mint of usdc and wsol
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub wsol_mint: InterfaceAccount<'info, Mint>,

    //user accounts
    #[account(mut,token::mint = pool_state_account.usdc_mint, token::authority = signer)]
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut,token::mint = pool_state_account.wsol_mint, token::authority = signer)]
    pub user_wsol_account: InterfaceAccount<'info, TokenAccount>,

    //vault accounts
    #[account(mut)]
    pub usdc_vault_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub wsol_vault_account: InterfaceAccount<'info, TokenAccount>,

    //token_program
    pub token_program: Interface<'info, TokenInterface>,

    //pool_state_account
    #[account(mut)]
    pub pool_state_account: Account<'info, LpPoolAccountShape>,

    //lp_token_mint
    pub lp_mint: InterfaceAccount<'info, Mint>,

    //user lp token ata
    #[account(mut)]
    pub user_lp_ata: InterfaceAccount<'info, TokenAccount>,
}

#[error_code]
pub enum RemoveLiquidityErrors {
    #[msg("pool is empty")]
    EmptyPool,
}

impl<'info> RemoveLiquidity<'info> {
    fn remove_lp_main(&self, burnamount: u64) -> Result<()> {
        //calcualte the amounts
        let (transferusdcamount, transfersolamount) = self.calculate_amount(burnamount)?;

        //call the burn function
        self.burn_lptokens(burnamount)?;

        //call the tranfer function
        self.token_transfer(transferusdcamount, transfersolamount)?;
        Ok(())
    }

    fn calculate_amount(&self, burnamount: u64) -> Result<(u64, u64)> {
        let total_supply = self.lp_mint.supply;
        let usdc_vault_amount = self.user_usdc_account.amount;
        let wsol_vault_amount = self.user_wsol_account.amount;

        //safety check for the token account
        if total_supply == 0 {
            return err!(RemoveLiquidityErrors::EmptyPool);
        }

        let usdc_return_amount = (burnamount as u128)
            .checked_mul(usdc_vault_amount as u128)
            .unwrap()
            .checked_div(total_supply as u128)
            .unwrap() as u64;

        let wsol_return_amount = (burnamount as u128)
            .checked_mul(wsol_vault_amount as u128)
            .unwrap()
            .checked_div(total_supply as u128)
            .unwrap() as u64;

        Ok((usdc_return_amount, wsol_return_amount))
    }

    fn token_transfer(&self, transferusdcamount: u64, transfersolamount: u64) -> Result<()> {
        self.tranfer_usdc(transferusdcamount)?;
        self.tranfer_wsol(transfersolamount)?;
        Ok(())
    }

    fn burn_lptokens(&self, burnamount: u64) -> Result<()> {
        let cpi_accounts = Burn {
            mint: self.lp_mint.to_account_info(),
            from: self.user_lp_ata.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_progam = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_progam, cpi_accounts);
        token_interface::burn(cpi_context, burnamount)?;
        Ok(())
    }
    fn tranfer_usdc(&self, tranferusdcamount: u64) -> Result<()> {
        let decimals = self.usdc_mint.decimals;
        //tranfer from user to input vault
        let cpi_accounts = TransferChecked {
            mint: self.usdc_mint.to_account_info(),
            to: self.user_usdc_account.to_account_info(),
            from: self.usdc_vault_account.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, tranferusdcamount, decimals)?;
        Ok(())
    }

    fn tranfer_wsol(&self, transfersolamount: u64) -> Result<()> {
        let decimals = self.wsol_mint.decimals;
        //tranfer from user to input vault
        let cpi_accounts = TransferChecked {
            mint: self.wsol_mint.to_account_info(),
            to: self.user_wsol_account.to_account_info(),
            from: self.wsol_vault_account.to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, transfersolamount, decimals)?;
        Ok(())
    }
}
