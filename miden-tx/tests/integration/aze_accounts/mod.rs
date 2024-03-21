use miden_lib::{accounts::aze_accounts::create_basic_aze_game_account, AuthScheme};
use miden_objects::{
    accounts::{Account, AccountId, AccountStorage, StorageSlotType},
    assembly::ProgramAst,
    assets::{Asset, AssetVault, FungibleAsset},
    crypto::dsa::rpo_falcon512::{KeyPair, PublicKey},
    transaction::TransactionArgs,
    Felt, Word, ONE, ZERO,
};
use miden_tx::TransactionExecutor;
use mock::{
    constants::{
        ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN, ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_ON_CHAIN,
        ACCOUNT_ID_SENDER, DEFAULT_AUTH_SCRIPT,
    },
    utils::prepare_word,
};

use crate::{
    get_account_with_default_account_code, get_new_key_pair_with_advice_map,
    get_note_with_fungible_asset_and_script, prove_and_verify_transaction, MockDataStore,
    get_game_account_with_default_account_code
};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn aze_game_account_creation() {
    // we need a Falcon Public Key to create the wallet account

    use miden_objects::accounts::AccountType;
    let key_pair: KeyPair = KeyPair::new().unwrap();
    let pub_key: PublicKey = key_pair.public_key();
    let auth_scheme: AuthScheme = AuthScheme::RpoFalcon512 { pub_key };

    // we need to use an initial seed to create the wallet account
    let init_seed: [u8; 32] = [
        95, 113, 209, 94, 84, 105, 250, 242, 223, 203, 216, 124, 22, 159, 14, 132, 215, 85, 183,
        204, 149, 90, 166, 68, 100, 73, 106, 168, 125, 237, 138, 16,
    ];

    let (game_account, _) =
        create_basic_aze_game_account(init_seed, auth_scheme, AccountType::RegularAccountImmutableCode)
            .unwrap();

    // sender_account_id not relevant here, just to create a default account code
    let sender_account_id = AccountId::try_from(ACCOUNT_ID_SENDER).unwrap();
    let expected_code_root =
        get_game_account_with_default_account_code(sender_account_id, pub_key.into(), None)
            .code()
            .root();

    assert!(game_account.is_regular_account());
    assert_eq!(game_account.code().root(), expected_code_root);
    let pub_key_word: Word = pub_key.into();
    assert_eq!(game_account.storage().get_item(0).as_elements(), pub_key_word);
}
