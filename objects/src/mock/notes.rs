use crate::mock::{
    non_fungible_asset_2, prepare_assets, prepare_word, ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN_1,
    ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN_2, ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN_3,
    ACCOUNT_ID_NON_FUNGIBLE_FAUCET_ON_CHAIN, ACCOUNT_ID_SENDER, CONSUMED_NOTE_1_AMOUNT,
    CONSUMED_NOTE_2_AMOUNT, CONSUMED_NOTE_3_AMOUNT,
};

use super::super::{
    assets::{Asset, FungibleAsset},
    notes::{Note, NoteScript},
    AccountId, Felt, Vec, Word,
};
use assembly::{ast::ProgramAst, Assembler};
use miden_core::FieldElement;

pub enum AssetPreservationStatus {
    TooFewInput,
    Preserved,
    TooManyFungibleInput,
    TooManyNonFungibleInput,
}

pub fn mock_notes(
    account_id: AccountId,
    assembler: &mut Assembler,
    asset_preservation: AssetPreservationStatus,
) -> (Vec<Note>, Vec<Note>) {
    // Note Assets
    let faucet_id_1 = AccountId::try_from(ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN_1).unwrap();
    let faucet_id_2 = AccountId::try_from(ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN_2).unwrap();
    let faucet_id_3 = AccountId::try_from(ACCOUNT_ID_FUNGIBLE_FAUCET_ON_CHAIN_3).unwrap();
    let fungible_asset_1: Asset =
        FungibleAsset::new(faucet_id_1, CONSUMED_NOTE_1_AMOUNT).unwrap().into();
    let fungible_asset_2: Asset =
        FungibleAsset::new(faucet_id_2, CONSUMED_NOTE_2_AMOUNT).unwrap().into();
    let fungible_asset_3: Asset =
        FungibleAsset::new(faucet_id_3, CONSUMED_NOTE_3_AMOUNT).unwrap().into();

    // Sender account
    let sender = AccountId::try_from(ACCOUNT_ID_SENDER).unwrap();

    // CREATED NOTES
    // --------------------------------------------------------------------------------------------
    // create note script
    let note_program_ast = ProgramAst::parse("begin push.1 drop end").unwrap();
    let (note_script, _) = NoteScript::new(note_program_ast, assembler).unwrap();

    // Created Notes
    const SERIAL_NUM_4: Word = [Felt::new(13), Felt::new(14), Felt::new(15), Felt::new(16)];
    let created_note_1 = Note::new(
        note_script.clone(),
        &[Felt::new(1)],
        &[fungible_asset_1],
        SERIAL_NUM_4,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    const SERIAL_NUM_5: Word = [Felt::new(17), Felt::new(18), Felt::new(19), Felt::new(20)];
    let created_note_2 = Note::new(
        note_script.clone(),
        &[Felt::new(2)],
        &[fungible_asset_2],
        SERIAL_NUM_5,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    const SERIAL_NUM_6: Word = [Felt::new(21), Felt::new(22), Felt::new(23), Felt::new(24)];
    let created_note_3 = Note::new(
        note_script,
        &[Felt::new(2)],
        &[fungible_asset_3],
        SERIAL_NUM_6,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    let created_notes = vec![created_note_1, created_note_2, created_note_3];

    // CONSUMED NOTES
    // --------------------------------------------------------------------------------------------

    // create note 1 script
    let note_1_script_src = format!(
        "\
        use.context::account_{account_id}

        proc.wrap_create_note
            call.account_{account_id}::create_note
            drop dropw dropw
        end

        begin
            # create note 0
            push.{created_note_0_recipient}
            push.{created_note_0_tag}
            push.{created_note_0_asset}
            exec.wrap_create_note

            # create note 1
            push.{created_note_1_recipient}
            push.{created_note_1_tag}
            push.{created_note_1_asset}
            exec.wrap_create_note
        end
    ",
        created_note_0_recipient = prepare_word(&created_notes[0].recipient()),
        created_note_0_tag = created_notes[0].metadata().tag(),
        created_note_0_asset = prepare_assets(created_notes[0].vault())[0],
        created_note_1_recipient = prepare_word(&created_notes[1].recipient()),
        created_note_1_tag = created_notes[1].metadata().tag(),
        created_note_1_asset = prepare_assets(created_notes[1].vault())[0],
    );
    let note_1_script_ast = ProgramAst::parse(&note_1_script_src).unwrap();
    let (note_1_script, _) = NoteScript::new(note_1_script_ast, assembler).unwrap();

    // create note 2 script
    let note_2_script_src = format!(
        "\
        use.context::account_{account_id}

        proc.wrap_create_note_2
            call.account_{account_id}::create_note
            drop dropw dropw
        end

        begin
            # create note 2
            push.{created_note_2_recipient}
            push.{created_note_2_tag}
            push.{created_note_2_asset}
            exec.wrap_create_note_2
        end
        ",
        created_note_2_recipient = prepare_word(&created_notes[2].recipient()),
        created_note_2_tag = created_notes[2].metadata().tag(),
        created_note_2_asset = prepare_assets(created_notes[2].vault())[0],
    );
    let note_2_script_ast = ProgramAst::parse(&note_2_script_src).unwrap();
    let (note_2_script, _) = NoteScript::new(note_2_script_ast, assembler).unwrap();

    // Consumed Notes
    const SERIAL_NUM_1: Word = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let consumed_note_1 = Note::new(
        note_1_script,
        &[Felt::new(1)],
        &[fungible_asset_1],
        SERIAL_NUM_1,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    const SERIAL_NUM_2: Word = [Felt::new(5), Felt::new(6), Felt::new(7), Felt::new(8)];
    let consumed_note_2 = Note::new(
        note_2_script,
        &[Felt::new(2)],
        &[fungible_asset_2, fungible_asset_3],
        SERIAL_NUM_2,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    let note_3_script_ast = ProgramAst::parse(&"begin push.1 drop end").unwrap();
    let (note_3_script, _) = NoteScript::new(note_3_script_ast, assembler).unwrap();

    const SERIAL_NUM_3: Word = [Felt::new(9), Felt::new(10), Felt::new(11), Felt::new(12)];
    let consumed_note_3 = Note::new(
        note_3_script.clone(),
        &[Felt::new(2)],
        &[fungible_asset_2, fungible_asset_3],
        SERIAL_NUM_3,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    let note_4_script_ast = ProgramAst::parse(&"begin push.1 drop end").unwrap();
    let (note_4_script, _) = NoteScript::new(note_4_script_ast, &assembler).unwrap();

    const SERIAL_NUM_7: Word = [Felt::new(25), Felt::new(26), Felt::new(27), Felt::new(28)];
    let consumed_note_4 = Note::new(
        note_4_script,
        &[Felt::new(1)],
        &[non_fungible_asset_2(ACCOUNT_ID_NON_FUNGIBLE_FAUCET_ON_CHAIN)],
        SERIAL_NUM_7,
        sender,
        Felt::ZERO,
        None,
    )
    .unwrap();

    let consumed_notes = match asset_preservation {
        AssetPreservationStatus::TooFewInput => vec![consumed_note_1],
        AssetPreservationStatus::Preserved => vec![consumed_note_1, consumed_note_2],
        AssetPreservationStatus::TooManyFungibleInput => {
            vec![consumed_note_1, consumed_note_2, consumed_note_3]
        }
        AssetPreservationStatus::TooManyNonFungibleInput => {
            vec![consumed_note_1, consumed_note_2, consumed_note_4]
        }
    };

    (consumed_notes, created_notes)
}
