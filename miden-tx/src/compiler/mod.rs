use super::{
    AccountCode, AccountId, BTreeMap, CodeBlock, Digest, NoteScript, Operation, Program, SatKernel,
    TransactionCompilerError,
};
use miden_objects::{
    assembly::{Assembler, AssemblyContext, ModuleAst, ProgramAst},
    notes::RecordedNote,
};
use vm_processor::ProgramInfo;

#[cfg(test)]
mod tests;

// TRANSACTION COMPILER
// ================================================================================================

/// The transaction compiler is responsible for building executable programs for Miden rollup
/// transactions.
///
/// The generated programs can then be executed on the Miden VM to update the states of accounts
/// involved in these transactions.
///
/// In addition to transaction compilation, transaction compiler provides methods which can be
/// used to compile Miden account code and note scripts.
pub struct TransactionCompiler {
    assembler: Assembler,
    account_procedures: BTreeMap<AccountId, Vec<Digest>>,
    kernel_main: CodeBlock,
}

impl TransactionCompiler {
    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    /// Returns a new instance of the [TransactionComplier].
    pub fn new() -> TransactionCompiler {
        let assembler = miden_lib::assembler::assembler();

        // compile transaction kernel main
        let main_ast = ProgramAst::parse(SatKernel::main()).expect("main is well formed");
        let kernel_main = assembler
            .compile_in_context(&main_ast, &mut AssemblyContext::for_program(Some(&main_ast)))
            .expect("main is well formed");

        TransactionCompiler {
            assembler,
            account_procedures: BTreeMap::default(),
            kernel_main,
        }
    }

    // ACCOUNT CODE AND NOTE SCRIPT COMPILERS
    // --------------------------------------------------------------------------------------------

    /// Compiles the provided module into [AccountCode] and associates the resulting procedures
    /// with the specified account ID.
    pub fn load_account(
        &mut self,
        account_id: AccountId,
        account_code: ModuleAst,
    ) -> Result<AccountCode, TransactionCompilerError> {
        let account_code = AccountCode::new(account_code, &self.assembler)
            .map_err(TransactionCompilerError::LoadAccountFailed)?;
        self.account_procedures.insert(account_id, account_code.procedures().to_vec());
        Ok(account_code)
    }

    /// Loads the provided account interface (vector of procedure digests) into the this compiler.
    /// Returns the old account interface if it previously existed.
    pub fn load_account_interface(
        &mut self,
        account_id: AccountId,
        procedures: Vec<Digest>,
    ) -> Option<Vec<Digest>> {
        self.account_procedures.insert(account_id, procedures)
    }

    /// Compiles the provided program into the [NoteScript] and checks (to the extent possible)
    /// if a note could be executed against all accounts with the specified interfaces.
    pub fn compile_note_script(
        &mut self,
        note_script_ast: ProgramAst,
        target_account_proc: Vec<NoteTarget>,
    ) -> Result<NoteScript, TransactionCompilerError> {
        let (note_script, code_block) = NoteScript::new(note_script_ast, &self.assembler)
            .map_err(|_| TransactionCompilerError::CompileNoteScriptFailed)?;
        for note_target in target_account_proc.into_iter() {
            verify_program_account_compatibility(
                &code_block,
                &self.get_target_interface(note_target)?,
            )
            .map_err(|_| {
                TransactionCompilerError::NoteIncompatibleWithAccountInterface(code_block.hash())
            })?;
        }

        Ok(note_script)
    }

    // TRANSACTION PROGRAM BUILDER
    // --------------------------------------------------------------------------------------------
    /// Compiles a transaction which executes the provided notes and an optional tx script against
    /// the specified account. Returns the a tuple containing the compiled program and the root
    /// hash of the transaction script if it was provided.
    ///
    /// The account is assumed to have been previously loaded into this compiler.
    pub fn compile_transaction(
        &mut self,
        account_id: AccountId,
        notes: &[RecordedNote],
        tx_script: Option<ProgramAst>,
    ) -> Result<(Program, Option<Digest>), TransactionCompilerError> {
        // Fetch the account interface from the `account_procedures` map. Return an error if the
        // interface is not found.
        let target_account_interface = self
            .account_procedures
            .get(&account_id)
            .cloned()
            .ok_or(TransactionCompilerError::AccountInterfaceNotFound(account_id))?;

        // Transaction must contain at least one input note or a transaction script
        if notes.is_empty() && tx_script.is_none() {
            return Err(TransactionCompilerError::InvalidTransactionInputs);
        }

        // Create the [AssemblyContext] for compilation of notes scripts and the transaction script
        let mut assembly_context = AssemblyContext::for_program(None);

        // Compile note scripts
        let note_script_programs =
            self.compile_notes(&target_account_interface, notes, &mut assembly_context)?;

        // Create the transaction program
        let (tx_script_program, tx_script_hash) = self.compile_tx_script_program(
            tx_script,
            &mut assembly_context,
            target_account_interface,
        )?;

        // Create [CodeBlockTable] from [AssemblyContext]
        let mut cb_table = self
            .assembler
            .build_cb_table(assembly_context)
            .map_err(TransactionCompilerError::BuildCodeBlockTableFailed)?;

        // insert note roots into [CodeBlockTable]
        note_script_programs.into_iter().for_each(|note_root| {
            cb_table.insert(note_root);
        });

        // insert transaction script into [CodeBlockTable]
        cb_table.insert(tx_script_program);

        // Create transaction program with kernel
        let program = Program::with_kernel(
            self.kernel_main.clone(),
            self.assembler.kernel().clone(),
            cb_table,
        );

        // Create compiled transaction
        Ok((program, tx_script_hash))
    }

    /// Returns a [ProgramInfo] associated with the transaction kernel program.
    pub fn build_program_info(&self) -> ProgramInfo {
        ProgramInfo::new(self.kernel_main.hash(), self.assembler.kernel().clone())
    }

    // HELPER METHODS
    // --------------------------------------------------------------------------------------------

    /// Compiles the provided notes into [CodeBlock]s (programs) and verifies that each note is
    /// compatible with the target account interfaces. Returns a vector of the compiled note
    /// programs.
    fn compile_notes(
        &mut self,
        target_account_interface: &[Digest],
        notes: &[RecordedNote],
        assembly_context: &mut AssemblyContext,
    ) -> Result<Vec<CodeBlock>, TransactionCompilerError> {
        let mut note_programs = Vec::new();

        // Create and verify note programs. Note programs are verified against the target account.
        for recorded_note in notes.iter() {
            let note_program = self
                .assembler
                .compile_in_context(recorded_note.note().script().code(), assembly_context)
                .map_err(|_| TransactionCompilerError::CompileNoteScriptFailed)?;
            verify_program_account_compatibility(&note_program, target_account_interface).map_err(
                |_| {
                    TransactionCompilerError::NoteIncompatibleWithAccountInterface(
                        note_program.hash(),
                    )
                },
            )?;
            note_programs.push(note_program);
        }

        Ok(note_programs)
    }

    /// Returns a ([CodeBlock], Option<Digest>) tuple where the first element is the compiled
    /// transaction script program and the second element is the hash of the transaction script
    /// program. If no transaction script is provided, the first element is a [CodeBlock] containing
    /// a single [Operation::Noop] and the second element is `None`.
    fn compile_tx_script_program(
        &mut self,
        tx_script: Option<ProgramAst>,
        assembly_context: &mut AssemblyContext,
        target_account_interface: Vec<Digest>,
    ) -> Result<(CodeBlock, Option<Digest>), TransactionCompilerError> {
        let tx_script_is_some = tx_script.is_some();
        let tx_script_code_block = match tx_script {
            Some(tx_script) => self
                .assembler
                .compile_in_context(&tx_script, assembly_context)
                .map_err(TransactionCompilerError::CompileTxScriptFailed)?,
            None => CodeBlock::new_span(vec![Operation::Noop]),
        };
        verify_program_account_compatibility(&tx_script_code_block, &target_account_interface)
            .map_err(|_| {
                TransactionCompilerError::TxScriptIncompatibleWithAccountInterface(
                    tx_script_code_block.hash(),
                )
            })?;
        let tx_script_hash = tx_script_is_some.then_some(tx_script_code_block.hash());
        Ok((tx_script_code_block, tx_script_hash))
    }

    /// Returns the account interface associated with the provided [NoteTarget].
    ///
    /// # Errors
    /// - If the account interface associated with the [AccountId] provided as a target can not be
    ///   found in the `account_procedures` map.
    fn get_target_interface(
        &self,
        target: NoteTarget,
    ) -> Result<Vec<Digest>, TransactionCompilerError> {
        match target {
            NoteTarget::AccountId(id) => self
                .account_procedures
                .get(&id)
                .cloned()
                .ok_or(TransactionCompilerError::AccountInterfaceNotFound(id)),
            NoteTarget::Procedures(procs) => Ok(procs),
        }
    }
}

impl Default for TransactionCompiler {
    fn default() -> Self {
        Self::new()
    }
}

// TRANSACTION COMPILER HELPERS
// ------------------------------------------------------------------------------------------------

/// Verifies that the provided program is compatible with the target account interface.
/// This is achieved by checking that at least one execution branch in the program is compatible
/// with the target account interface.
///
/// # Errors
/// Returns an error if the note script is not compatible with the target account interface.
fn verify_program_account_compatibility(
    program: &CodeBlock,
    target_account_interface: &[Digest],
) -> Result<(), TransactionCompilerError> {
    // collect call branches
    let branches = collect_call_branches(program);

    // if none of the branches are compatible with the target account, return an error
    if !branches.iter().any(|call_targets| {
        call_targets.iter().all(|target| target_account_interface.contains(target))
    }) {
        return Err(TransactionCompilerError::ProgramIncompatibleWithAccountInterface(
            program.hash(),
        ));
    }

    Ok(())
}

/// Collect call branches by recursively traversing through program execution branches and
/// accumulating call targets.
fn collect_call_branches(code_block: &CodeBlock) -> Vec<Vec<Digest>> {
    let mut branches = vec![vec![]];
    recursively_collect_call_branches(code_block, &mut branches);
    branches
}

/// Generates a list of calls invoked in each execution branch of the provided code block.
fn recursively_collect_call_branches(code_block: &CodeBlock, branches: &mut Vec<Vec<Digest>>) {
    match code_block {
        CodeBlock::Join(block) => {
            recursively_collect_call_branches(block.first(), branches);
            recursively_collect_call_branches(block.second(), branches);
        }
        CodeBlock::Split(block) => {
            let current_len = branches.last().expect("at least one execution branch").len();
            recursively_collect_call_branches(block.on_false(), branches);

            // If the previous branch had additional calls we need to create a new branch
            if branches.last().expect("at least one execution branch").len() > current_len {
                branches.push(
                    branches.last().expect("at least one execution branch")[..current_len].to_vec(),
                );
            }

            recursively_collect_call_branches(block.on_true(), branches);
        }
        CodeBlock::Loop(block) => {
            recursively_collect_call_branches(block.body(), branches);
        }
        CodeBlock::Call(block) => {
            if block.is_syscall() {
                return;
            }

            branches
                .last_mut()
                .expect("at least one execution branch")
                .push(block.fn_hash());
        }
        CodeBlock::Span(_) => {}
        CodeBlock::Proxy(_) => {}
        CodeBlock::Dyn(_) => {}
    }
}

// NOTE TARGET
// ================================================================================================

#[derive(Clone)]
pub enum NoteTarget {
    AccountId(AccountId),
    Procedures(Vec<Digest>),
}
