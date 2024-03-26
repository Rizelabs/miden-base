use miden_objects::{
    accounts::{Account, AccountCode, AccountId, AccountStorage, AccountType, StorageSlotType},
    assembly::ModuleAst,
    assets::AssetVault,
    utils::format,
    AccountError, Word, ZERO,
};

use super::{AuthScheme, TransactionKernel};
use crate::utils::{string::*, vec};

// BASIC AZE GAME ACCOUNT
// ================================================================================================

/// Creates a new account with basic wallet interface and the specified authentication scheme.
/// Basic wallets can be specified to have either mutable or immutable code.
///
/// The basic wallet interface exposes two procedures:
/// - `receive_asset`, which can be used to add an asset to the account.
/// - `send_asset`, which can be used to remove an asset from the account and put into a note
///    addressed to the specified recipient.
///
/// Both methods require authentication. The authentication procedure is defined by the specified
/// authentication scheme. Public key information for the scheme is stored in the account storage
/// at slot 0.
pub fn create_basic_aze_player_account(
    init_seed: [u8; 32],
    auth_scheme: AuthScheme,
    account_type: AccountType,
) -> Result<(Account, Word), AccountError> {
    if matches!(account_type, AccountType::FungibleFaucet | AccountType::NonFungibleFaucet) {
        return Err(AccountError::AccountIdInvalidFieldElement(
            "Basic aze player accounts cannot have a faucet account type".to_string(),
        ));
    }

    let (_, storage_slot_0_data): (&str, Word) = match auth_scheme {
        AuthScheme::RpoFalcon512 { pub_key } => ("basic::auth_tx_rpo_falcon512", pub_key.into()),
    };

    let aze_player_account_code_string: String = format!(
        "
        use.miden::contracts::wallets::basic->basic_wallet
        use.miden::contracts::auth::basic->basic_eoa
        use.miden::account
    
        export.basic_wallet::receive_asset
        export.basic_wallet::send_asset
        export.basic_eoa::auth_tx_rpo_falcon512
    
        export.account::set_item
        "
    );

    let aze_player_account_code_src: &str = &aze_player_account_code_string;

    let aze_player_account_code_ast = ModuleAst::parse(aze_player_account_code_src)
        .map_err(|e| AccountError::AccountCodeAssemblerError(e.into()))?;
    let account_assembler = TransactionKernel::assembler();
    let aze_player_account_code = AccountCode::new(aze_player_account_code_ast.clone(), &account_assembler)?;

    let aze_player_account_storage = AccountStorage::new(vec![(
        0,
        (StorageSlotType::Value { value_arity: 0 }, storage_slot_0_data),
    )])?;
    let account_vault = AssetVault::new(&[]).expect("error on empty vault");

    let account_seed = AccountId::get_account_seed(
        init_seed,
        account_type,
        false,
        aze_player_account_code.root(),
        aze_player_account_storage.root(),
    )?;
    let account_id = AccountId::new(account_seed, aze_player_account_code.root(), aze_player_account_storage.root())?;
    Ok((
        Account::new(account_id, account_vault, aze_player_account_storage, aze_player_account_code, ZERO),
        account_seed,
    ))
}
