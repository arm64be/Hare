use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const HARE_IR_SCHEMA_VERSION: u32 = 1;
pub const PINNED_BUN_REVISION: &str = "bbe3f6a2629adf808adbd0da199ae8c94a3c0d47";
pub const PINNED_WEBKIT_REVISION: &str = "34c01d13391e00c06862a3d2c5b7fff350ac87e0";

macro_rules! dense_id {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
        pub struct $name(pub u32);

        impl $name {
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

dense_id!(SourceId);
dense_id!(FunctionId);
dense_id!(BasicBlockId);
dense_id!(ValueId);
dense_id!(RealmId);
dense_id!(EnvironmentId);
dense_id!(RuntimeCapabilityId);
dense_id!(AmbientStateKey);
dense_id!(FrameLayoutId);
dense_id!(RootLayoutId);
dense_id!(ContinuationId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputKind {
    ModuleProgram,
    ClassicProgram,
    DirectEval,
    FunctionConstructor,
    NestedFunctionCall,
    NestedFunctionConstruct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionRelation {
    Root,
    Declaration { index: u32 },
    Expression { index: u32 },
    FunctionConstructorSpecialization { order: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionSpecialization {
    Program,
    Module,
    Eval,
    Call,
    Construct,
    FunctionConstructorCall,
    FunctionConstructorConstruct,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceText {
    Latin1(Box<[u8]>),
    Utf16(Box<[u16]>),
}

impl SourceText {
    pub fn code_unit_len(&self) -> usize {
        match self {
            Self::Latin1(value) => value.len(),
            Self::Utf16(value) => value.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRecord {
    pub id: SourceId,
    pub public_name: Box<str>,
    pub text: SourceText,
    pub start_line: u32,
    pub start_column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub source: SourceId,
    pub start_code_unit: u32,
    pub end_code_unit: u32,
    pub line: u32,
    pub column: u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperandRole {
    ValueUse,
    ValueDefinition,
    ValueUseDefinition,
    RegisterRangeUse,
    ConstantOrRegister,
    ControlTarget,
    ArgumentCount,
    ArgumentIndex,
    ArgumentRangeBase,
    IdentifierIndex,
    FunctionIndex,
    SwitchTableIndex,
    BitVectorIndex,
    FrameSlotBase,
    ElementOrFieldIndex,
    LexicalFeatureFlags,
    PropertyAttributes,
    StructureFlags,
    ScopeDepth,
    ScopeSlotIndex,
    SymbolTableOrScopeDepth,
    ResumePoint,
    ModeOrFlags,
    BooleanControl,
    Count,
    CacheOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperandValue {
    Signed(i64),
    Unsigned(u64),
    Boolean(bool),
    Bytes(Box<[u8]>),
    Utf16(Box<[u16]>),
    StableId(u32),
    ValidatedAbsent,
    Excluded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisitorOperand {
    pub manifest_id: Box<str>,
    pub role: OperandRole,
    pub value: OperandValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisitorInstruction {
    pub byte_offset: u32,
    pub opcode_id: u32,
    pub encoded_size: u32,
    pub opcode_id_bytes: u32,
    pub width_bytes: u32,
    pub operands: Vec<VisitorOperand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisitorFunction {
    pub id: FunctionId,
    pub parent: Option<FunctionId>,
    pub relation: FunctionRelation,
    pub specialization: FunctionSpecialization,
    pub source: SourceId,
    pub parse_mode: u32,
    pub script_mode: u32,
    pub code_type: u32,
    pub lexical_features: u32,
    pub code_features: u32,
    pub num_parameters: u32,
    pub num_vars: u32,
    pub num_callee_locals: u32,
    pub this_register: i32,
    pub scope_register: i32,
    pub instruction_bytes: u32,
    pub instructions: Vec<VisitorInstruction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageState {
    Mapped,
    ConditionallyMapped,
    Excluded,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedVisitorUnit {
    pub schema_version: u32,
    pub bun_revision: Box<str>,
    pub webkit_revision: Box<str>,
    pub input_kind: InputKind,
    pub sources: Vec<SourceRecord>,
    pub functions: Vec<VisitorFunction>,
    pub definition_coverage: BTreeMap<Box<str>, CoverageState>,
    pub structurally_complete: bool,
}

impl OwnedVisitorUnit {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != HARE_IR_SCHEMA_VERSION {
            return Err(ValidationError::SchemaVersion {
                expected: HARE_IR_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.bun_revision.as_ref() != PINNED_BUN_REVISION
            || self.webkit_revision.as_ref() != PINNED_WEBKIT_REVISION
        {
            return Err(ValidationError::RevisionMismatch);
        }
        if self.sources.is_empty() {
            return Err(ValidationError::MissingSource);
        }
        if self.functions.is_empty() {
            return Err(ValidationError::MissingRootFunction);
        }

        for (index, source) in self.sources.iter().enumerate() {
            if source.id.index() != index {
                return Err(ValidationError::NonDenseSourceId(source.id));
            }
        }

        for (index, function) in self.functions.iter().enumerate() {
            if function.id.index() != index {
                return Err(ValidationError::NonDenseFunctionId(function.id));
            }
            if function.source.index() >= self.sources.len() {
                return Err(ValidationError::UnknownSource(function.source));
            }
            if index == 0 {
                let valid_root_relation = matches!(
                    function.relation,
                    FunctionRelation::Root
                        | FunctionRelation::FunctionConstructorSpecialization { .. }
                );
                if function.parent.is_some() || !valid_root_relation {
                    return Err(ValidationError::InvalidRootRelation);
                }
            } else if matches!(
                function.relation,
                FunctionRelation::FunctionConstructorSpecialization { .. }
            ) && function.parent.is_none()
            {
                // A Function constructor may have multiple independently
                // reachable call/construct specialization roots.
            } else {
                let parent = function
                    .parent
                    .ok_or(ValidationError::MissingFunctionParent(function.id))?;
                if parent.index() >= index {
                    return Err(ValidationError::InvalidFunctionParent {
                        function: function.id,
                        parent,
                    });
                }
            }

            let mut expected_offset = 0_u32;
            for instruction in &function.instructions {
                if instruction.encoded_size == 0 || instruction.byte_offset != expected_offset {
                    return Err(ValidationError::MalformedInstructionStream {
                        function: function.id,
                        offset: instruction.byte_offset,
                    });
                }
                if !matches!(instruction.width_bytes, 1 | 2 | 4)
                    || !matches!(instruction.opcode_id_bytes, 1 | 2 | 4)
                {
                    return Err(ValidationError::InvalidInstructionWidth {
                        function: function.id,
                        offset: instruction.byte_offset,
                    });
                }
                let mut operand_ids = BTreeSet::new();
                for operand in &instruction.operands {
                    if !operand_ids.insert(operand.manifest_id.as_ref()) {
                        return Err(ValidationError::DuplicateOperandManifest {
                            function: function.id,
                            offset: instruction.byte_offset,
                        });
                    }
                    if operand.role == OperandRole::CacheOnly
                        || operand.value == OperandValue::Excluded
                    {
                        return Err(ValidationError::CacheOperandCrossedBoundary {
                            function: function.id,
                            offset: instruction.byte_offset,
                        });
                    }
                }
                expected_offset = expected_offset
                    .checked_add(instruction.encoded_size)
                    .ok_or(ValidationError::InstructionOffsetOverflow(function.id))?;
            }
            if expected_offset != function.instruction_bytes {
                return Err(ValidationError::InstructionStreamLength {
                    function: function.id,
                    expected: function.instruction_bytes,
                    actual: expected_offset,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParserDiagnosticRole {
    BuildDiagnostic,
    RuntimeSyntaxErrorRecipe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxErrorRecipe {
    pub role: ParserDiagnosticRole,
    pub parser_category: u32,
    pub message: SourceText,
    pub source: SourceId,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentKind {
    Global,
    Lexical,
    Variable,
    PrivateName,
    Function,
    Module,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootOwnership {
    Frame,
    Realm,
    Worker,
    Continuation,
    ExplicitHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentHandle {
    pub id: EnvironmentId,
    pub realm: RealmId,
    pub kind: EnvironmentKind,
    pub storage_id: u64,
    pub mutable: bool,
    pub owner: RootOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingState {
    Absent,
    Present(ValueId),
    Identity(EnvironmentId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionDestination {
    Discard,
    Value(ValueId),
    CallerCompletion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectEvalContext {
    pub active_realm: RealmId,
    pub global_environment: EnvironmentId,
    pub lexical_environment: EnvironmentId,
    pub variable_environment: EnvironmentId,
    pub private_name_environment: Option<EnvironmentId>,
    pub caller_strict: bool,
    pub source_strict: bool,
    pub this_binding: BindingState,
    pub new_target: BindingState,
    pub super_binding: BindingState,
    pub home_object: BindingState,
    pub derived_constructor: bool,
    pub caller_script_mode: u32,
    pub caller_function_parse_mode: u32,
    pub class_context: bool,
    pub class_field_initializer: bool,
    pub private_brand_required: bool,
    pub annex_b: bool,
    pub completion: CompletionDestination,
    pub captured_bindings: Vec<EnvironmentHandle>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionConstructionMode {
    Call,
    Construct,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionConstructorContext {
    pub argument_values_in_tostring_order: Vec<ValueId>,
    pub caller_realm: RealmId,
    pub constructor_realm: RealmId,
    pub callee_global_environment: EnvironmentId,
    pub host_policy_operation: ValueId,
    pub mode: FunctionConstructionMode,
    pub constructor: ValueId,
    pub new_target: BindingState,
    pub prototype_lookup_operation: ValueId,
    pub function_kind: u32,
    pub constructible: bool,
    pub parameters_end_code_unit: u32,
    pub source: SourceId,
    pub allocation_owner: RootOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Representation {
    Never,
    Undefined,
    Boolean,
    Int32,
    UInt32,
    Float64Bits,
    TaggedValue,
    ManagedReference,
    NativePointer,
    EnvironmentHandle,
    RealmHandle,
    Completion,
    Continuation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Constant {
    Undefined,
    Null,
    Boolean(bool),
    Int32(i32),
    Float64Bits(u64),
    String(SourceText),
    BigInt {
        negative: bool,
        magnitude_be: Box<[u8]>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbruptKind {
    Throw,
    Cancel,
    Suspend,
    Yield,
    Terminate,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet(u64);

impl EffectSet {
    pub const NONE: Self = Self(0);
    pub const HEAP_READ: Self = Self(1 << 0);
    pub const HEAP_WRITE: Self = Self(1 << 1);
    pub const MAY_ALLOCATE: Self = Self(1 << 2);
    pub const MAY_THROW: Self = Self(1 << 3);
    pub const MAY_CALL_USER: Self = Self(1 << 4);
    pub const MAY_SUSPEND: Self = Self(1 << 5);
    pub const MAY_SCHEDULE: Self = Self(1 << 6);
    pub const MAY_SYNCHRONIZE: Self = Self(1 << 7);
    pub const SAFEPOINT: Self = Self(1 << 8);
    pub const READS_AMBIENT: Self = Self(1 << 9);
    pub const WRITES_AMBIENT: Self = Self(1 << 10);
    pub const OWNERSHIP_TRANSITION: Self = Self(1 << 11);
    pub const READS_FRAME: Self = Self(1 << 12);
    pub const WRITES_FRAME: Self = Self(1 << 13);
    pub const REALM_ACCESS: Self = Self(1 << 14);
    pub const SCOPE_ACCESS: Self = Self(1 << 15);
    pub const CONTROL_FLOW: Self = Self(1 << 16);
    pub const TRAP: Self = Self(1 << 17);
    pub const INTERRUPTION_CHECK: Self = Self(1 << 18);
    pub const EXCEPTION_STATE_READ: Self = Self(1 << 19);

    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn bits(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueDefinition {
    pub id: ValueId,
    pub representation: Representation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Constant(Constant),
    ImportedOpcode {
        manifest_id: Box<str>,
        opcode_id: u32,
        operands: Vec<VisitorOperand>,
    },
    RuntimeCapability {
        capability: RuntimeCapabilityId,
        arguments: Vec<ValueId>,
    },
    PublishNativeReentry {
        continuation: ContinuationId,
        frame_layout: FrameLayoutId,
        root_layout: RootLayoutId,
        live_roots: Vec<ValueId>,
    },
    RestoreNativeReentry {
        continuation: ContinuationId,
    },
    AmbientRead {
        key: AmbientStateKey,
    },
    AmbientWrite {
        key: AmbientStateKey,
        value: ValueId,
    },
    Root(ValueId),
    Unroot(ValueId),
    Pin(ValueId),
    Unpin(ValueId),
    Move(ValueId),
    Copy(ValueId),
    Barrier {
        owner: ValueId,
        value: ValueId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    pub results: Vec<ValueId>,
    pub kind: OperationKind,
    pub effects: EffectSet,
    pub source: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Return(Option<ValueId>),
    Jump {
        target: BasicBlockId,
        arguments: Vec<ValueId>,
    },
    Branch {
        condition: ValueId,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    Invoke {
        operation: ValueId,
        normal: BasicBlockId,
        abrupt: BTreeMap<AbruptKind, BasicBlockId>,
    },
    Throw(ValueId),
    Suspend {
        token: ValueId,
        resume: BasicBlockId,
        cancel: BasicBlockId,
    },
    Terminate,
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub parameters: Vec<ValueId>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub source: SourceId,
    pub specialization: FunctionSpecialization,
    pub parameters: Vec<ValueDefinition>,
    pub values: Vec<ValueDefinition>,
    pub blocks: Vec<BasicBlock>,
    pub direct_eval_context: Option<DirectEvalContext>,
    pub function_constructor_context: Option<FunctionConstructorContext>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCapability {
    pub id: RuntimeCapabilityId,
    pub semantic_name: Box<str>,
    pub effects: EffectSet,
    pub safepoint: bool,
    pub retains_inputs: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationUnit {
    pub schema_version: u32,
    pub sources: Vec<SourceRecord>,
    pub functions: Vec<Function>,
    pub entry_points: Vec<FunctionId>,
    pub capabilities: Vec<RuntimeCapability>,
}

impl CompilationUnit {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != HARE_IR_SCHEMA_VERSION {
            return Err(ValidationError::SchemaVersion {
                expected: HARE_IR_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        for (index, function) in self.functions.iter().enumerate() {
            if function.id.index() != index {
                return Err(ValidationError::NonDenseFunctionId(function.id));
            }
            if function.source.index() >= self.sources.len() {
                return Err(ValidationError::UnknownSource(function.source));
            }
            validate_function(function)?;
        }
        for entry in &self.entry_points {
            if entry.index() >= self.functions.len() {
                return Err(ValidationError::UnknownEntryPoint(*entry));
            }
        }
        Ok(())
    }
}

fn validate_function(function: &Function) -> Result<(), ValidationError> {
    let value_ids = function
        .parameters
        .iter()
        .chain(&function.values)
        .map(|value| value.id)
        .collect::<BTreeSet<_>>();
    if value_ids.len() != function.parameters.len() + function.values.len() {
        return Err(ValidationError::DuplicateValue(function.id));
    }
    for (index, block) in function.blocks.iter().enumerate() {
        if block.id.index() != index {
            return Err(ValidationError::NonDenseBlockId {
                function: function.id,
                block: block.id,
            });
        }
        for parameter in &block.parameters {
            if !value_ids.contains(parameter) {
                return Err(ValidationError::UnknownValue {
                    function: function.id,
                    value: *parameter,
                });
            }
        }
        for operation in &block.operations {
            for result in &operation.results {
                if !value_ids.contains(result) {
                    return Err(ValidationError::UnknownValue {
                        function: function.id,
                        value: *result,
                    });
                }
            }
            if operation.effects.contains(EffectSet::MAY_CALL_USER)
                && !operation.effects.contains(EffectSet::SAFEPOINT)
            {
                return Err(ValidationError::ReentryWithoutSafepoint(function.id));
            }
        }
        validate_terminator(
            function.id,
            function.blocks.len(),
            &value_ids,
            &block.terminator,
        )?;
    }
    Ok(())
}

fn validate_terminator(
    function: FunctionId,
    block_count: usize,
    values: &BTreeSet<ValueId>,
    terminator: &Terminator,
) -> Result<(), ValidationError> {
    let valid_block = |block: &BasicBlockId| block.index() < block_count;
    let valid_value = |value: &ValueId| values.contains(value);
    let ok = match terminator {
        Terminator::Return(value) => value.as_ref().is_none_or(valid_value),
        Terminator::Jump { target, arguments } => {
            valid_block(target) && arguments.iter().all(valid_value)
        }
        Terminator::Branch {
            condition,
            then_target,
            else_target,
        } => valid_value(condition) && valid_block(then_target) && valid_block(else_target),
        Terminator::Invoke {
            operation,
            normal,
            abrupt,
        } => {
            valid_value(operation)
                && valid_block(normal)
                && abrupt.values().all(valid_block)
                && !abrupt.is_empty()
        }
        Terminator::Throw(value) => valid_value(value),
        Terminator::Suspend {
            token,
            resume,
            cancel,
        } => valid_value(token) && valid_block(resume) && valid_block(cancel),
        Terminator::Terminate | Terminator::Unreachable => true,
    };
    if ok {
        Ok(())
    } else {
        Err(ValidationError::InvalidTerminator(function))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportError {
    DuplicateFunction(FunctionId),
    FunctionOutOfOrder(FunctionId),
    InstructionWithoutFunction,
    InstructionOffset { expected: u32, actual: u32 },
    InvalidInstructionWidth(u32),
    OperandWithoutInstruction,
    DuplicateOperandManifest(Box<str>),
    CacheOperandCrossedBoundary(Box<str>),
    IntegerOverflow,
    VisitorRejected(Box<str>),
    Validation(ValidationError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ImportError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParserDiagnostic {
    pub error_type: u32,
    pub syntax_error_type: u32,
    pub line: i32,
    pub column: u32,
    pub message: SourceText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HareImportError {
    UnsupportedFormat,
    Parser(ParserDiagnostic),
    MissingParserDiagnostic,
    NullRoot,
    Visitor(ImportError),
    InternalCxxException,
    InvalidSpecializationPlan,
    NativeProbeFailed,
    PendingJscException,
    InternalBridge(u32),
}

impl fmt::Display for HareImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for HareImportError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    SchemaVersion {
        expected: u32,
        actual: u32,
    },
    RevisionMismatch,
    MissingSource,
    MissingRootFunction,
    NonDenseSourceId(SourceId),
    NonDenseFunctionId(FunctionId),
    UnknownSource(SourceId),
    InvalidRootRelation,
    MissingFunctionParent(FunctionId),
    InvalidFunctionParent {
        function: FunctionId,
        parent: FunctionId,
    },
    MalformedInstructionStream {
        function: FunctionId,
        offset: u32,
    },
    InvalidInstructionWidth {
        function: FunctionId,
        offset: u32,
    },
    InstructionOffsetOverflow(FunctionId),
    InstructionStreamLength {
        function: FunctionId,
        expected: u32,
        actual: u32,
    },
    DuplicateOperandManifest {
        function: FunctionId,
        offset: u32,
    },
    CacheOperandCrossedBoundary {
        function: FunctionId,
        offset: u32,
    },
    UnknownEntryPoint(FunctionId),
    NonDenseBlockId {
        function: FunctionId,
        block: BasicBlockId,
    },
    DuplicateValue(FunctionId),
    UnknownValue {
        function: FunctionId,
        value: ValueId,
    },
    ReentryWithoutSafepoint(FunctionId),
    InvalidTerminator(FunctionId),
    StructuralCoverageIncomplete,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ValidationError {}
