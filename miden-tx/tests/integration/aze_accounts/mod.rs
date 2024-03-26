use miden_lib::{accounts::{aze_accounts::create_basic_aze_game_account, player_accounts::create_basic_aze_player_account}, AuthScheme};
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
    get_game_account_with_default_account_code,
    get_player_account_with_default_account_code
};

#[test]
// Testing the game account dealing card notes
fn prove_send_card_via_aze_game_account() {
    // Mock data
    // We need an asset and an account that owns that asset
    // Create assets

    let sender_account_id = AccountId::try_from(ACCOUNT_ID_SENDER).unwrap();
    let (sender_pub_key, sender_keypair_felt) = get_new_key_pair_with_advice_map();
    let sender_account = get_game_account_with_default_account_code(
        sender_account_id,
        sender_pub_key,
        None
    );

    // CONSTRUCT AND EXECUTE TX (Success)
    // --------------------------------------------------------------------------------------------
    let data_store = MockDataStore::with_existing(Some(sender_account.clone()), Some(vec![]));

    let mut executor = TransactionExecutor::new(data_store.clone());
    executor.load_account(sender_account.id()).unwrap();

    let block_ref = data_store.block_header.block_num();
    let note_ids = data_store.notes.iter().map(|note| note.id()).collect::<Vec<_>>();

    let recipient = [ZERO, ONE, Felt::new(2), Felt::new(3)];
    let tag = Felt::new(4);

    let tx_script_code = ProgramAst::parse(
        format!(
            "
        use.miden::contracts::auth::basic->auth_tx
        use.miden::contracts::wallets::basic->wallet

        begin
            push.{recipient}
            push.{tag}
            push.0 swap loc_store.0 padw push.0.0.0 swapdw loc_load.0
            drop
            dropw dropw
            call.auth_tx::auth_tx_rpo_falcon512
        end
        ",
            recipient = prepare_word(&recipient),
            tag = tag,
        )
        .as_str(),
    )
    .unwrap();
    let tx_script = executor
        .compile_tx_script(tx_script_code, vec![(sender_pub_key, sender_keypair_felt)], vec![])
        .unwrap();
    let tx_args: TransactionArgs = TransactionArgs::with_tx_script(tx_script);

    // Execute the transaction and get the witness
    let executed_transaction = executor
        .execute_transaction(sender_account.id(), block_ref, &note_ids, Some(tx_args))
        .unwrap();

    // Prove, serialize/deserialize and verify the transaction
    assert!(prove_and_verify_transaction(executed_transaction.clone()).is_ok());

    // clones account info
    let sender_account_storage =
        AccountStorage::new(vec![(0, (StorageSlotType::Value { value_arity: 0 }, sender_pub_key))])
            .unwrap();
    let sender_account_code = sender_account.code().clone();

    // vault delta
    let sender_account_after: Account = Account::new(
        data_store.account.id(),
        AssetVault::new(&[]).unwrap(),
        sender_account_storage,
        sender_account_code,
        Felt::new(2),
    );
    assert_eq!(executed_transaction.final_account().hash(), sender_account_after.hash());
}

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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn aze_player_account_creation() {
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
        create_basic_aze_player_account(init_seed, auth_scheme, AccountType::RegularAccountImmutableCode)
            .unwrap();

    // sender_account_id not relevant here, just to create a default account code
    let sender_account_id = AccountId::try_from(ACCOUNT_ID_SENDER).unwrap();
    let expected_code_root =
        get_player_account_with_default_account_code(sender_account_id, pub_key.into(), None)
            .code()
            .root();

    assert!(game_account.is_regular_account());
    assert_eq!(game_account.code().root(), expected_code_root);
    let pub_key_word: Word = pub_key.into();
    assert_eq!(game_account.storage().get_item(0).as_elements(), pub_key_word);
}