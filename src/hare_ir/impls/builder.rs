use std::collections::BTreeMap;

use crate::{
    CoverageState, FunctionId, FunctionRelation, FunctionSpecialization, HARE_IR_SCHEMA_VERSION,
    ImportError, InputKind, OperandRole, OperandValue, OwnedVisitorUnit, PINNED_BUN_REVISION,
    PINNED_WEBKIT_REVISION, SourceId, SourceRecord, VisitorFunction, VisitorInstruction,
    VisitorOperand,
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
            instruction_bytes,
            instructions: Vec::new(),
        });
        self.active_function = Some(id);
        self.next_instruction_offset = 0;
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

    pub fn finish(self) -> Result<OwnedVisitorUnit, ImportError> {
        self.unit.validate().map_err(ImportError::Validation)?;
        Ok(self.unit)
    }
}
