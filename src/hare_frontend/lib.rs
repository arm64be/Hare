//! Generated JSC-instruction ownership and Tier 1 frontend dispatch.

mod impls;

use std::collections::BTreeSet;
use std::fmt;

use hare_ir::{
    EffectSet, OperandRole, OperandValue, OwnedVisitorUnit, VisitorInstruction, VisitorOperand,
};

pub use impls::{cache, control, exception, function, module, numeric, object};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OpcodeFamily {
    Control,
    Numeric,
    Object,
    Function,
    Exception,
    Module,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedValueKind {
    Signed,
    Unsigned,
    Boolean,
}

impl ImportedValueKind {
    fn accepts(self, value: &OperandValue) -> bool {
        matches!(
            (self, value),
            (Self::Signed, OperandValue::Signed(_))
                | (Self::Unsigned, OperandValue::Unsigned(_))
                | (Self::Boolean, OperandValue::Boolean(_))
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperandDescriptor {
    pub manifest_id: &'static str,
    pub name: &'static str,
    pub role: OperandRole,
    pub value_kind: ImportedValueKind,
    pub source_type: &'static str,
    pub optional: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExcludedOperandDescriptor {
    pub manifest_id: &'static str,
    pub name: &'static str,
    pub source_type: &'static str,
    pub exclusion_reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionRecordKind {
    Checkpoint,
    Temporary,
    DerivedOperand,
    MetadataField,
    MetadataReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstructionRecordDescriptor {
    pub manifest_id: &'static str,
    pub kind: InstructionRecordKind,
    pub semantic: bool,
    pub representation: &'static str,
    pub destination: Option<&'static str>,
    pub exclusion_reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpcodeDescriptor {
    pub opcode_id: u16,
    pub opcode: &'static str,
    pub generated_type: &'static str,
    pub owner: &'static str,
    pub family: OpcodeFamily,
    pub effects: EffectSet,
    pub operands: &'static [OperandDescriptor],
    pub excluded_operands: &'static [ExcludedOperandDescriptor],
    pub instruction_records: &'static [InstructionRecordDescriptor],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheOpcodeDescriptor {
    pub opcode_id: u16,
    pub opcode: &'static str,
    pub generated_type: &'static str,
    pub exclusion: &'static str,
    pub excluded_operands: &'static [ExcludedOperandDescriptor],
    pub instruction_records: &'static [InstructionRecordDescriptor],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImportCoverage {
    pub semantic_instructions: usize,
    pub excluded_cache_instructions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredInstruction {
    pub opcode_id: u16,
    pub opcode: &'static str,
    pub family: OpcodeFamily,
    pub effects: EffectSet,
    pub operands: Vec<VisitorOperand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrontendError {
    UnknownOpcode(u32),
    WrongFamily {
        opcode: &'static str,
        expected: OpcodeFamily,
        actual: OpcodeFamily,
    },
    OpcodeIdOverflow(u32),
    OperandCount {
        opcode: &'static str,
        expected: usize,
        actual: usize,
    },
    OperandManifest {
        opcode: &'static str,
        index: usize,
    },
    OperandRole {
        opcode: &'static str,
        index: usize,
    },
    OperandValueKind {
        opcode: &'static str,
        index: usize,
    },
    InventoryCount(usize),
    DuplicateOpcode(u16),
    DuplicateManifest(&'static str),
    InvalidAuxiliaryRecord(&'static str),
    CacheRole(&'static str),
    ReentryWithoutSafepoint(&'static str),
}

impl fmt::Display for FrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FrontendError {}

pub const fn inventories() -> [&'static [OpcodeDescriptor]; 6] {
    [
        control::OPCODES,
        numeric::OPCODES,
        object::OPCODES,
        function::OPCODES,
        exception::OPCODES,
        module::OPCODES,
    ]
}

pub fn descriptor(opcode_id: u32) -> Option<&'static OpcodeDescriptor> {
    let opcode_id = u16::try_from(opcode_id).ok()?;
    inventories()
        .into_iter()
        .flat_map(|inventory| inventory.iter())
        .find(|descriptor| descriptor.opcode_id == opcode_id)
}

pub fn cache_descriptor(opcode_id: u32) -> Option<&'static CacheOpcodeDescriptor> {
    let opcode_id = u16::try_from(opcode_id).ok()?;
    cache::OPCODES
        .iter()
        .find(|descriptor| descriptor.opcode_id == opcode_id)
}

pub fn lower_instruction(
    instruction: &VisitorInstruction,
) -> Result<LoweredInstruction, FrontendError> {
    let descriptor = descriptor(instruction.opcode_id)
        .ok_or(FrontendError::UnknownOpcode(instruction.opcode_id))?;
    match descriptor.family {
        OpcodeFamily::Control => control::lower(instruction),
        OpcodeFamily::Numeric => numeric::lower(instruction),
        OpcodeFamily::Object => object::lower(instruction),
        OpcodeFamily::Function => function::lower(instruction),
        OpcodeFamily::Exception => exception::lower(instruction),
        OpcodeFamily::Module => module::lower(instruction),
    }
}

pub fn validate_inventory() -> Result<(), FrontendError> {
    let all = inventories();
    let count = all.iter().map(|inventory| inventory.len()).sum::<usize>();
    if count != 183 {
        return Err(FrontendError::InventoryCount(count));
    }
    let mut opcodes = BTreeSet::new();
    let mut operands = BTreeSet::new();
    let mut excluded_operands = BTreeSet::new();
    let mut instruction_records = BTreeSet::new();
    for descriptor in all.into_iter().flatten() {
        if !opcodes.insert(descriptor.opcode_id) {
            return Err(FrontendError::DuplicateOpcode(descriptor.opcode_id));
        }
        if descriptor.effects.contains(EffectSet::MAY_CALL_USER)
            && !descriptor.effects.contains(EffectSet::SAFEPOINT)
        {
            return Err(FrontendError::ReentryWithoutSafepoint(descriptor.opcode));
        }
        for operand in descriptor.operands {
            if operand.role == OperandRole::CacheOnly {
                return Err(FrontendError::CacheRole(operand.manifest_id));
            }
            if !operands.insert(operand.manifest_id) {
                return Err(FrontendError::DuplicateManifest(operand.manifest_id));
            }
        }
        validate_auxiliary_records(
            descriptor.excluded_operands,
            descriptor.instruction_records,
            &mut excluded_operands,
            &mut instruction_records,
        )?;
    }
    for descriptor in cache::OPCODES {
        if !opcodes.insert(descriptor.opcode_id) {
            return Err(FrontendError::DuplicateOpcode(descriptor.opcode_id));
        }
        validate_auxiliary_records(
            descriptor.excluded_operands,
            descriptor.instruction_records,
            &mut excluded_operands,
            &mut instruction_records,
        )?;
    }
    if opcodes.len() != 194 || cache::OPCODES.len() != 11 {
        return Err(FrontendError::InventoryCount(opcodes.len()));
    }
    if operands.len() != 529 {
        return Err(FrontendError::InventoryCount(operands.len()));
    }
    if excluded_operands.len() != 80 || instruction_records.len() != 183 {
        return Err(FrontendError::InventoryCount(
            excluded_operands.len() + instruction_records.len(),
        ));
    }
    Ok(())
}

fn validate_auxiliary_records(
    excluded: &'static [ExcludedOperandDescriptor],
    records: &'static [InstructionRecordDescriptor],
    excluded_ids: &mut BTreeSet<&'static str>,
    record_ids: &mut BTreeSet<&'static str>,
) -> Result<(), FrontendError> {
    for operand in excluded {
        if operand.exclusion_reason.is_empty() || !excluded_ids.insert(operand.manifest_id) {
            return Err(FrontendError::InvalidAuxiliaryRecord(operand.manifest_id));
        }
    }
    for record in records {
        let classification_valid = if record.semantic {
            record.destination.is_some() && record.exclusion_reason.is_none()
        } else {
            record.destination.is_none() && record.exclusion_reason.is_some()
        };
        if !classification_valid || !record_ids.insert(record.manifest_id) {
            return Err(FrontendError::InvalidAuxiliaryRecord(record.manifest_id));
        }
    }
    Ok(())
}

pub fn validate_imported_unit(unit: &OwnedVisitorUnit) -> Result<ImportCoverage, FrontendError> {
    validate_inventory()?;
    let mut coverage = ImportCoverage::default();
    for instruction in unit
        .functions
        .iter()
        .flat_map(|function| &function.instructions)
    {
        if descriptor(instruction.opcode_id).is_some() {
            lower_instruction(instruction)?;
            coverage.semantic_instructions += 1;
        } else if cache_descriptor(instruction.opcode_id).is_some() {
            if !instruction.operands.is_empty() {
                return Err(FrontendError::OperandCount {
                    opcode: cache_descriptor(instruction.opcode_id).unwrap().opcode,
                    expected: 0,
                    actual: instruction.operands.len(),
                });
            }
            coverage.excluded_cache_instructions += 1;
        } else {
            return Err(FrontendError::UnknownOpcode(instruction.opcode_id));
        }
    }
    Ok(coverage)
}

pub(crate) fn lower_owned_instruction(
    expected_family: OpcodeFamily,
    instruction: &VisitorInstruction,
) -> Result<LoweredInstruction, FrontendError> {
    let descriptor = descriptor(instruction.opcode_id)
        .ok_or(FrontendError::UnknownOpcode(instruction.opcode_id))?;
    if descriptor.family != expected_family {
        return Err(FrontendError::WrongFamily {
            opcode: descriptor.opcode,
            expected: expected_family,
            actual: descriptor.family,
        });
    }
    if descriptor.operands.len() != instruction.operands.len() {
        return Err(FrontendError::OperandCount {
            opcode: descriptor.opcode,
            expected: descriptor.operands.len(),
            actual: instruction.operands.len(),
        });
    }
    for (index, (expected, actual)) in descriptor
        .operands
        .iter()
        .zip(&instruction.operands)
        .enumerate()
    {
        if expected.manifest_id != actual.manifest_id.as_ref() {
            return Err(FrontendError::OperandManifest {
                opcode: descriptor.opcode,
                index,
            });
        }
        if expected.role != actual.role {
            return Err(FrontendError::OperandRole {
                opcode: descriptor.opcode,
                index,
            });
        }
        if !expected.value_kind.accepts(&actual.value) {
            return Err(FrontendError::OperandValueKind {
                opcode: descriptor.opcode,
                index,
            });
        }
    }
    Ok(LoweredInstruction {
        opcode_id: u16::try_from(instruction.opcode_id)
            .map_err(|_| FrontendError::OpcodeIdOverflow(instruction.opcode_id))?,
        opcode: descriptor.opcode,
        family: descriptor.family,
        effects: descriptor.effects,
        operands: instruction.operands.clone(),
    })
}
