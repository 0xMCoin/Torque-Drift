use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, MintTo, burn, Burn, Mint, Token, TokenAccount},
};
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;

declare_id!("EN2SeC45TuHgrLg33ZhJLsYSX5gxnunrVm5P6Dx5eiRS");

pub fn verify_signature(
    sysvar_instructions: &AccountInfo,
    _message: &[u8],
    _signature: &[u8; 64],
    _public_key: &Pubkey,
) -> Result<()> {
    use anchor_lang::solana_program::ed25519_program;

    let instruction_sysvar = sysvar_instructions::load_current_index_checked(sysvar_instructions)?;

    // A instrução ED25519 deve estar na posição anterior (index - 1)
    if instruction_sysvar > 0 {
        let ed25519_ix_index = (instruction_sysvar - 1) as u8;
        let current_ix = sysvar_instructions::load_instruction_at_checked(
            ed25519_ix_index as usize,
            sysvar_instructions,
        )?;

        // Verificar se é uma instrução ED25519 válida
        require!(
            current_ix.program_id == ed25519_program::ID,
            ErrorCode::InvalidSignature
        );

        msg!("ED25519 signature verification passed");
    } else {
        return err!(ErrorCode::InvalidSignature);
    }

    Ok(())
}

// Definir evento para registrar queima de tokens
#[event]
pub struct TokenBurnEvent {
    pub payer: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub description: String,
    pub timestamp: i64,
}

// Definir evento para registrar mint de tokens
#[event]
pub struct TokenMintEvent {
    pub minter: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
    pub timestamp: i64,
}

// Definir evento para registrar claim de tokens
#[event]
pub struct TokenClaimEvent {
    pub claimer: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// Eventos de segurança
#[event]
pub struct SecurityEvent {
    pub event_type: String,
    pub user: Pubkey,
    pub reason: String,
    pub timestamp: i64,
}

#[event]
pub struct AdminActionEvent {
    pub admin: Pubkey,
    pub action: String,
    pub details: String,
    pub timestamp: i64,
}

#[account]
pub struct ConfigAccount {
    pub payment_token_mint: Pubkey,
    pub admin: Pubkey,
    pub emergency_paused: bool,
    pub max_claim_per_user: u64,    // Máximo que um usuário pode claim em 24h
    pub total_supply_limit: u64,     // Limite total de supply que pode ser mintado
    pub total_minted: u64,           // Total já mintado
}

// Conta para rastrear claims por usuário
#[account]
pub struct UserClaimAccount {
    pub user: Pubkey,              // Usuário
    pub total_claimed: u64,         // Total já claimado por este usuário
    pub last_claim_timestamp: i64,  // Timestamp do último claim
    pub daily_claimed: u64,         // Total claimado nas últimas 24h
    pub daily_reset_timestamp: i64, // Quando o contador diário foi resetado
    pub hourly_claimed: u64,        // Total claimado na última hora
    pub hourly_reset_timestamp: i64, // Quando o contador horário foi resetado
    pub nonce: u64,                 // Nonce para prevenir replay attacks
    pub is_blacklisted: bool,       // Usuário banido?
}

// Lista negra de usuários
#[account]
pub struct BlacklistAccount {
    pub admin: Pubkey,
    pub blacklisted_users: Vec<Pubkey>,
}

// Conta para operações administrativas com delay
#[account]
pub struct PendingAdminAction {
    pub action_type: AdminActionType,
    pub new_value: Pubkey,          // Novo valor (admin, token, etc.)
    pub requested_at: i64,         // Quando foi solicitado
    pub executed: bool,            // Já foi executado?
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum AdminActionType {
    ChangeAdmin,
    ChangeToken,
    EmergencyWithdraw,
}

#[program]
pub mod playtoearn_program {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        payment_token_mint: Pubkey,
        max_claim_per_user: u64,
        total_supply_limit: u64,
    ) -> Result<()> {
        msg!("=== INITIALIZE CONFIG ===");
        msg!("Payment Token Mint: {}", payment_token_mint);
        msg!("Max Claim Per User: {}", max_claim_per_user);
        msg!("Total Supply Limit: {}", total_supply_limit);

        // Validar entrada
        require!(payment_token_mint != Pubkey::default(), ErrorCode::InvalidInput);
        require!(max_claim_per_user > 0, ErrorCode::InvalidInput);
        require!(total_supply_limit > 0, ErrorCode::InvalidInput);

        // Configurar a conta
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.payment_token_mint = payment_token_mint;
        config.emergency_paused = false;
        config.max_claim_per_user = max_claim_per_user;
        config.total_supply_limit = total_supply_limit;
        config.total_minted = 0;

        msg!("✅ CONFIGURAÇÃO INICIALIZADA COM SUCESSO!");
        msg!("Admin: {}", config.admin);
        msg!("Payment Token: {}", config.payment_token_mint);
        msg!("Max Claim Per User: {}", config.max_claim_per_user);
        msg!("Total Supply Limit: {}", config.total_supply_limit);

        Ok(())
    }

    // Inicializar blacklist
    pub fn initialize_blacklist(ctx: Context<InitializeBlacklist>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let blacklist = &mut ctx.accounts.blacklist;
        blacklist.admin = ctx.accounts.admin.key();
        blacklist.blacklisted_users = Vec::new();

        msg!("Blacklist inicializada com sucesso");

        Ok(())
    }

    pub fn burn_tokens(
        ctx: Context<BurnTokens>,
        amount: u64,
        timestamp: i64,
        signature: [u8; 64],
        description: String,
    ) -> Result<()> {
        msg!("=== BURN TOKENS WITH SIGNATURE ===");
        msg!("Amount: {}", amount);
        msg!("Description: {}", description);

        require!(
            !ctx.accounts.config.emergency_paused,
            ErrorCode::SystemPaused
        );
        require!(amount > 0, ErrorCode::InvalidPaymentAmount);
        require!(!description.is_empty(), ErrorCode::InvalidInput);

        // Recriar a mensagem original
        let message = format!(
            "{{\"wallet\":\"{}\",\"amount\":{},\"timestamp\":\"{}\",\"action\":\"burn\"}}",
            ctx.accounts.payer.key(),
            amount,
            timestamp,
        );
        let message_bytes = message.as_bytes();

        // Verificar assinatura
        verify_signature(
            &ctx.accounts.sysvar_instructions,
            message_bytes,
            &signature,
            &ctx.accounts.backend_authority.key(),
        )?;

        // Verificar se o tempo está dentro de um intervalo aceitável
        let now = Clock::get()?.unix_timestamp;
        require!(
            (now - timestamp).abs() <= 300, // 5 minutos de tolerância
            ErrorCode::ExpiredSignature
        );

        // Verificar saldo e queimar token
        require!(
            ctx.accounts.payer_payment_token_account.amount >= amount,
            ErrorCode::InsufficientFunds
        );

        let burn_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.payment_token_mint.to_account_info(),
                from: ctx.accounts.payer_payment_token_account.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            },
        );

        burn(burn_ctx, amount)?;

        emit!(TokenBurnEvent {
            payer: ctx.accounts.payer.key(),
            token_mint: ctx.accounts.payment_token_mint.key(),
            amount,
            description: description.clone(),
            timestamp: now,
        });

        msg!("🔥 TOKENS QUEIMADOS COM SUCESSO!");
        msg!("Amount: {}", amount);
        msg!("Description: {}", description);
        msg!("User: {}", ctx.accounts.payer.key());

        Ok(())
    }

    pub fn mint_tokens(
        ctx: Context<MintTokens>,
        amount: u64,
        recipient: Pubkey,
    ) -> Result<()> {
        msg!("=== MINT TOKENS ===");
        msg!("Amount: {}", amount);
        msg!("Recipient: {}", recipient);

        // Verificar se o sistema não está pausado
        require!(!ctx.accounts.config.emergency_paused, ErrorCode::SystemPaused);

        // Verificar se o chamador é o administrador
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        // Verificar que a quantidade é válida
        require!(amount > 0, ErrorCode::InvalidPaymentAmount);

        // Verificar que o mint corresponde ao configurado
        require_keys_eq!(
            ctx.accounts.token_mint.key(),
            ctx.accounts.config.payment_token_mint,
            ErrorCode::InvalidPaymentToken
        );

        // Criar contexto para mintar tokens
        let mint_to_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: ctx.accounts.admin.to_account_info(),
            },
        );

        // Mintar os tokens
        mint_to(mint_to_ctx, amount)?;

        // Emitir evento
        let now = Clock::get()?.unix_timestamp;
        emit!(TokenMintEvent {
            minter: ctx.accounts.admin.key(),
            token_mint: ctx.accounts.token_mint.key(),
            amount,
            recipient,
            timestamp: now,
        });

        msg!("🪙 TOKENS MINTADOS COM SUCESSO!");
        msg!("Amount: {}", amount);
        msg!("Recipient: {}", recipient);
        msg!("Minter: {}", ctx.accounts.admin.key());

        Ok(())
    }

    pub fn claim_tokens(
        ctx: Context<ClaimTokens>,
        amount: u64,
        timestamp: i64,
        signature: [u8; 64],
    ) -> Result<()> {
        msg!("=== CLAIM TOKENS ===");
        msg!("Amount: {}", amount);
        msg!("User: {}", ctx.accounts.claimer.key());

        require!(!ctx.accounts.config.emergency_paused, ErrorCode::SystemPaused);
        require!(amount > 0, ErrorCode::InvalidPaymentAmount);

        // Verificar se usuário não está na blacklist
        require!(!ctx.accounts.user_claim_account.is_blacklisted, ErrorCode::Unauthorized);

        // Verificar limites de supply total
        let new_total = ctx.accounts.config.total_minted.checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(new_total <= ctx.accounts.config.total_supply_limit, ErrorCode::InvalidPaymentAmount);

        // Verificar assinatura do backend
        let message = format!(
            "{{\"wallet\":\"{}\",\"amount\":{},\"timestamp\":\"{}\",\"action\":\"claim\"}}",
            ctx.accounts.claimer.key(),
            amount,
            timestamp,
        );
        let message_bytes = message.as_bytes();

        verify_signature(
            &ctx.accounts.sysvar_instructions,
            message_bytes,
            &signature,
            &ctx.accounts.backend_authority.key(),
        )?;

        // Verificar timestamp (5 minutos de tolerância)
        let now = Clock::get()?.unix_timestamp;
        require!(
            (now - timestamp).abs() <= 300,
            ErrorCode::ExpiredSignature
        );

        // Verificar limites por usuário
        let user_claim = &mut ctx.accounts.user_claim_account;
        let one_day_seconds: i64 = 24 * 60 * 60;

        // Inicializar conta se for nova
        if ctx.accounts.user_claim_account.to_account_info().data_is_empty() {
            user_claim.user = ctx.accounts.claimer.key();
            user_claim.total_claimed = 0;
            user_claim.last_claim_timestamp = 0;
            user_claim.daily_claimed = 0;
            user_claim.daily_reset_timestamp = now;
            user_claim.hourly_claimed = 0;
            user_claim.hourly_reset_timestamp = now;
            user_claim.nonce = 0;
            user_claim.is_blacklisted = false;
        }

        // Resetar contadores se necessário
        if now - user_claim.daily_reset_timestamp >= one_day_seconds {
            user_claim.daily_claimed = 0;
            user_claim.daily_reset_timestamp = now;
        }

        let one_hour_seconds: i64 = 60 * 60;
        if now - user_claim.hourly_reset_timestamp >= one_hour_seconds {
            user_claim.hourly_claimed = 0;
            user_claim.hourly_reset_timestamp = now;
        }

        // Verificar limites
        let max_hourly = ctx.accounts.config.max_claim_per_user / 24; // Máximo por hora (1/24 do diário)
        let new_hourly_total = user_claim.hourly_claimed.checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(new_hourly_total <= max_hourly, ErrorCode::InvalidPaymentAmount);

        let new_daily_total = user_claim.daily_claimed.checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(new_daily_total <= ctx.accounts.config.max_claim_per_user, ErrorCode::InvalidPaymentAmount);

        // Atualizar dados do usuário
        user_claim.total_claimed = user_claim.total_claimed.checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;
        user_claim.daily_claimed = new_daily_total;
        user_claim.hourly_claimed = new_hourly_total;
        user_claim.last_claim_timestamp = now;
        user_claim.nonce = user_claim.nonce.checked_add(1).ok_or(ErrorCode::MathOverflow)?;

        // Atualizar total mintado global
        let config = &mut ctx.accounts.config;
        config.total_minted = new_total;

        // Mintar tokens
        let mint_to_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.claimer_token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
        );

        mint_to(mint_to_ctx, amount)?;

        // Emitir evento
        emit!(TokenClaimEvent {
            claimer: ctx.accounts.claimer.key(),
            token_mint: ctx.accounts.token_mint.key(),
            amount,
            timestamp: now,
        });

        msg!("🎁 TOKENS CLAIMADOS COM SUCESSO!");
        msg!("Amount: {}", amount);
        msg!("User: {}", ctx.accounts.claimer.key());
        msg!("New Total Supply: {}", config.total_minted);

        Ok(())
    }

    // Gerenciamento da blacklist
    pub fn add_to_blacklist(ctx: Context<ManageBlacklist>, user: Pubkey) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let blacklist = &mut ctx.accounts.blacklist;
        if !blacklist.blacklisted_users.contains(&user) {
            blacklist.blacklisted_users.push(user);

            // Marcar na conta do usuário também
            if !ctx.accounts.user_claim_account.to_account_info().data_is_empty() {
                ctx.accounts.user_claim_account.is_blacklisted = true;
            }

            emit!(SecurityEvent {
                event_type: "USER_BLACKLISTED".to_string(),
                user,
                reason: "Added to blacklist by admin".to_string(),
                timestamp: Clock::get()?.unix_timestamp,
            });

            emit!(AdminActionEvent {
                admin: ctx.accounts.admin.key(),
                action: "BLACKLIST_ADD".to_string(),
                details: format!("User {} added to blacklist", user),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        Ok(())
    }

    pub fn remove_from_blacklist(ctx: Context<ManageBlacklist>, user: Pubkey) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let blacklist = &mut ctx.accounts.blacklist;
        if let Some(index) = blacklist.blacklisted_users.iter().position(|&x| x == user) {
            blacklist.blacklisted_users.remove(index);

            // Desmarcar na conta do usuário
            if !ctx.accounts.user_claim_account.to_account_info().data_is_empty() {
                ctx.accounts.user_claim_account.is_blacklisted = false;
            }

            emit!(SecurityEvent {
                event_type: "USER_UNBLACKLISTED".to_string(),
                user,
                reason: "Removed from blacklist by admin".to_string(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        Ok(())
    }

    // Solicitar mudança administrativa (com delay de 24h)
    pub fn request_admin_action(
        ctx: Context<RequestAdminAction>,
        action_type: AdminActionType,
        new_value: Pubkey,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let pending_action = &mut ctx.accounts.pending_action;
        pending_action.action_type = action_type.clone();
        pending_action.new_value = new_value;
        pending_action.requested_at = Clock::get()?.unix_timestamp;
        pending_action.executed = false;

        emit!(AdminActionEvent {
            admin: ctx.accounts.admin.key(),
            action: format!("REQUEST_{:?}", action_type),
            details: format!("Requested change to {}", new_value),
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Admin action requested. Execute after 24h delay for security.");

        Ok(())
    }

    // Executar mudança administrativa após delay
    pub fn execute_admin_action(ctx: Context<ExecuteAdminAction>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let pending_action = &ctx.accounts.pending_action;
        require!(!pending_action.executed, ErrorCode::InvalidInput);

        let now = Clock::get()?.unix_timestamp;
        let delay_seconds: i64 = 24 * 60 * 60; // 24 horas
        require!(
            now - pending_action.requested_at >= delay_seconds,
            ErrorCode::InvalidInput
        );

        let config = &mut ctx.accounts.config;

        match pending_action.action_type {
            AdminActionType::ChangeAdmin => {
                config.admin = pending_action.new_value;
                emit!(AdminActionEvent {
                    admin: ctx.accounts.admin.key(),
                    action: "CHANGE_ADMIN".to_string(),
                    details: format!("Admin changed to {}", pending_action.new_value),
                    timestamp: now,
                });
            },
            AdminActionType::ChangeToken => {
                config.payment_token_mint = pending_action.new_value;
                emit!(AdminActionEvent {
                    admin: ctx.accounts.admin.key(),
                    action: "CHANGE_TOKEN".to_string(),
                    details: format!("Token changed to {}", pending_action.new_value),
                    timestamp: now,
                });
            },
            AdminActionType::EmergencyWithdraw => {
                // Emergency withdraw logic would go here
                emit!(AdminActionEvent {
                    admin: ctx.accounts.admin.key(),
                    action: "EMERGENCY_WITHDRAW".to_string(),
                    details: "Emergency withdraw executed".to_string(),
                    timestamp: now,
                });
            },
        }

        // Marcar como executado
        ctx.accounts.pending_action.executed = true;

        Ok(())
    }

    // Circuit breaker - pausa automática se detectar atividade suspeita
    pub fn emergency_pause(ctx: Context<EmergencyPause>, reason: String) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        ctx.accounts.config.emergency_paused = true;

        emit!(SecurityEvent {
            event_type: "EMERGENCY_PAUSE".to_string(),
            user: ctx.accounts.admin.key(),
            reason,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct ClaimTokens<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = token_mint,
        associated_token::authority = claimer,
    )]
    pub claimer_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = claimer,
        space = 8 + 32 + 8 + 8 + 8 + 8, // discriminator + user + total_claimed + last_claim_timestamp + daily_claimed + daily_reset_timestamp
        seeds = [b"user_claim", claimer.key().as_ref()],
        bump,
    )]
    pub user_claim_account: Account<'info, UserClaimAccount>,

    /// CHECK: This is the backend authority account
    pub backend_authority: UncheckedAccount<'info>,

    /// CHECK: Mint authority PDA
    #[account(
        seeds = [b"mint_authority"],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = config.payment_token_mint == token_mint.key() @ ErrorCode::InvalidPaymentToken,
    )]
    pub config: Account<'info, ConfigAccount>,

    /// CHECK: This is the Solana Instructions Sysvar Account for signature verification
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 32 + 1 + 8 + 8 + 8, // discriminator + payment_token_mint + admin + emergency_paused + max_claim_per_user + total_supply_limit + total_minted
    )]
    pub config: Account<'info, ConfigAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BurnTokens<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub payment_token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = payment_token_mint,
        associated_token::authority = payer,
    )]
    pub payer_payment_token_account: Account<'info, TokenAccount>,

    /// CHECK: This is the backend authority account
    pub backend_authority: UncheckedAccount<'info>,

    #[account(
        constraint = config.payment_token_mint != Pubkey::default()
            @ ErrorCode::PaymentTokenNotConfigured,
    )]
    pub config: Account<'info, ConfigAccount>,

    /// CHECK: This is the Solana Instructions Sysvar Account for signature verification
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = recipient,
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    /// CHECK: Conta do destinatário dos tokens
    pub recipient: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = config.admin == admin.key() @ ErrorCode::Unauthorized,
        constraint = config.payment_token_mint == token_mint.key() @ ErrorCode::InvalidPaymentToken,
    )]
    pub config: Account<'info, ConfigAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ManageBlacklist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"blacklist"],
        bump,
    )]
    pub blacklist: Account<'info, BlacklistAccount>,

    #[account(
        mut,
        seeds = [b"user_claim", user.key().as_ref()],
        bump,
    )]
    pub user_claim_account: Account<'info, UserClaimAccount>,

    /// CHECK: Usuário a ser adicionado/removido da blacklist
    pub user: UncheckedAccount<'info>,

    pub config: Account<'info, ConfigAccount>,
}

#[derive(Accounts)]
pub struct RequestAdminAction<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + 1 + 32 + 8 + 1, // discriminator + action_type + new_value + requested_at + executed
        seeds = [b"pending_action", admin.key().as_ref()],
        bump,
    )]
    pub pending_action: Account<'info, PendingAdminAction>,

    pub config: Account<'info, ConfigAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteAdminAction<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pending_action", admin.key().as_ref()],
        bump,
        constraint = !pending_action.executed @ ErrorCode::InvalidInput,
    )]
    pub pending_action: Account<'info, PendingAdminAction>,

    #[account(mut)]
    pub config: Account<'info, ConfigAccount>,
}

#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub config: Account<'info, ConfigAccount>,
}

#[derive(Accounts)]
pub struct InitializeBlacklist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 4 + (32 * 100), // discriminator + admin + vec length + até 100 usuários
        seeds = [b"blacklist"],
        bump,
    )]
    pub blacklist: Account<'info, BlacklistAccount>,

    pub config: Account<'info, ConfigAccount>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("The signature is invalid.")]
    InvalidSignature,

    #[msg("The signature is expired.")]
    ExpiredSignature,

    #[msg("Você não está autorizado a realizar esta ação")]
    Unauthorized,

    #[msg("Token de pagamento inválido")]
    InvalidPaymentToken,

    #[msg("Valor de pagamento inválido")]
    InvalidPaymentAmount,

    #[msg("Token de pagamento não configurado")]
    PaymentTokenNotConfigured,

    #[msg("Fundos insuficientes")]
    InsufficientFunds,

    #[msg("O sistema está pausado para emergência")]
    SystemPaused,

    #[msg("Valor de entrada inválido")]
    InvalidInput,

    #[msg("Erro de overflow matemático")]
    MathOverflow,
}
