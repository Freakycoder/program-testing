use anchor_lang::prelude::*;

declare_id!("6KNjJqfUVrDZRnosDUDrWeHVAeMcKzvuPjoNC8Gc4JJq");

#[program]
pub mod escrow_program {
    use super::*;
    use anchor_lang::{solana_program::{ed25519_program, sysvar::instructions::load_instruction_at_checked}, system_program};
    pub fn initialize_fee_account(ctx: Context<InitializeFeeAccount>) -> Result<()>{
        let fee_account = &mut ctx.accounts.fee_account; // here we take mutable reference so that the instruction doesn't consume fee account PDA
        let admin_key = ctx.accounts.admin.key();
        if fee_account.admin == Pubkey::default(){
            msg!("Fee Account does not exist...creating new!");
            fee_account.admin = admin_key;
            fee_account.total_fee_amount = 0;
        }
        else {
            msg!("Fee Account already exist.");
            require!(fee_account.admin == admin_key, FeeAccountError::AdminError);
        }
        Ok(())
    }

    pub fn initialize_vault(ctx: Context<InitializeVault>, operator : Pubkey, fee_account : Pubkey) -> Result<()> {

        let vault = &mut ctx.accounts.vault;
        if vault.admin == Pubkey::default(){
            msg!("Vault doesn't exist...created new!");
            vault.admin = ctx.accounts.admin.key();
            vault.operator = operator;
            vault.is_paused = false;
            vault.fee_account = fee_account;
            vault.last_admin_withdrawal = None;
            vault.withdrawal_fee_bps = 200;
            vault.total_deposited = 0;
            vault.total_withdrawn = 0;
            vault.bump = ctx.bumps.vault;

            emit!(VaultInitialized{
                admin : vault.admin,
                operator : vault.operator
            })
        }
        else {
            msg!("Vault exist, confirming Authority.");
            require!(ctx.accounts.vault.admin == ctx.accounts.admin.key(), VaultError::AdminError);
            msg!("Authority confirmation Succesfull");
        }
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount : u64) -> Result<()>{
        let vault = &mut ctx.accounts.vault;
        require!(!vault.is_paused , VaultError::VaultPaused);
        require!(amount > 0 , VaultError::InvalidAmount);

        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer{
                from : ctx.accounts.user.to_account_info(),
                to : vault.to_account_info()
            });
            system_program::transfer(cpi_context, amount)?;

        vault.total_deposited = vault.total_deposited.checked_add(amount).ok_or(VaultError::MathOverflow)?;
        Ok(())
    }

    pub fn withdrawal(ctx : Context<Withdraw>, amount : u64, _operator_key : String, signed_message : String) -> Result<()>{
        let vault = &mut ctx.accounts.vault;
        let vault_admin = vault.admin;
        require!(!vault.is_paused, VaultError::VaultPaused);
        require!(amount > 0, VaultError::InvalidAmount);
        require!(vault.total_deposited > amount, VaultError::InsufficientBalance);
        let operator_pubkey = vault.operator;
        let ix = load_instruction_at_checked(0, &ctx.accounts.instruction_sysvar)?;
        require!(ix.program_id == ed25519_program::ID, VaultError::ProgramMissing);
        let is_message = ix.data.windows(signed_message.len()).any(|message| message == signed_message.as_bytes());
        require!(is_message, VaultError::SignedMessageMissing);
        let is_operator_key = ix.data.windows(operator_pubkey.to_bytes().len()).any(|pubkey| pubkey == operator_pubkey.to_bytes());
        if is_operator_key == true {
            let fee_amount = (amount* 2)/100;
            let withdrawal_amount = amount - fee_amount;
            let seeds = &[b"vault".as_ref(), vault_admin.as_ref(), &[vault.bump]];
            let signer = &[&seeds[..]];
            let cpi = CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer{
                    from : vault.to_account_info(),
                    to : ctx.accounts.user.to_account_info()
                },
                signer
            );
            match system_program::transfer(cpi, withdrawal_amount){
                Ok(_) => {
                    msg!("succesfull transfer from vault to user wallet");
                    vault.total_deposited = vault.total_deposited.checked_sub(amount).ok_or(VaultError::MathOverflow)?;
                    vault.total_withdrawn = vault.total_withdrawn.checked_add(amount).ok_or(VaultError::MathOverflow)?;
                    vault.withdrawal_counter += 1;
                    let cpi = CpiContext::new_with_signer(
                        ctx.accounts.system_program.to_account_info(),
                        system_program::Transfer{
                            from : vault.to_account_info(),
                            to : ctx.accounts.fee_account.to_account_info(),
                        },
                        signer
                    );
                    system_program::transfer(cpi, fee_amount)?;

                    emit!(WithdrawalSucces{
                        user : ctx.accounts.user.key(),
                        amount,
                        fee : fee_amount,
                        timestamp : Clock::get()?.unix_timestamp
                    })
                }
                Err(e) => {
                    msg!("Error transferring amount : {} from vault to user : {}", e, ctx.accounts.user.key());
                }
            }
        }
        Ok(())
    }

    pub fn admin_withdrawal(ctx : Context<AdminWithdraw>) -> Result<()>{
        let vault = &mut ctx.accounts.vault;
        let vault_admin = vault.admin;
        let rent_exempt = Rent::get()?.minimum_balance(Vault::INIT_SPACE);
        require!(vault.total_deposited > rent_exempt, VaultError::AdminWithdrawal);
        let withdrawal_amount = vault.get_lamports().checked_sub(rent_exempt).ok_or(VaultError::MathOverflow)?;
        let seeds = &[b"vault".as_ref(), vault_admin.as_ref(), &[vault.bump]];
        let signer = &[&seeds[..]];
        let cpi = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer{
                from : vault.to_account_info(),
                to : ctx.accounts.admin_address.to_account_info()
            },
        signer);
        match system_program::transfer(cpi, withdrawal_amount){
            Ok(_) => {
                msg!("Admin withdrawal Succesfull");
                vault.total_deposited = vault.total_deposited.checked_sub(withdrawal_amount).ok_or(VaultError::MathOverflow)?;
                vault.total_withdrawn = vault.total_withdrawn.checked_add(withdrawal_amount).ok_or(VaultError::MathOverflow)?;
                vault.withdrawal_counter += 1;
            }
            Err(e) =>{
                msg!("Unable to process admin withdrawal due to : {}",e);
            }
        };
        Ok(())
    }

    pub fn set_vault_pause(ctx : Context<SetPause>, paused : bool) -> Result<()>{
        let vault = &mut ctx.accounts.vault;
        vault.is_paused = paused;
        emit!(PauseStatus{
            paused
        });
        Ok(())
    }

}

#[derive(Accounts, Debug)]
pub struct InitializeFeeAccount<'info> {
    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + FeeAccount::INIT_SPACE,
        seeds = [b"fee_account".as_ref() , admin.key().as_ref()],
        bump
    )]
    pub fee_account : Account<'info, FeeAccount>,
    #[account(mut)]
    pub admin : Signer<'info>,
    pub system_program : SystemAccount<'info>,
}

#[account]
#[derive(InitSpace, Debug)]
pub struct FeeAccount{
    pub admin : Pubkey,
    pub total_fee_amount : u64
}

#[derive(Accounts, Debug)]
pub struct InitializeVault<'info> {
    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault".as_ref() , admin.key().as_ref()],
        bump
    )]
    pub vault : Account<'info, Vault>,
    #[account(mut)]
    pub admin : Signer<'info>,
    pub system_program : SystemAccount<'info>,
}

#[account]
#[derive(InitSpace ,Debug)]
pub struct Vault{
    pub admin : Pubkey,
    pub operator : Pubkey,
    pub fee_account : Pubkey,
    pub total_deposited : u64,
    pub total_withdrawn : u64,
    pub withdrawal_fee_bps : u16,
    pub last_admin_withdrawal : Option<i64>,
    pub is_paused : bool,
    pub withdrawal_counter : u64,
    pub bump : u8
}

#[derive(Accounts, Debug)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub vault : Account<'info, Vault>,
    #[account(mut)]
    pub user : Signer<'info>,
    pub system_program : SystemAccount<'info>,
}

#[derive(Accounts, Debug)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub vault : Account<'info, Vault>,
    #[account(mut)]
    pub user : Signer<'info>,
    pub system_program : SystemAccount<'info>,
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar : AccountInfo<'info>,
    pub fee_account : Account<'info, FeeAccount>
}

#[derive(Accounts,Debug)]
pub struct AdminWithdraw<'info>{
     #[account(mut)]
    pub vault : Account<'info, Vault>,
    #[account(mut)]
    pub admin : Signer<'info>,
    pub admin_address : AccountInfo<'info>,
    pub system_program : SystemAccount<'info>,
}

#[derive(Accounts,Debug)]
pub struct SetPause<'info>{
    #[account(mut)]
    pub vault : Account<'info, Vault>,
    pub admin : Signer<'info>,
}

#[error_code]
pub enum VaultError {
    #[msg("the vault admin doesn't match the admin key")]
    AdminError,
    #[msg("the vault is on pause")]
    VaultPaused,
    #[msg("requested amount must be not NILL")]
    InvalidAmount,
    #[msg("overflow error in adding deposited amount to vault")]
    MathOverflow,
    #[msg("could not find the program in the instruction returned")]
    ProgramMissing,
    #[msg("could not find the signed message in ed25519 instruction")]
    SignedMessageMissing,
    #[msg("deposited amount in vault less than withdraw amount")]
    InsufficientBalance,
    #[msg("unable to withdraw deposited amount to admin address")]
    AdminWithdrawal
}

#[error_code]
pub enum FeeAccountError {
    #[msg("the fee account admin does not match the specified pubkey")]
    AdminError
}

#[event]
pub struct VaultInitialized{
    pub admin : Pubkey,
    pub operator : Pubkey
}
#[event]
pub struct WithdrawalSucces{
    pub user : Pubkey,
    pub amount  : u64,
    pub fee : u64,
    pub timestamp : i64
}

#[event]
pub struct PauseStatus{
    pub paused : bool
}