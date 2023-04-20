use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, TransferChecked};

declare_id!("CZqii7Xi2WqsWci4WhB7hT7Dgbr3FyHZy8yVJ4Z81FiS");

#[warn(unused_doc_comments)]
/** Program Architecture
 * 
 * There are three main parts of the program: Processor, Instructions, & Account
 * 
 * Processor: Main business logic located in processor
 * 
 * Instructions (Account Context): Instruction data packing/unpacking, and 
 * account constraints and access control locate in Instruction handling part.
 * 
 * Accounts: Declaration of accounts owned by program locates in account part.
 * 
 */

// Processor

#[program]
pub mod anchor_escrow {
    use super::*;

    const AUTHORITY_SEED: &[u8] = b"authority";

    pub fn initialize(
        ctx: Context<Initialize>,
        random_seed: u64,
        initializer_amount: u64,
        taker_amount: u64,
    ) -> Result<()> {
        ctx.accounts.escrow_state.initializer_key = *ctx.accounts.initializer.key;
        ctx.accounts.escrow_state.initializer_deposit_token_account = *ctx
            .accounts
            .initializer_deposit_token_account
            .to_account_info()
            .key;
        ctx.accounts.escrow_state.initializer_receive_token_account = *ctx
            .accounts
            .initializer_receive_token_account
            .to_account_info()
            .key;
        ctx.accounts.escrow_state.initializer_amount = initializer_amount;
        ctx.accounts.escrow_state.taker_amount = taker_amount; // for exchange, later
        ctx.accounts.escrow_state.random_seed = random_seed;

        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[AUTHORITY_SEED], ctx.program_id);
        ctx.accounts.escrow_state.vault_authority_bump = vault_authority_bump;

        token::transfer_checked(
            ctx.accounts.into_transfer_to_pda_context(),
            ctx.accounts.escrow_state.initializer_amount,
            ctx.accounts.mint.decimals,
        )?;

        Ok(())
    }

    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        let authority_seeds = &[
            &AUTHORITY_SEED[..],
            &[ctx.accounts.escrow_state.vault_authority_bump],
        ];

        token::transfer_checked(
            ctx.accounts
                .into_transfer_to_initializer_context()
                .with_signer(&[&authority_seeds[..]]),
            ctx.accounts.escrow_state.initializer_amount,
            ctx.accounts.mint.decimals,
        )?;

        token::close_account(
            ctx.accounts
                .into_close_context()
                .with_signer(&[&authority_seeds[..]]),
        )?;

        Ok(())
    }

    pub fn exchange(ctx: Context<Exchange>) -> Result<()> {
        let authority_seeds = &[
            &AUTHORITY_SEED[..],
            &[ctx.accounts.escrow_state.vault_authority_bump],
        ];

        token::transfer_checked(
            ctx.accounts.into_transfer_to_initializer_context(),
            ctx.accounts.escrow_state.taker_amount,
            ctx.accounts.taker_deposit_token_mint.decimals,
        )?;

        token::transfer_checked(
            ctx.accounts
                .into_transfer_to_taker_context()
                .with_signer(&[&authority_seeds[..]]),
            ctx.accounts.escrow_state.initializer_amount,
            ctx.accounts.initializer_deposit_token_mint.decimals,
        )?;

        token::close_account(
            ctx.accounts
                .into_close_context()
                .with_signer(&[&authority_seeds[..]]),
        )?;

        Ok(())
    }
}

// Instructions - should bring in the accounts needed for operations
//
    /// Starts by creating and populating an escrow account and transferring 
    /// ownership of the given temp token account to the PDA
    ///
    ///
    /// Accounts expected:
    ///
    /// 0. `[initializer]` The account of the person initializing the escrow
    /// 1. `[mint]` Token account for tokens deposited to escrow
    /// 2. `[vault_authority] 
    /// 
    /// ToDo - fill in the rest based on Vault architecture below
    /// describes the original paulx, non-anchor, pure transfer system
    /// 
    /// 
    /// 1. `[writable]` Temporary token account that should be created prior to this instruction and owned by the initializer
    /// 2. `[]` The initializer's token account for the token they will receive should the trade go through
    /// 3. `[writable]` The escrow account, it will hold all necessary info about the trade.
    /// 4. `[]` The rent sysvar
    /// 
    /// ToDo - this one holds true still?
    /// 5. `[token_program]` The token program itself, used by Processor
//
#[derive(Accounts)]
#[instruction(escrow_seed: u64, initializer_amount: u64)]
pub struct Initialize<'info> {
    /// CHECK: This is not dangerous because we control who signs
    #[account(mut)]
    pub initializer: Signer<'info>,
    pub mint: Account<'info, Mint>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        seeds = [b"authority".as_ref()],
        bump,
    )]
    pub vault_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint,
        associated_token::authority = vault_authority
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = initializer_deposit_token_account.amount >= initializer_amount
    )]
    pub initializer_deposit_token_account: Account<'info, TokenAccount>,
    pub initializer_receive_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        seeds = [b"state".as_ref(), &escrow_seed.to_le_bytes()],
        bump,
        payer = initializer,
        space = EscrowState::space()
    )]
    pub escrow_state: Box<Account<'info, EscrowState>>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub associated_token_program: Program<'info, AssociatedToken>,
}



// Accounts, aka Program Accounts, required for building the escrow and vault

#[account]
pub struct EscrowAccount {
    pub initializer_key: Pubkey,              // Authorize the actions properly
    pub initializer_deposit_token_account: Pubkey,  // Record the deposit account of initializer
    pub initializer_receive_token_account: Pubkey,  // Record the receiving account of initializer
    pub initializer_amount: u64,              // Record amt tokens I. transfers to R.
    pub taker_amount: u64,                    // Record amt tokens R receives from I.
}


#[derive(Accounts)]
pub struct Cancel<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub initializer: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        seeds = [b"authority".as_ref()],
        bump,
    )]
    pub vault_authority: AccountInfo<'info>,
    #[account(mut)]
    pub initializer_deposit_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = escrow_state.initializer_key == *initializer.key,
        constraint = escrow_state.initializer_deposit_token_account == *initializer_deposit_token_account.to_account_info().key,
        close = initializer
    )]
    pub escrow_state: Box<Account<'info, EscrowState>>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}


#[derive(Accounts)]
pub struct Exchange<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub taker: Signer<'info>,
    pub initializer_deposit_token_mint: Account<'info, Mint>,
    pub taker_deposit_token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub taker_deposit_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub taker_receive_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub initializer_deposit_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub initializer_receive_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub initializer: AccountInfo<'info>,
    #[account(
        mut,
        constraint = escrow_state.taker_amount <= taker_deposit_token_account.amount,
        constraint = escrow_state.initializer_deposit_token_account == *initializer_deposit_token_account.to_account_info().key,
        constraint = escrow_state.initializer_receive_token_account == *initializer_receive_token_account.to_account_info().key,
        constraint = escrow_state.initializer_key == *initializer.key,
        close = initializer
    )]
    pub escrow_state: Box<Account<'info, EscrowState>>,
    #[account(mut)]
    pub vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(
        seeds = [b"authority".as_ref()],
        bump,
    )]
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct EscrowState {
    pub random_seed: u64,
    pub initializer_key: Pubkey,
    pub initializer_deposit_token_account: Pubkey,
    pub initializer_receive_token_account: Pubkey,
    pub initializer_amount: u64,
    pub taker_amount: u64,
    pub vault_authority_bump: u8,
}

impl EscrowState {
    pub fn space() -> usize {
        8 + 121
    }
}

// Utils -- some functions used for wrapping and passing data.

impl<'info> Initialize<'info> {
    fn into_transfer_to_pda_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.initializer_deposit_token_account.to_account_info(),
            mint: self.mint.to_account_info(),
            to: self.vault.to_account_info(),
            authority: self.initializer.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

impl<'info> Cancel<'info> {
    fn into_transfer_to_initializer_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.vault.to_account_info(),
            mint: self.mint.to_account_info(),
            to: self.initializer_deposit_token_account.to_account_info(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    fn into_close_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.vault.to_account_info(),
            destination: self.initializer.to_account_info(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

impl<'info> Exchange<'info> {
    fn into_transfer_to_initializer_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.taker_deposit_token_account.to_account_info(),
            mint: self.taker_deposit_token_mint.to_account_info(),
            to: self.initializer_receive_token_account.to_account_info(),
            authority: self.taker.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    fn into_transfer_to_taker_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.vault.to_account_info(),
            mint: self.initializer_deposit_token_mint.to_account_info(),
            to: self.taker_receive_token_account.to_account_info(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    fn into_close_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.vault.to_account_info(),
            destination: self.initializer.clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}
