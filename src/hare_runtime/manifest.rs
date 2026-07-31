use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use hare_frontend::{
    ImportedValueKind, OpcodeDescriptor, OpcodeFamily, OperandDescriptor, inventories,
    validate_inventory,
};
use hare_ir::{EffectSet, OperandRole, RuntimeCapabilityId};
use hare_llvm::{
    AbiOwnership, AbiType, CallingConvention, HelperEntry, HelperManifest, HelperParameter,
    LlvmError, TargetLayout,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier1Strategy {
    NativeIr,
    RuntimeHelper(RuntimeCapabilityId),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VisibleCheck {
    DecodeDomain,
    ValueTag,
    NumericCoercion,
    Bounds,
    Shape,
    Prototype,
    ProxyOrAccessor,
    Callability,
    Constructibility,
    Realm,
    Scope,
    ExceptionState,
    Interruption,
    PublishFrameAndRoots,
    PublishAmbientState,
    RestoreAmbientState,
    RevalidateAfterReentry,
    AbruptCompletion,
    Suspension,
    AllocationSafepoint,
    Trap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationContract {
    pub opcode_id: u16,
    pub opcode: &'static str,
    pub family: OpcodeFamily,
    pub strategy: Tier1Strategy,
    pub effects: EffectSet,
    pub checks: BTreeSet<VisibleCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeManifest {
    pub operations: Vec<OperationContract>,
    pub helpers: HelperManifest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeManifestError {
    Frontend(Box<str>),
    Llvm(LlvmError),
    DuplicateOpcode(u16),
    Coverage(usize),
    MissingReentryProtocol(&'static str),
    AllocationWithoutSafepoint(&'static str),
    OpaqueDispatcher(&'static str),
}

impl fmt::Display for RuntimeManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RuntimeManifestError {}

impl From<LlvmError> for RuntimeManifestError {
    fn from(error: LlvmError) -> Self {
        Self::Llvm(error)
    }
}

impl RuntimeManifest {
    pub fn build(target: TargetLayout) -> Result<Self, RuntimeManifestError> {
        validate_inventory()
            .map_err(|error| RuntimeManifestError::Frontend(error.to_string().into()))?;
        let mut operations = Vec::with_capacity(183);
        let mut helpers = HelperManifest::new(target);
        let mut next_capability = 0_u32;

        for descriptor in inventories().into_iter().flatten() {
            let checks = checks_for(descriptor);
            let strategy = if requires_runtime_helper(descriptor) {
                let capability = RuntimeCapabilityId(next_capability);
                next_capability += 1;
                helpers.insert(helper_for(descriptor, capability))?;
                Tier1Strategy::RuntimeHelper(capability)
            } else {
                Tier1Strategy::NativeIr
            };
            operations.push(OperationContract {
                opcode_id: descriptor.opcode_id,
                opcode: descriptor.opcode,
                family: descriptor.family,
                strategy,
                effects: descriptor.effects,
                checks,
            });
        }
        operations.sort_by_key(|operation| operation.opcode_id);
        let manifest = Self {
            operations,
            helpers,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), RuntimeManifestError> {
        if self.operations.len() != 183 {
            return Err(RuntimeManifestError::Coverage(self.operations.len()));
        }
        let mut ids = BTreeSet::new();
        for operation in &self.operations {
            if !ids.insert(operation.opcode_id) {
                return Err(RuntimeManifestError::DuplicateOpcode(operation.opcode_id));
            }
            if operation.opcode.contains("execute_opcode") {
                return Err(RuntimeManifestError::OpaqueDispatcher(operation.opcode));
            }
            if operation.effects.contains(EffectSet::MAY_ALLOCATE)
                && (!operation.effects.contains(EffectSet::SAFEPOINT)
                    || !operation
                        .checks
                        .contains(&VisibleCheck::AllocationSafepoint))
            {
                return Err(RuntimeManifestError::AllocationWithoutSafepoint(
                    operation.opcode,
                ));
            }
            if operation.effects.contains(EffectSet::MAY_CALL_USER) {
                let required = [
                    VisibleCheck::PublishFrameAndRoots,
                    VisibleCheck::PublishAmbientState,
                    VisibleCheck::RestoreAmbientState,
                    VisibleCheck::RevalidateAfterReentry,
                ];
                if required
                    .iter()
                    .any(|check| !operation.checks.contains(check))
                {
                    return Err(RuntimeManifestError::MissingReentryProtocol(
                        operation.opcode,
                    ));
                }
            }
        }
        if self
            .helpers
            .entries
            .values()
            .any(|entry| entry.semantic_operation.contains("execute_opcode"))
        {
            return Err(RuntimeManifestError::OpaqueDispatcher("helper manifest"));
        }
        Ok(())
    }

    pub fn operation(&self, opcode_id: u16) -> Option<&OperationContract> {
        self.operations
            .binary_search_by_key(&opcode_id, |operation| operation.opcode_id)
            .ok()
            .map(|index| &self.operations[index])
    }
}

fn requires_runtime_helper(descriptor: &OpcodeDescriptor) -> bool {
    let helper_effects = EffectSet::HEAP_WRITE
        .union(EffectSet::MAY_ALLOCATE)
        .union(EffectSet::MAY_CALL_USER)
        .union(EffectSet::MAY_THROW)
        .union(EffectSet::MAY_SUSPEND)
        .union(EffectSet::REALM_ACCESS)
        .union(EffectSet::SCOPE_ACCESS)
        .union(EffectSet::TRAP)
        .union(EffectSet::INTERRUPTION_CHECK)
        .union(EffectSet::EXCEPTION_STATE_READ);
    descriptor.effects.intersects(helper_effects)
        || matches!(
            descriptor.family,
            OpcodeFamily::Object
                | OpcodeFamily::Function
                | OpcodeFamily::Exception
                | OpcodeFamily::Module
        )
}

fn checks_for(descriptor: &OpcodeDescriptor) -> BTreeSet<VisibleCheck> {
    let mut checks = BTreeSet::from([VisibleCheck::DecodeDomain]);
    if descriptor
        .operands
        .iter()
        .any(|operand| operand.value_kind == ImportedValueKind::Signed)
    {
        checks.insert(VisibleCheck::ValueTag);
    }
    if descriptor.family == OpcodeFamily::Numeric {
        checks.insert(VisibleCheck::NumericCoercion);
    }
    if descriptor.family == OpcodeFamily::Object {
        checks.extend([
            VisibleCheck::Bounds,
            VisibleCheck::Shape,
            VisibleCheck::Prototype,
            VisibleCheck::ProxyOrAccessor,
        ]);
    }
    if descriptor.family == OpcodeFamily::Function {
        checks.extend([VisibleCheck::Callability, VisibleCheck::Constructibility]);
    }
    if descriptor.effects.contains(EffectSet::REALM_ACCESS) {
        checks.insert(VisibleCheck::Realm);
    }
    if descriptor.effects.contains(EffectSet::SCOPE_ACCESS) {
        checks.insert(VisibleCheck::Scope);
    }
    if descriptor.effects.contains(EffectSet::EXCEPTION_STATE_READ) {
        checks.insert(VisibleCheck::ExceptionState);
    }
    if descriptor.effects.contains(EffectSet::INTERRUPTION_CHECK) {
        checks.insert(VisibleCheck::Interruption);
    }
    if descriptor.effects.contains(EffectSet::MAY_ALLOCATE) {
        checks.insert(VisibleCheck::AllocationSafepoint);
    }
    if descriptor.effects.contains(EffectSet::MAY_THROW) {
        checks.insert(VisibleCheck::AbruptCompletion);
    }
    if descriptor.effects.contains(EffectSet::MAY_SUSPEND) {
        checks.insert(VisibleCheck::Suspension);
    }
    if descriptor.effects.contains(EffectSet::TRAP) {
        checks.insert(VisibleCheck::Trap);
    }
    if descriptor.effects.contains(EffectSet::MAY_CALL_USER) {
        checks.extend([
            VisibleCheck::PublishFrameAndRoots,
            VisibleCheck::PublishAmbientState,
            VisibleCheck::RestoreAmbientState,
            VisibleCheck::RevalidateAfterReentry,
        ]);
    }
    checks
}

fn helper_for(descriptor: &OpcodeDescriptor, capability: RuntimeCapabilityId) -> HelperEntry {
    let mut parameters = vec![HelperParameter {
        name: "frame".into(),
        abi_type: AbiType::RuntimeServiceHandle,
        ownership: AbiOwnership::Borrowed,
        retained_after_return: false,
        retention_owner: None,
    }];
    parameters.extend(
        descriptor
            .operands
            .iter()
            .filter(|operand| helper_parameter(operand))
            .map(|operand| HelperParameter {
                name: operand.name.into(),
                abi_type: operand_abi(operand),
                ownership: AbiOwnership::Copied,
                retained_after_return: false,
                retention_owner: None,
            }),
    );

    let mut abrupt_results = BTreeSet::new();
    if descriptor.effects.contains(EffectSet::MAY_THROW) {
        abrupt_results.insert("throw".into());
    }
    if descriptor.effects.contains(EffectSet::MAY_SUSPEND) {
        abrupt_results.insert("suspend".into());
        abrupt_results.insert("cancel".into());
    }
    if descriptor.effects.contains(EffectSet::TRAP) {
        abrupt_results.insert("terminate".into());
    }

    let mut reads_ambient_state = BTreeSet::new();
    let mut writes_ambient_state = BTreeSet::new();
    if descriptor.effects.intersects(
        EffectSet::REALM_ACCESS
            .union(EffectSet::SCOPE_ACCESS)
            .union(EffectSet::MAY_CALL_USER),
    ) {
        reads_ambient_state.extend([
            "realm".into(),
            "worker".into(),
            "async_context".into(),
            "effect_current_fiber".into(),
        ]);
    }
    if descriptor.effects.contains(EffectSet::MAY_CALL_USER) {
        writes_ambient_state = reads_ambient_state.clone();
    }

    let mut optimizer_attributes = BTreeSet::from(["nounwind".into()]);
    if !descriptor
        .effects
        .intersects(EffectSet::MAY_SUSPEND.union(EffectSet::TRAP))
    {
        optimizer_attributes.insert("willreturn".into());
    }

    HelperEntry {
        capability,
        semantic_operation: format!("tier1.{}", descriptor.opcode).into(),
        symbol: format!("Bun__Hare__tier1__{}", descriptor.opcode).into(),
        calling_convention: CallingConvention::C,
        parameters,
        result: AbiType::TaggedCompletion,
        normal_result: true,
        abrupt_results,
        effects: descriptor.effects,
        safepoint: descriptor.effects.contains(EffectSet::SAFEPOINT),
        reads_ambient_state,
        writes_ambient_state,
        optimizer_attributes,
    }
}

fn helper_parameter(operand: &OperandDescriptor) -> bool {
    !matches!(operand.role, OperandRole::ValueDefinition)
}

fn operand_abi(operand: &OperandDescriptor) -> AbiType {
    match operand.role {
        OperandRole::ValueUse
        | OperandRole::ValueUseDefinition
        | OperandRole::RegisterRangeUse
        | OperandRole::ConstantOrRegister => AbiType::TaggedValue,
        OperandRole::BooleanControl => AbiType::I1,
        _ => match operand.value_kind {
            ImportedValueKind::Signed | ImportedValueKind::Unsigned => AbiType::I32,
            ImportedValueKind::Boolean => AbiType::I1,
        },
    }
}

pub fn helper_symbols(manifest: &RuntimeManifest) -> BTreeMap<RuntimeCapabilityId, &str> {
    manifest
        .helpers
        .entries
        .iter()
        .map(|(capability, entry)| (*capability, entry.symbol.as_ref()))
        .collect()
}
