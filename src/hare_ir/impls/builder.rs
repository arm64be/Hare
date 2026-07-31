use std::collections::BTreeMap;

use crate::{
    CoverageState, ExceptionHandlerKind, FunctionId, FunctionRelation, FunctionSpecialization,
    HARE_IR_SCHEMA_VERSION, ImportError, InputKind, OperandRole, OperandValue, OwnedVisitorUnit,
    PINNED_BUN_REVISION, PINNED_WEBKIT_REVISION, SourceId, SourceRecord, SourceText,
    VisitorConstant, VisitorExceptionHandler, VisitorFunction, VisitorInstruction, VisitorOperand,
    VisitorSimpleSwitchTable, VisitorStringSwitchEntry, VisitorStringSwitchTable,
};

/// Single-use builder populated by the sealed JSC bridge.
pub struct ImportBuilder {
    unit: OwnedVisitorUnit,
    active_function: Option<FunctionId>,
    next_instruction_offset: u32,
}

impl ImportBuilder {
    pub fn new(input_kind: InputKind, source: SourceRecord) -> Self {
        Self {
            unit: OwnedVisitorUnit {
                schema_version: HARE_IR_SCHEMA_VERSION,
                bun_revision: PINNED_BUN_REVISION.into(),
                webkit_revision: PINNED_WEBKIT_REVISION.into(),
                input_kind,
                sources: vec![source],
                functions: Vec::new(),
                definition_coverage: BTreeMap::new(),
                structurally_complete: false,
            },
            active_function: None,
            next_instruction_offset: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_function(
        &mut self,
        id: FunctionId,
        parent: Option<FunctionId>,
        relation: FunctionRelation,
        specialization: FunctionSpecialization,
        source: SourceId,
        parse_mode: u32,
        script_mode: u32,
        code_type: u32,
        lexical_features: u32,
        code_features: u32,
        num_parameters: u32,
        num_vars: u32,
        num_callee_locals: u32,
        this_register: i32,
        scope_register: i32,
        call_frame_callee_register: i32,
        call_frame_this_argument_register: i32,
        call_frame_first_argument_register: i32,
        instruction_bytes: u32,
    ) -> Result<(), ImportError> {
        if id.index() != self.unit.functions.len() {
            return Err(ImportError::FunctionOutOfOrder(id));
        }
        if self.unit.functions.iter().any(|function| function.id == id) {
            return Err(ImportError::DuplicateFunction(id));
        }
        self.unit.functions.push(VisitorFunction {
            id,
            parent,
            relation,
            specialization,
            source,
            parse_mode,
            script_mode,
            code_type,
            lexical_features,
            code_features,
            num_parameters,
            num_vars,
            num_callee_locals,
            this_register,
            scope_register,
            call_frame_callee_register,
            call_frame_this_argument_register,
            call_frame_first_argument_register,
            constants: Vec::new(),
            identifiers: Vec::new(),
            simple_switch_tables: Vec::new(),
            string_switch_tables: Vec::new(),
            exception_handlers: Vec::new(),
            instruction_bytes,
            instructions: Vec::new(),
        });
        self.active_function = Some(id);
        self.next_instruction_offset = 0;
        Ok(())
    }

    pub fn constant(&mut self, index: u32, constant: VisitorConstant) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::ConstantWithoutFunction)?;
        let function = &mut self.unit.functions[function_id.index()];
        if usize::try_from(index).ok() != Some(function.constants.len()) {
            return Err(ImportError::ConstantOutOfOrder(index));
        }
        function.constants.push(constant);
        Ok(())
    }

    pub fn identifier(&mut self, index: u32, value: SourceText) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::IdentifierWithoutFunction)?;
        let function = &mut self.unit.functions[function_id.index()];
        if usize::try_from(index).ok() != Some(function.identifiers.len()) {
            return Err(ImportError::IdentifierOutOfOrder(index));
        }
        function.identifiers.push(value);
        Ok(())
    }

    pub fn simple_switch_table(
        &mut self,
        index: u32,
        minimum: i32,
        default_offset: i32,
        is_list: bool,
        branch_offsets: Box<[i32]>,
    ) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::SwitchTableWithoutFunction)?;
        let function = &mut self.unit.functions[function_id.index()];
        if usize::try_from(index).ok() != Some(function.simple_switch_tables.len()) {
            return Err(ImportError::SimpleSwitchTableOutOfOrder(index));
        }
        function
            .simple_switch_tables
            .push(VisitorSimpleSwitchTable {
                minimum,
                default_offset,
                is_list,
                branch_offsets,
            });
        Ok(())
    }

    pub fn begin_string_switch_table(
        &mut self,
        index: u32,
        minimum_length: u32,
        maximum_length: u32,
        default_offset: i32,
        declared_entry_count: u32,
    ) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::SwitchTableWithoutFunction)?;
        let function = &mut self.unit.functions[function_id.index()];
        if usize::try_from(index).ok() != Some(function.string_switch_tables.len()) {
            return Err(ImportError::StringSwitchTableOutOfOrder(index));
        }
        function
            .string_switch_tables
            .push(VisitorStringSwitchTable {
                minimum_length,
                maximum_length,
                default_offset,
                declared_entry_count,
                entries: Vec::with_capacity(declared_entry_count as usize),
            });
        Ok(())
    }

    pub fn string_switch_entry(
        &mut self,
        table_index: u32,
        key: SourceText,
        branch_offset: i32,
        index_in_table: u32,
    ) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::SwitchTableWithoutFunction)?;
        let table = self.unit.functions[function_id.index()]
            .string_switch_tables
            .get_mut(table_index as usize)
            .ok_or(ImportError::StringSwitchEntryWithoutTable(table_index))?;
        if table.entries.len() >= table.declared_entry_count as usize {
            return Err(ImportError::StringSwitchEntryOverflow(table_index));
        }
        table.entries.push(VisitorStringSwitchEntry {
            key,
            branch_offset,
            index_in_table,
        });
        Ok(())
    }

    pub fn exception_handler(
        &mut self,
        index: u32,
        start: u32,
        end: u32,
        target: u32,
        kind: ExceptionHandlerKind,
    ) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::ExceptionHandlerWithoutFunction)?;
        let function = &mut self.unit.functions[function_id.index()];
        if usize::try_from(index).ok() != Some(function.exception_handlers.len()) {
            return Err(ImportError::ExceptionHandlerOutOfOrder(index));
        }
        function.exception_handlers.push(VisitorExceptionHandler {
            start,
            end,
            target,
            kind,
        });
        Ok(())
    }

    pub fn instruction(
        &mut self,
        byte_offset: u32,
        opcode_id: u32,
        encoded_size: u32,
        opcode_id_bytes: u32,
        width_bytes: u32,
    ) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::InstructionWithoutFunction)?;
        if byte_offset != self.next_instruction_offset {
            return Err(ImportError::InstructionOffset {
                expected: self.next_instruction_offset,
                actual: byte_offset,
            });
        }
        if encoded_size == 0
            || !matches!(opcode_id_bytes, 1 | 2 | 4)
            || !matches!(width_bytes, 1 | 2 | 4)
        {
            return Err(ImportError::InvalidInstructionWidth(byte_offset));
        }
        let function = self
            .unit
            .functions
            .get_mut(function_id.index())
            .ok_or(ImportError::InstructionWithoutFunction)?;
        function.instructions.push(VisitorInstruction {
            byte_offset,
            opcode_id,
            encoded_size,
            opcode_id_bytes,
            width_bytes,
            operands: Vec::new(),
        });
        self.next_instruction_offset = byte_offset
            .checked_add(encoded_size)
            .ok_or(ImportError::IntegerOverflow)?;
        Ok(())
    }

    pub fn mark_definition(&mut self, manifest_id: Box<str>, state: CoverageState) {
        self.unit.definition_coverage.insert(manifest_id, state);
    }

    pub fn operand(
        &mut self,
        manifest_id: Box<str>,
        role: OperandRole,
        value: OperandValue,
    ) -> Result<(), ImportError> {
        let function_id = self
            .active_function
            .ok_or(ImportError::OperandWithoutInstruction)?;
        let instruction = self
            .unit
            .functions
            .get_mut(function_id.index())
            .and_then(|function| function.instructions.last_mut())
            .ok_or(ImportError::OperandWithoutInstruction)?;
        if role == OperandRole::CacheOnly || value == OperandValue::Excluded {
            return Err(ImportError::CacheOperandCrossedBoundary(manifest_id));
        }
        if instruction
            .operands
            .iter()
            .any(|operand| operand.manifest_id == manifest_id)
        {
            return Err(ImportError::DuplicateOperandManifest(manifest_id));
        }
        instruction.operands.push(VisitorOperand {
            manifest_id,
            role,
            value,
        });
        Ok(())
    }

    pub fn mark_structurally_complete(&mut self) {
        self.unit.structurally_complete = true;
    }

    pub fn finish(mut self) -> Result<OwnedVisitorUnit, ImportError> {
        for function in &mut self.unit.functions {
            for table in &mut function.string_switch_tables {
                table
                    .entries
                    .sort_by(|left, right| left.key.cmp(&right.key));
            }
        }
        self.unit.validate().map_err(ImportError::Validation)?;
        Ok(self.unit)
    }
}
