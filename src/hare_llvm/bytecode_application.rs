use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::rc::Rc;

use hare_ir::{
    FIRST_CONSTANT_REGISTER_INDEX, FunctionId, FunctionRelation, FunctionSpecialization,
    OperandValue, OwnedVisitorUnit, SourceText, VisitorConstantValue, VisitorFunction,
    VisitorInstruction,
};

use crate::{LlvmError, TargetLayout};

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScalarExpression {
    Integer(i64),
    Parameter(usize),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    Remainder(Box<Self>, Box<Self>),
    Power(Box<Self>, Box<Self>),
    BitAnd(Box<Self>, Box<Self>),
    BitOr(Box<Self>, Box<Self>),
    BitXor(Box<Self>, Box<Self>),
    LeftShift(Box<Self>, Box<Self>),
    RightShift(Box<Self>, Box<Self>),
    UnsignedRightShift(Box<Self>, Box<Self>),
    Negate(Box<Self>),
    BitNot(Box<Self>),
    Unsigned(Box<Self>),
    Equal(Box<Self>, Box<Self>),
    NotEqual(Box<Self>, Box<Self>),
    Less(Box<Self>, Box<Self>),
    LessEqual(Box<Self>, Box<Self>),
    Greater(Box<Self>, Box<Self>),
    GreaterEqual(Box<Self>, Box<Self>),
    Below(Box<Self>, Box<Self>),
    BelowEqual(Box<Self>, Box<Self>),
    LogicalNot(Box<Self>),
    Call {
        function: FunctionId,
        arguments: Vec<Self>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarKind {
    Integer,
    Boolean,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Builtin {
    Array,
    CreatePrivateSymbol,
    EmptyPropertyNameEnumerator,
    Eval,
    HasOwnPropertyFunction,
    Object,
    SentinelString,
    SetPrototypeDirectOrThrow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InternalObjectKind {
    Generator,
    AsyncGenerator,
    AsyncFunctionGenerator,
    Promise,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArgumentsKind {
    Direct,
    Cloned,
}

fn builtin_is_callable(builtin: Builtin) -> bool {
    matches!(
        builtin,
        Builtin::Array
            | Builtin::CreatePrivateSymbol
            | Builtin::Eval
            | Builtin::HasOwnPropertyFunction
            | Builtin::Object
            | Builtin::SetPrototypeDirectOrThrow
    )
}

fn builtin_is_constructor(builtin: Builtin) -> bool {
    matches!(builtin, Builtin::Array | Builtin::Object)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScalarFunction {
    id: FunctionId,
    parameter_count: usize,
    result_kind: ScalarKind,
    result: ScalarExpression,
}

type StaticEnvironmentRef = Rc<RefCell<StaticEnvironment>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticEnvironmentKind {
    Var,
    Lexical,
    With,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticEnvironment {
    kind: StaticEnvironmentKind,
    parent: Option<StaticEnvironmentRef>,
    object_scope: Option<RegisterValue>,
    bindings: BTreeMap<Box<str>, RegisterValue>,
    scoped_argument_values: BTreeMap<usize, RegisterValue>,
    scoped_argument_names: BTreeMap<Box<str>, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RegisterValue {
    Scalar(ScalarExpression),
    BigInt {
        negative: bool,
        magnitude_be: Box<[u8]>,
    },
    String(Box<str>),
    Concatenation(Vec<Self>),
    BooleanScalar(ScalarExpression),
    Function {
        call: FunctionId,
        construct: Option<FunctionId>,
        environment: Option<StaticEnvironmentRef>,
        identity: u32,
        heap_id: u32,
    },
    RegExpTemplate {
        pattern: SourceText,
        flags: u32,
    },
    RegExp {
        pattern: SourceText,
        flags: u32,
        identity: u32,
    },
    PrivateName {
        identity: u32,
        description: Box<str>,
    },
    AccessorGetter(Box<Self>),
    ArrayTemplate(Vec<Self>),
    Environment(StaticEnvironmentRef),
    GlobalObject,
    ConsoleScope,
    NaNScope,
    InfinityScope,
    UndefinedScope,
    ConsoleObject,
    ConsoleLog,
    ConsoleNoArgument,
    BuiltinScope(Builtin),
    Builtin(Builtin),
    Enumerator {
        heap_id: u32,
        keys: Vec<Box<str>>,
    },
    Arguments {
        kind: ArgumentsKind,
        values: Rc<RefCell<Vec<Self>>>,
    },
    ScopedArguments {
        environment: StaticEnvironmentRef,
        length: Rc<Cell<usize>>,
    },
    ExceptionObject(Box<Self>),
    Error {
        kind: u32,
        message: Box<str>,
    },
    Object(u32),
    Array(u32),
    ArrayIteratorMethod,
    ArrayIteratorNext,
    ArrayIterator {
        array_id: u32,
        next_index: Rc<Cell<u32>>,
    },
    AsyncFromSyncIteratorNext,
    AsyncFromSyncIterator {
        array_id: u32,
        next_index: Rc<Cell<u32>>,
    },
    FulfilledPromise(Box<Self>),
    InternalObject {
        identity: u32,
        kind: InternalObjectKind,
        prototype: Option<Box<Self>>,
        fields: Rc<RefCell<BTreeMap<u32, Self>>>,
    },
    Spread(Vec<Self>),
    Empty,
    Undefined,
    Null,
    Boolean(bool),
    NaN,
    NegativeZero,
    PositiveInfinity,
    NegativeInfinity,
    Opaque,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StaticHeapEntry {
    Object {
        prototype: Option<u32>,
        properties: BTreeMap<Box<str>, StaticProperty>,
        property_order: Vec<Box<str>>,
        private_properties: BTreeMap<u32, RegisterValue>,
        private_brands: BTreeSet<u32>,
    },
    Array {
        prototype: Option<u32>,
        elements: BTreeMap<u32, StaticProperty>,
        properties: BTreeMap<Box<str>, StaticProperty>,
        property_order: Vec<Box<str>>,
        length: u32,
        private_properties: BTreeMap<u32, RegisterValue>,
        private_brands: BTreeSet<u32>,
    },
    Function {
        prototype: Option<RegisterValue>,
        properties: BTreeMap<Box<str>, StaticProperty>,
        property_order: Vec<Box<str>>,
        private_properties: BTreeMap<u32, RegisterValue>,
        private_brands: BTreeSet<u32>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticProperty {
    value: RegisterValue,
    getter: Option<RegisterValue>,
    setter: Option<RegisterValue>,
    is_accessor: bool,
    writable: bool,
    enumerable: bool,
    configurable: bool,
}

impl StaticProperty {
    fn assigned(value: RegisterValue) -> Self {
        Self {
            value,
            getter: None,
            setter: None,
            is_accessor: false,
            writable: true,
            enumerable: true,
            configurable: true,
        }
    }

    fn accessor(
        getter: Option<RegisterValue>,
        setter: Option<RegisterValue>,
        enumerable: bool,
        configurable: bool,
    ) -> Self {
        Self {
            value: RegisterValue::Undefined,
            getter,
            setter,
            is_accessor: true,
            writable: false,
            enumerable,
            configurable,
        }
    }

    fn read(&self) -> RegisterValue {
        if !self.is_accessor {
            return self.value.clone();
        }
        self.getter
            .clone()
            .map(|getter| RegisterValue::AccessorGetter(Box::new(getter)))
            .unwrap_or(RegisterValue::Undefined)
    }
}

#[derive(Default)]
struct StaticExecutionState {
    heap: BTreeMap<u32, StaticHeapEntry>,
    next_heap_id: u32,
    next_function_identity: u32,
    next_internal_object_identity: u32,
    next_private_name_identity: u32,
}

impl StaticExecutionState {
    fn allocate_heap_id(&mut self) -> Result<u32, LlvmError> {
        let id = self.next_heap_id;
        self.next_heap_id = self
            .next_heap_id
            .checked_add(1)
            .ok_or_else(|| imported_error("static heap id overflow"))?;
        Ok(id)
    }

    fn allocate_function_identity(&mut self) -> Result<u32, LlvmError> {
        let identity = self.next_function_identity;
        self.next_function_identity = self
            .next_function_identity
            .checked_add(1)
            .ok_or_else(|| imported_error("static function identity overflow"))?;
        Ok(identity)
    }

    fn allocate_private_name_identity(&mut self) -> Result<u32, LlvmError> {
        let identity = self.next_private_name_identity;
        self.next_private_name_identity = self
            .next_private_name_identity
            .checked_add(1)
            .ok_or_else(|| imported_error("private-name identity overflow"))?;
        Ok(identity)
    }

    fn allocate_internal_object_identity(&mut self) -> Result<u32, LlvmError> {
        let identity = self.next_internal_object_identity;
        self.next_internal_object_identity = self
            .next_internal_object_identity
            .checked_add(1)
            .ok_or_else(|| imported_error("internal-object identity overflow"))?;
        Ok(identity)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StaticPropertyKey {
    Index(u32),
    Name(Box<str>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticDefinePropertyAttributes {
    configurable: Option<bool>,
    enumerable: Option<bool>,
    writable: Option<bool>,
    has_value: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticDefineAccessorAttributes {
    configurable: Option<bool>,
    enumerable: Option<bool>,
    has_get: bool,
    has_set: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoweredBody {
    result: Option<RegisterValue>,
    writes: Vec<RegisterValue>,
    abrupt: Option<RegisterValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NativeWrite {
    Integer(ScalarExpression),
    Boolean(ScalarExpression),
    Text(Box<[u8]>),
}

/// Compile the first executable Tier 1 slice directly from the owned JSC
/// visitor unit. This slice is deliberately fail-closed: it accepts a closed
/// primitive call graph and admitted `console.log` writes only after proving
/// every reachable numeric call inside the exact safe-integer domain.
/// Unsupported visitor operations never fall through to runtime JSC.
pub fn compile_imported_scalar_application(
    unit: &OwnedVisitorUnit,
    target: TargetLayout,
) -> Result<String, LlvmError> {
    if !unit.structurally_complete {
        return Err(imported_error(
            "owned visitor unit is structurally incomplete",
        ));
    }
    unit.validate()
        .map_err(|error| imported_error(format!("invalid owned visitor unit: {error}")))?;
    hare_frontend::validate_imported_unit(unit)
        .map_err(|error| imported_error(format!("invalid frontend instruction: {error}")))?;
    let root = unit
        .functions
        .first()
        .ok_or_else(|| imported_error("owned visitor unit has no root function"))?;
    let call_arities = infer_static_call_arities(unit)?;

    let mut functions = BTreeMap::new();
    for function in unit.functions.iter().skip(1) {
        let mut state = StaticExecutionState::default();
        let Ok(body) = lower_function(
            unit,
            function,
            &functions,
            &call_arities,
            None,
            None,
            None,
            None,
            &mut state,
            0,
        ) else {
            continue;
        };
        if !body.writes.is_empty() {
            continue;
        }
        if body.abrupt.is_some() {
            continue;
        }
        let Some(result) = body.result else {
            continue;
        };
        let (result_kind, result) = match result {
            RegisterValue::Scalar(result) => (ScalarKind::Integer, result),
            RegisterValue::BooleanScalar(result) => (ScalarKind::Boolean, result),
            RegisterValue::Boolean(result) => (
                ScalarKind::Boolean,
                ScalarExpression::Integer(i64::from(result)),
            ),
            _ => continue,
        };
        functions.insert(
            function.id,
            ScalarFunction {
                id: function.id,
                parameter_count: static_parameter_count(function, &call_arities),
                result_kind,
                result,
            },
        );
    }

    let mut state = StaticExecutionState::default();
    let root_body = lower_function(
        unit,
        root,
        &functions,
        &call_arities,
        None,
        None,
        None,
        None,
        &mut state,
        0,
    )?;
    if root_body.abrupt.is_some() {
        return Err(imported_error("root function completes abruptly"));
    }
    if root_body.writes.is_empty() {
        return Err(imported_error(
            "root function performs no admitted native output",
        ));
    }
    let writes = root_body
        .writes
        .into_iter()
        .map(|value| normalize_write(value, &functions))
        .collect::<Result<Vec<_>, _>>()?;
    emit_application(target, &functions, &writes)
}

fn infer_static_call_arities(
    unit: &OwnedVisitorUnit,
) -> Result<BTreeMap<FunctionId, usize>, LlvmError> {
    let mut arities = BTreeMap::<FunctionId, usize>::new();
    for function in &unit.functions {
        let mut function_registers = BTreeMap::<i64, FunctionId>::new();
        for instruction in &function.instructions {
            let Some(descriptor) = hare_frontend::descriptor(instruction.opcode_id) else {
                continue;
            };
            match descriptor.opcode {
                "op_new_func" => {
                    let child = child_function(
                        unit,
                        function.id,
                        descriptor.opcode,
                        unsigned_operand(instruction, "functionDecl")?,
                        FunctionSpecialization::Call,
                    )?;
                    function_registers.insert(signed_operand(instruction, "dst")?, child);
                }
                "op_new_func_exp" => {
                    let child = child_function(
                        unit,
                        function.id,
                        descriptor.opcode,
                        unsigned_operand(instruction, "functionDecl")?,
                        FunctionSpecialization::Call,
                    )?;
                    function_registers.insert(signed_operand(instruction, "dst")?, child);
                }
                "op_mov" => {
                    let destination = signed_operand(instruction, "dst")?;
                    let source = signed_operand(instruction, "src")?;
                    if let Some(child) = function_registers.get(&source).copied() {
                        function_registers.insert(destination, child);
                    } else {
                        function_registers.remove(&destination);
                    }
                }
                "op_call" | "op_tail_call" => {
                    let callee = signed_operand(instruction, "callee")?;
                    let Some(child) = function_registers.get(&callee).copied() else {
                        continue;
                    };
                    let count = usize::try_from(unsigned_operand(instruction, "argc")?)
                        .map_err(|_| imported_error("call argument count exceeds usize"))?
                        .checked_sub(1)
                        .ok_or_else(|| imported_error("JSC call omits its this argument"))?;
                    arities
                        .entry(child)
                        .and_modify(|arity| *arity = (*arity).max(count))
                        .or_insert(count);
                }
                _ => {}
            }
        }
    }
    Ok(arities)
}

fn static_parameter_count(
    function: &VisitorFunction,
    call_arities: &BTreeMap<FunctionId, usize>,
) -> usize {
    usize::try_from(function.num_parameters.saturating_sub(1))
        .unwrap_or(usize::MAX)
        .max(call_arities.get(&function.id).copied().unwrap_or(0))
}

fn lower_function(
    unit: &OwnedVisitorUnit,
    function: &VisitorFunction,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    call_arities: &BTreeMap<FunctionId, usize>,
    explicit_arguments: Option<&[RegisterValue]>,
    explicit_environment: Option<StaticEnvironmentRef>,
    explicit_this: Option<RegisterValue>,
    explicit_callee: Option<RegisterValue>,
    state: &mut StaticExecutionState,
    call_depth: usize,
) -> Result<LoweredBody, LlvmError> {
    if call_depth > 64 {
        return Err(imported_error("static call depth exceeds 64"));
    }
    let mut registers = BTreeMap::new();
    let first_var = -i64::from(function.num_vars);
    let scope_register = i64::from(function.scope_register);
    if first_var > scope_register {
        return Err(imported_error(format!(
            "f{} has an invalid numVars/scope-register layout",
            function.id.0
        )));
    }
    for register in first_var..scope_register {
        registers.insert(register, RegisterValue::Undefined);
    }
    let environment = explicit_environment.unwrap_or_else(|| {
        Rc::new(RefCell::new(StaticEnvironment {
            kind: StaticEnvironmentKind::Var,
            parent: None,
            object_scope: None,
            bindings: BTreeMap::new(),
            scoped_argument_values: BTreeMap::new(),
            scoped_argument_names: BTreeMap::new(),
        }))
    });
    registers.insert(
        scope_register,
        RegisterValue::Environment(environment.clone()),
    );
    let this_value = explicit_this.unwrap_or(RegisterValue::Undefined);
    registers.insert(function.this_register as i64, this_value.clone());
    registers.insert(
        function.call_frame_this_argument_register as i64,
        this_value,
    );
    let callee_scope = explicit_callee
        .as_ref()
        .and_then(|callee| match callee {
            RegisterValue::Function { environment, .. } => environment.clone(),
            _ => None,
        })
        .unwrap_or_else(|| environment.clone());
    if let Some(callee) = explicit_callee {
        registers.insert(function.call_frame_callee_register as i64, callee);
    }
    let parameter_count = explicit_arguments.map_or_else(
        || static_parameter_count(function, call_arities),
        |arguments| {
            usize::try_from(function.num_parameters.saturating_sub(1))
                .unwrap_or(usize::MAX)
                .max(arguments.len())
        },
    );
    let parameter_values = (0..parameter_count)
        .map(|index| {
            explicit_arguments
                .and_then(|arguments| arguments.get(index).cloned())
                .unwrap_or_else(|| {
                    if explicit_arguments.is_some() {
                        RegisterValue::Undefined
                    } else {
                        RegisterValue::Scalar(ScalarExpression::Parameter(index))
                    }
                })
        })
        .collect::<Vec<_>>();
    for index in 0..parameter_count {
        registers.insert(
            i64::from(function.call_frame_first_argument_register) + index as i64,
            parameter_values[index].clone(),
        );
    }

    let mut writes = Vec::new();
    let mut result = None;
    let mut pending_exception = None;
    let mut abrupt = None;
    let instruction_indices = function
        .instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| (instruction.byte_offset, index))
        .collect::<BTreeMap<_, _>>();
    let mut instruction_index = 0;
    let mut executed_instructions = 0_usize;
    while let Some(instruction) = function.instructions.get(instruction_index) {
        executed_instructions += 1;
        if executed_instructions > function.instructions.len().saturating_mul(4).max(1) {
            return Err(imported_error(format!(
                "f{} requires a runtime control-flow loop",
                function.id.0
            )));
        }
        if hare_frontend::cache_descriptor(instruction.opcode_id).is_some() {
            instruction_index += 1;
            continue;
        }
        let descriptor = hare_frontend::descriptor(instruction.opcode_id)
            .ok_or_else(|| imported_error(format!("unknown opcode {}", instruction.opcode_id)))?;
        match descriptor.opcode {
            "op_enter" => {}
            "op_jmp" => {
                instruction_index = branch_target_index(instruction, &instruction_indices)?;
                continue;
            }
            "op_jtrue" | "op_jfalse" => {
                let condition = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "condition")?,
                )?;
                let truthy = known_truthiness(&condition, functions)?
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                let should_branch = if descriptor.opcode == "op_jtrue" {
                    truthy
                } else {
                    !truthy
                };
                if should_branch {
                    instruction_index = branch_target_index(instruction, &instruction_indices)?;
                    continue;
                }
            }
            "op_jeq" | "op_jstricteq" | "op_jneq" | "op_jnstricteq" | "op_jless" | "op_jlesseq"
            | "op_jgreater" | "op_jgreatereq" | "op_jnless" | "op_jnlesseq" | "op_jngreater"
            | "op_jngreatereq" | "op_jbelow" | "op_jbeloweq" => {
                let left =
                    read_register(function, &registers, signed_operand(instruction, "lhs")?)?;
                let right =
                    read_register(function, &registers, signed_operand(instruction, "rhs")?)?;
                let should_branch =
                    known_branch_comparison(descriptor.opcode, &left, &right, functions)?
                        .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                if should_branch {
                    instruction_index = branch_target_index(instruction, &instruction_indices)?;
                    continue;
                }
            }
            "op_jneq_ptr" | "op_jeq_ptr" => {
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let special_pointer = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "specialPointer")?,
                )?;
                let equal = known_pointer_equality(&value, &special_pointer)
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                let should_branch = if descriptor.opcode == "op_jeq_ptr" {
                    equal
                } else {
                    !equal
                };
                if should_branch {
                    instruction_index = branch_target_index(instruction, &instruction_indices)?;
                    continue;
                }
            }
            "op_switch_imm" | "op_switch_char" | "op_switch_string" => {
                let table_index = usize::try_from(unsigned_operand(instruction, "tableIndex")?)
                    .map_err(|_| imported_error("switch table index does not fit usize"))?;
                let scrutinee = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "scrutinee")?,
                )?;
                let relative_offset = match descriptor.opcode {
                    "op_switch_imm" => {
                        let RegisterValue::Scalar(expression) = scrutinee else {
                            return Err(unsupported(function, instruction, descriptor.opcode));
                        };
                        if !expression_is_closed(&expression) {
                            return Err(unsupported(function, instruction, descriptor.opcode));
                        }
                        let value = evaluate(&expression, functions, &[], 0)?;
                        simple_switch_offset(function, table_index, i32::try_from(value).ok())?
                    }
                    "op_switch_char" => {
                        let value = known_switch_string(&scrutinee, functions)?
                            .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                        let mut code_units = value.encode_utf16();
                        let first = code_units.next();
                        let key = first.filter(|_| code_units.next().is_none()).map(i32::from);
                        simple_switch_offset(function, table_index, key)?
                    }
                    "op_switch_string" => {
                        let value = known_switch_string(&scrutinee, functions)?
                            .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                        string_switch_offset(function, table_index, &value)?
                    }
                    _ => unreachable!(),
                };
                instruction_index =
                    relative_target_index(instruction, relative_offset, &instruction_indices)?;
                continue;
            }
            "op_jeq_null" | "op_jneq_null" | "op_jundefined_or_null" | "op_jnundefined_or_null" => {
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let is_nullish = known_unary_predicate("op_is_undefined_or_null", &value)
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                let should_branch = match descriptor.opcode {
                    "op_jeq_null" | "op_jundefined_or_null" => is_nullish,
                    "op_jneq_null" | "op_jnundefined_or_null" => !is_nullish,
                    _ => unreachable!(),
                };
                if should_branch {
                    instruction_index = branch_target_index(instruction, &instruction_indices)?;
                    continue;
                }
            }
            "op_mov" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = signed_operand(instruction, "src")?;
                let value = read_register(function, &registers, source)?;
                registers.insert(destination, value);
            }
            "op_throw" | "op_throw_static_error" => {
                let thrown = if descriptor.opcode == "op_throw" {
                    read_register(function, &registers, signed_operand(instruction, "value")?)?
                } else {
                    let message = read_register(
                        function,
                        &registers,
                        signed_operand(instruction, "message")?,
                    )?;
                    let RegisterValue::String(message) = message else {
                        return Err(unsupported(function, instruction, descriptor.opcode));
                    };
                    RegisterValue::Error {
                        kind: u32::try_from(unsigned_operand(instruction, "errorType")?)
                            .map_err(|_| imported_error("static error kind exceeds u32"))?,
                        message,
                    }
                };
                if let Some(target) =
                    static_exception_target(function, instruction, &instruction_indices)?
                {
                    pending_exception = Some(thrown);
                    instruction_index = target;
                    continue;
                }
                abrupt = Some(thrown);
                break;
            }
            "op_catch" => {
                let thrown = pending_exception.take().ok_or_else(|| {
                    imported_error(format!(
                        "f{} enters a catch handler without a pending exception",
                        function.id.0
                    ))
                })?;
                registers.insert(
                    signed_operand(instruction, "exception")?,
                    RegisterValue::ExceptionObject(Box::new(thrown.clone())),
                );
                registers.insert(signed_operand(instruction, "thrownValue")?, thrown);
            }
            "op_unreachable" => {
                return Err(imported_error(format!(
                    "f{} reached op_unreachable at byte {}",
                    function.id.0, instruction.byte_offset
                )));
            }
            "op_new_object" => {
                let destination = signed_operand(instruction, "dst")?;
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Object {
                        prototype: None,
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Object(id));
            }
            "op_create_generator"
            | "op_create_async_generator"
            | "op_create_promise"
            | "op_new_generator"
            | "op_new_async_function_generator"
            | "op_new_promise" => {
                let destination = signed_operand(instruction, "dst")?;
                let prototype = if matches!(
                    descriptor.opcode,
                    "op_create_generator" | "op_create_async_generator" | "op_create_promise"
                ) {
                    let callee = read_register(
                        function,
                        &registers,
                        signed_operand(instruction, "callee")?,
                    )?;
                    if !matches!(callee, RegisterValue::Function { .. }) {
                        return Err(unsupported(function, instruction, descriptor.opcode));
                    }
                    let prototype = static_get_property(
                        &state.heap,
                        &callee,
                        StaticPropertyKey::Name("prototype".into()),
                    )?;
                    matches!(
                        prototype,
                        RegisterValue::Object(_)
                            | RegisterValue::Array(_)
                            | RegisterValue::Function { .. }
                    )
                    .then(|| Box::new(prototype))
                } else {
                    None
                };
                let kind = match descriptor.opcode {
                    "op_create_generator" | "op_new_generator" => InternalObjectKind::Generator,
                    "op_create_async_generator" => InternalObjectKind::AsyncGenerator,
                    "op_new_async_function_generator" => InternalObjectKind::AsyncFunctionGenerator,
                    "op_create_promise" | "op_new_promise" => InternalObjectKind::Promise,
                    _ => unreachable!(),
                };
                registers.insert(
                    destination,
                    RegisterValue::InternalObject {
                        identity: state.allocate_internal_object_identity()?,
                        kind,
                        prototype,
                        fields: Rc::new(RefCell::new(BTreeMap::new())),
                    },
                );
            }
            "op_create_this" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee =
                    read_register(function, &registers, signed_operand(instruction, "callee")?)?;
                if !matches!(callee, RegisterValue::Function { .. }) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let prototype = static_get_property(
                    &state.heap,
                    &callee,
                    StaticPropertyKey::Name("prototype".into()),
                )?;
                let prototype = match prototype {
                    RegisterValue::Object(id) => id,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Object {
                        prototype: Some(prototype),
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Object(id));
            }
            "op_new_reg_exp" => {
                let destination = signed_operand(instruction, "dst")?;
                let template =
                    read_register(function, &registers, signed_operand(instruction, "regexp")?)?;
                let RegisterValue::RegExpTemplate { pattern, flags } = template else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let identity = state.allocate_heap_id()?;
                registers.insert(
                    destination,
                    RegisterValue::RegExp {
                        pattern,
                        flags,
                        identity,
                    },
                );
            }
            "op_create_direct_arguments" | "op_create_cloned_arguments" => {
                let destination = signed_operand(instruction, "dst")?;
                let kind = if descriptor.opcode == "op_create_direct_arguments" {
                    ArgumentsKind::Direct
                } else {
                    ArgumentsKind::Cloned
                };
                registers.insert(
                    destination,
                    RegisterValue::Arguments {
                        kind,
                        values: Rc::new(RefCell::new(parameter_values.clone())),
                    },
                );
            }
            "op_create_scoped_arguments" => {
                let destination = signed_operand(instruction, "dst")?;
                let scope =
                    read_register(function, &registers, signed_operand(instruction, "scope")?)?;
                let RegisterValue::Environment(environment) = scope else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                {
                    let mut environment = environment.borrow_mut();
                    for (index, value) in parameter_values.iter().cloned().enumerate() {
                        environment
                            .scoped_argument_values
                            .entry(index)
                            .or_insert(value);
                    }
                }
                registers.insert(
                    destination,
                    RegisterValue::ScopedArguments {
                        environment,
                        length: Rc::new(Cell::new(parameter_values.len())),
                    },
                );
            }
            "op_argument_count" => {
                let destination = signed_operand(instruction, "dst")?;
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::Integer(
                        i64::try_from(parameter_values.len())
                            .map_err(|_| imported_error("argument count exceeds i64"))?,
                    )),
                );
            }
            "op_get_argument" => {
                let destination = signed_operand(instruction, "dst")?;
                let index = signed_operand(instruction, "index")?
                    .checked_sub(1)
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                let index = usize::try_from(index)
                    .map_err(|_| unsupported(function, instruction, descriptor.opcode))?;
                registers.insert(
                    destination,
                    parameter_values
                        .get(index)
                        .cloned()
                        .unwrap_or(RegisterValue::Undefined),
                );
            }
            "op_get_from_arguments" => {
                let destination = signed_operand(instruction, "dst")?;
                let arguments = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "arguments")?,
                )?;
                let RegisterValue::Arguments {
                    kind: ArgumentsKind::Direct,
                    values,
                } = arguments
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let index = usize::try_from(unsigned_operand(instruction, "index")?)
                    .map_err(|_| imported_error("argument index exceeds usize"))?;
                let value = values
                    .borrow()
                    .get(index)
                    .cloned()
                    .unwrap_or(RegisterValue::Undefined);
                registers.insert(destination, value);
            }
            "op_put_to_arguments" => {
                let arguments = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "arguments")?,
                )?;
                let RegisterValue::Arguments {
                    kind: ArgumentsKind::Direct,
                    values,
                } = arguments
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let index = usize::try_from(unsigned_operand(instruction, "index")?)
                    .map_err(|_| imported_error("argument index exceeds usize"))?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let mut values = values.borrow_mut();
                if index >= values.len() {
                    values.resize(index + 1, RegisterValue::Undefined);
                }
                values[index] = value;
            }
            "op_new_array_buffer" => {
                let destination = signed_operand(instruction, "dst")?;
                let template = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "immutableButterfly")?,
                )?;
                let RegisterValue::ArrayTemplate(values) = template else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let length = u32::try_from(values.len())
                    .map_err(|_| imported_error("immutable array length exceeds u32"))?;
                let elements = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        u32::try_from(index)
                            .map(|index| (index, StaticProperty::assigned(value)))
                            .map_err(|_| imported_error("immutable array index exceeds u32"))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        prototype: None,
                        elements,
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        length,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Array(id));
            }
            "op_new_array" => {
                let destination = signed_operand(instruction, "dst")?;
                let first = signed_operand(instruction, "argv")?;
                let count = usize::try_from(unsigned_operand(instruction, "argc")?)
                    .map_err(|_| imported_error("array literal length does not fit usize"))?;
                let mut elements = BTreeMap::new();
                for index in 0..count {
                    let register = first
                        .checked_sub(index as i64)
                        .ok_or_else(|| imported_error("array register range underflow"))?;
                    let index = u32::try_from(index)
                        .map_err(|_| imported_error("array literal length exceeds u32"))?;
                    elements.insert(
                        index,
                        StaticProperty::assigned(read_register(function, &registers, register)?),
                    );
                }
                let length = u32::try_from(count)
                    .map_err(|_| imported_error("array literal length exceeds u32"))?;
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        prototype: None,
                        elements,
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        length,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Array(id));
            }
            "op_new_array_with_size" => {
                let destination = signed_operand(instruction, "dst")?;
                let length =
                    scalar_register(function, &registers, signed_operand(instruction, "length")?)?;
                if !expression_is_closed(&length) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let length = u32::try_from(evaluate(&length, functions, &[], 0)?)
                    .map_err(|_| unsupported(function, instruction, descriptor.opcode))?;
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        prototype: None,
                        elements: BTreeMap::new(),
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        length,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Array(id));
            }
            "op_new_array_with_species" => {
                let destination = signed_operand(instruction, "dst")?;
                let length =
                    scalar_register(function, &registers, signed_operand(instruction, "length")?)?;
                if !expression_is_closed(&length) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let length = u32::try_from(evaluate(&length, functions, &[], 0)?)
                    .map_err(|_| unsupported(function, instruction, descriptor.opcode))?;
                let array =
                    read_register(function, &registers, signed_operand(instruction, "array")?)?;
                let RegisterValue::Array(_) = array else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let constructor = static_get_own_property(
                    &state.heap,
                    &array,
                    StaticPropertyKey::Name("constructor".into()),
                )?;
                if !matches!(constructor, RegisterValue::Undefined) {
                    return Err(imported_error(
                        "custom Array species construction is not statically admitted",
                    ));
                }
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        prototype: None,
                        elements: BTreeMap::new(),
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        length,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Array(id));
            }
            "op_create_rest" => {
                let destination = signed_operand(instruction, "dst")?;
                let skip =
                    usize::try_from(unsigned_operand(instruction, "numParametersToSkip")?)
                        .map_err(|_| imported_error("rest parameter skip count exceeds usize"))?;
                let values = parameter_values
                    .get(skip..)
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                let length = u32::try_from(values.len())
                    .map_err(|_| imported_error("rest parameter count exceeds u32"))?;
                let elements = values
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(index, value)| (index as u32, StaticProperty::assigned(value)))
                    .collect();
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        prototype: None,
                        elements,
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        length,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Array(id));
            }
            "op_spread" => {
                let destination = signed_operand(instruction, "dst")?;
                let argument = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "argument")?,
                )?;
                let RegisterValue::Array(id) = argument else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let Some(StaticHeapEntry::Array {
                    elements, length, ..
                }) = state.heap.get(&id)
                else {
                    return Err(imported_error("spread references a missing static array"));
                };
                let values = (0..*length)
                    .map(|index| {
                        elements
                            .get(&index)
                            .map(StaticProperty::read)
                            .unwrap_or(RegisterValue::Undefined)
                    })
                    .collect();
                registers.insert(destination, RegisterValue::Spread(values));
            }
            "op_iterator_open" => {
                let iterator_register = signed_operand(instruction, "iterator")?;
                let next_register = signed_operand(instruction, "next")?;
                let method = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "symbolIterator")?,
                )?;
                let iterable = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "iterable")?,
                )?;
                let _stack_offset = unsigned_operand(instruction, "stackOffset")?;
                let (RegisterValue::ArrayIteratorMethod, RegisterValue::Array(array_id)) =
                    (method, iterable)
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                registers.insert(
                    iterator_register,
                    RegisterValue::ArrayIterator {
                        array_id,
                        next_index: Rc::new(Cell::new(0)),
                    },
                );
                registers.insert(next_register, RegisterValue::ArrayIteratorNext);
            }
            "op_iterator_next" => {
                let done_register = signed_operand(instruction, "done")?;
                let value_register = signed_operand(instruction, "value")?;
                let iterable = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "iterable")?,
                )?;
                let next =
                    read_register(function, &registers, signed_operand(instruction, "next")?)?;
                let iterator = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "iterator")?,
                )?;
                let _stack_offset = unsigned_operand(instruction, "stackOffset")?;
                let (
                    RegisterValue::Array(expected_array_id),
                    RegisterValue::ArrayIteratorNext,
                    RegisterValue::ArrayIterator {
                        array_id,
                        next_index,
                    },
                ) = (iterable, next, iterator)
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                if expected_array_id != array_id {
                    return Err(imported_error(
                        "array iterator is used with a different iterable",
                    ));
                }
                let Some(StaticHeapEntry::Array {
                    elements, length, ..
                }) = state.heap.get(&array_id)
                else {
                    return Err(imported_error(
                        "array iterator references a missing static array",
                    ));
                };
                let index = next_index.get();
                let done = index >= *length;
                let value = if done {
                    RegisterValue::Undefined
                } else {
                    next_index.set(index + 1);
                    elements
                        .get(&index)
                        .map(StaticProperty::read)
                        .unwrap_or(RegisterValue::Undefined)
                };
                registers.insert(done_register, RegisterValue::Boolean(done));
                registers.insert(value_register, value);
            }
            "op_async_iterator_open" => {
                let iterator_register = signed_operand(instruction, "iterator")?;
                let next_register = signed_operand(instruction, "next")?;
                let method = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "symbolIterator")?,
                )?;
                let iterable = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "iterable")?,
                )?;
                let _stack_offset = unsigned_operand(instruction, "stackOffset")?;
                let RegisterValue::Array(array_id) = iterable else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                if !matches!(method, RegisterValue::Undefined | RegisterValue::Null) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                registers.insert(
                    iterator_register,
                    RegisterValue::AsyncFromSyncIterator {
                        array_id,
                        next_index: Rc::new(Cell::new(0)),
                    },
                );
                registers.insert(next_register, RegisterValue::AsyncFromSyncIteratorNext);
            }
            "op_async_iterator_next" => {
                let destination = signed_operand(instruction, "dst")?;
                let next =
                    read_register(function, &registers, signed_operand(instruction, "next")?)?;
                let iterator = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "iterator")?,
                )?;
                let _driver =
                    read_register(function, &registers, signed_operand(instruction, "driver")?)?;
                let _has_value = boolean_operand(instruction, "hasValue")?;
                let _stack_offset = unsigned_operand(instruction, "stackOffset")?;
                let (
                    RegisterValue::AsyncFromSyncIteratorNext,
                    RegisterValue::AsyncFromSyncIterator {
                        array_id,
                        next_index,
                    },
                ) = (next, iterator)
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let Some(StaticHeapEntry::Array {
                    elements, length, ..
                }) = state.heap.get(&array_id)
                else {
                    return Err(imported_error(
                        "async-from-sync iterator references a missing static array",
                    ));
                };
                let index = next_index.get();
                let done = index >= *length;
                let value = if done {
                    RegisterValue::Undefined
                } else {
                    next_index.set(index + 1);
                    elements
                        .get(&index)
                        .map(StaticProperty::read)
                        .unwrap_or(RegisterValue::Undefined)
                };
                let result_id = state.allocate_heap_id()?;
                state.heap.insert(
                    result_id,
                    StaticHeapEntry::Object {
                        prototype: None,
                        properties: BTreeMap::from([
                            (
                                "done".into(),
                                StaticProperty::assigned(RegisterValue::Boolean(done)),
                            ),
                            ("value".into(), StaticProperty::assigned(value)),
                        ]),
                        property_order: vec!["value".into(), "done".into()],
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(
                    destination,
                    RegisterValue::FulfilledPromise(Box::new(RegisterValue::Object(result_id))),
                );
            }
            "op_new_array_with_spread" => {
                let destination = signed_operand(instruction, "dst")?;
                let first = signed_operand(instruction, "argv")?;
                let count = usize::try_from(unsigned_operand(instruction, "argc")?)
                    .map_err(|_| imported_error("spread array length does not fit usize"))?;
                let _bit_vector = unsigned_operand(instruction, "bitVector")?;
                let mut values = Vec::new();
                for index in 0..count {
                    let register = first
                        .checked_sub(index as i64)
                        .ok_or_else(|| imported_error("spread array register range underflow"))?;
                    match read_register(function, &registers, register)? {
                        RegisterValue::Spread(spread) => values.extend(spread),
                        value => values.push(value),
                    }
                }
                let length = u32::try_from(values.len())
                    .map_err(|_| imported_error("spread array length exceeds u32"))?;
                let elements = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| (index as u32, StaticProperty::assigned(value)))
                    .collect();
                let id = state.allocate_heap_id()?;
                state.heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        prototype: None,
                        elements,
                        properties: BTreeMap::new(),
                        property_order: Vec::new(),
                        length,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Array(id));
            }
            "op_put_by_id" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let property =
                    identifier_string(function, unsigned_operand(instruction, "property")?)?;
                let assignment = static_set_property(
                    &mut state.heap,
                    &base,
                    StaticPropertyKey::Name(property),
                    value.clone(),
                )?;
                apply_static_property_assignment(
                    unit,
                    functions,
                    call_arities,
                    assignment,
                    base,
                    value,
                    state,
                    call_depth,
                )?;
            }
            "op_define_data_property" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let property = static_property_key(&property, functions)?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let attributes = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "attributes")?,
                )?;
                let attributes = static_define_property_attributes(&attributes, functions)?;
                static_define_data_property(&mut state.heap, &base, property, value, attributes)?;
            }
            "op_define_accessor_property" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let property = static_property_key(&property, functions)?;
                let getter =
                    read_register(function, &registers, signed_operand(instruction, "getter")?)?;
                let setter =
                    read_register(function, &registers, signed_operand(instruction, "setter")?)?;
                let attributes = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "attributes")?,
                )?;
                let attributes = static_define_accessor_attributes(&attributes, functions)?;
                let getter = attributes
                    .has_get
                    .then(|| static_accessor_value(getter))
                    .transpose()?
                    .flatten();
                let setter = attributes
                    .has_set
                    .then(|| static_accessor_value(setter))
                    .transpose()?
                    .flatten();
                static_define_accessor_property(
                    &mut state.heap,
                    &base,
                    property,
                    getter,
                    setter,
                    attributes,
                )?;
            }
            "op_get_internal_field" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let index = u32::try_from(unsigned_operand(instruction, "index")?)
                    .map_err(|_| imported_error("internal-field index exceeds u32"))?;
                let RegisterValue::InternalObject { fields, .. } = base else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let value = fields
                    .borrow()
                    .get(&index)
                    .cloned()
                    .unwrap_or(RegisterValue::Undefined);
                registers.insert(destination, value);
            }
            "op_put_internal_field" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let index = u32::try_from(unsigned_operand(instruction, "index")?)
                    .map_err(|_| imported_error("internal-field index exceeds u32"))?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let RegisterValue::InternalObject { fields, .. } = base else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                fields.borrow_mut().insert(index, value);
            }
            "op_put_getter_by_id" | "op_put_setter_by_id" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = StaticPropertyKey::Name(identifier_string(
                    function,
                    unsigned_operand(instruction, "property")?,
                )?);
                let attributes = unsigned_operand(instruction, "attributes")?;
                let accessor = static_accessor_value(read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "accessor")?,
                )?)?;
                let (getter, setter) = if descriptor.opcode == "op_put_getter_by_id" {
                    (
                        StaticAccessorUpdate::Set(accessor),
                        StaticAccessorUpdate::Preserve,
                    )
                } else {
                    (
                        StaticAccessorUpdate::Preserve,
                        StaticAccessorUpdate::Set(accessor),
                    )
                };
                static_put_accessor(&mut state.heap, &base, property, attributes, getter, setter)?;
            }
            "op_put_getter_by_val" | "op_put_setter_by_val" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let property = static_property_key(&property, functions)?;
                let attributes = unsigned_operand(instruction, "attributes")?;
                let accessor = static_accessor_value(read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "accessor")?,
                )?)?;
                let (getter, setter) = if descriptor.opcode == "op_put_getter_by_val" {
                    (
                        StaticAccessorUpdate::Set(accessor),
                        StaticAccessorUpdate::Preserve,
                    )
                } else {
                    (
                        StaticAccessorUpdate::Preserve,
                        StaticAccessorUpdate::Set(accessor),
                    )
                };
                static_put_accessor(&mut state.heap, &base, property, attributes, getter, setter)?;
            }
            "op_put_getter_setter_by_id" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = StaticPropertyKey::Name(identifier_string(
                    function,
                    unsigned_operand(instruction, "property")?,
                )?);
                let attributes = unsigned_operand(instruction, "attributes")?;
                let getter = static_accessor_value(read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "getter")?,
                )?)?;
                let setter = static_accessor_value(read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "setter")?,
                )?)?;
                static_put_accessor(
                    &mut state.heap,
                    &base,
                    property,
                    attributes,
                    StaticAccessorUpdate::Set(getter),
                    StaticAccessorUpdate::Set(setter),
                )?;
            }
            "op_to_property_key" | "op_to_property_key_or_number" => {
                let destination = signed_operand(instruction, "dst")?;
                let source =
                    read_register(function, &registers, signed_operand(instruction, "src")?)?;
                let value = if descriptor.opcode == "op_to_property_key_or_number"
                    && matches!(
                        &source,
                        RegisterValue::Scalar(_)
                            | RegisterValue::NaN
                            | RegisterValue::NegativeZero
                            | RegisterValue::PositiveInfinity
                            | RegisterValue::NegativeInfinity
                    ) {
                    source
                } else if matches!(&source, RegisterValue::PrivateName { .. }) {
                    source
                } else {
                    match static_property_key(&source, functions)? {
                        StaticPropertyKey::Index(index) => {
                            RegisterValue::String(index.to_string().into_boxed_str())
                        }
                        StaticPropertyKey::Name(name) => RegisterValue::String(name),
                    }
                };
                registers.insert(destination, value);
            }
            "op_set_function_name" => {
                let function_value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "function")?,
                )?;
                if !matches!(function_value, RegisterValue::Function { .. }) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let name =
                    read_register(function, &registers, signed_operand(instruction, "name")?)?;
                let name = match static_property_key(&name, functions)? {
                    StaticPropertyKey::Index(index) => index.to_string().into_boxed_str(),
                    StaticPropertyKey::Name(name) => name,
                };
                static_define_data_property(
                    &mut state.heap,
                    &function_value,
                    StaticPropertyKey::Name("name".into()),
                    RegisterValue::String(name),
                    StaticDefinePropertyAttributes {
                        configurable: Some(true),
                        enumerable: Some(false),
                        writable: Some(false),
                        has_value: true,
                    },
                )?;
            }
            "op_put_private_name" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let put_kind = u32::try_from(unsigned_operand(instruction, "putKind")?)
                    .map_err(|_| imported_error("private-field put kind exceeds u32"))?;
                static_put_private_property(&mut state.heap, &base, &property, value, put_kind)?;
            }
            "op_get_private_name" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let value = static_get_private_property(&state.heap, &base, &property)?;
                registers.insert(destination, value);
            }
            "op_has_private_name" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let present = static_has_private_property(&state.heap, &base, &property)?;
                registers.insert(destination, RegisterValue::Boolean(present));
            }
            "op_set_private_brand" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let brand =
                    read_register(function, &registers, signed_operand(instruction, "brand")?)?;
                static_set_private_brand(&mut state.heap, &base, &brand)?;
            }
            "op_check_private_brand" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let brand =
                    read_register(function, &registers, signed_operand(instruction, "brand")?)?;
                if !static_has_private_brand(&state.heap, &base, &brand)? {
                    return Err(imported_error(
                        "private method access failed its brand check",
                    ));
                }
            }
            "op_has_private_brand" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let brand =
                    read_register(function, &registers, signed_operand(instruction, "brand")?)?;
                let present = static_has_private_brand(&state.heap, &base, &brand)?;
                registers.insert(destination, RegisterValue::Boolean(present));
            }
            "op_new_func"
            | "op_new_func_exp"
            | "op_new_generator_func"
            | "op_new_generator_func_exp"
            | "op_new_async_func"
            | "op_new_async_func_exp"
            | "op_new_async_generator_func"
            | "op_new_async_generator_func_exp" => {
                let destination = signed_operand(instruction, "dst")?;
                let index = unsigned_operand(instruction, "functionDecl")?;
                let call = child_function(
                    unit,
                    function.id,
                    descriptor.opcode,
                    index,
                    FunctionSpecialization::Call,
                )?;
                let construct = child_function_optional(
                    unit,
                    function.id,
                    descriptor.opcode,
                    index,
                    FunctionSpecialization::Construct,
                );
                let environment = match read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "scope")?,
                )? {
                    RegisterValue::Environment(environment) => Some(environment),
                    RegisterValue::Opaque => None,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                let identity = state.allocate_function_identity()?;
                let heap_id = state.allocate_heap_id()?;
                let value = RegisterValue::Function {
                    call,
                    construct,
                    environment,
                    identity,
                    heap_id,
                };
                let mut properties = BTreeMap::new();
                let mut property_order = Vec::new();
                if construct.is_some() {
                    let prototype_id = state.allocate_heap_id()?;
                    state.heap.insert(
                        prototype_id,
                        StaticHeapEntry::Object {
                            prototype: None,
                            properties: BTreeMap::from([(
                                "constructor".into(),
                                StaticProperty {
                                    value: value.clone(),
                                    getter: None,
                                    setter: None,
                                    is_accessor: false,
                                    writable: true,
                                    enumerable: false,
                                    configurable: true,
                                },
                            )]),
                            property_order: vec!["constructor".into()],
                            private_properties: BTreeMap::new(),
                            private_brands: BTreeSet::new(),
                        },
                    );
                    properties.insert(
                        "prototype".into(),
                        StaticProperty {
                            value: RegisterValue::Object(prototype_id),
                            getter: None,
                            setter: None,
                            is_accessor: false,
                            writable: true,
                            enumerable: false,
                            configurable: false,
                        },
                    );
                    property_order.push("prototype".into());
                }
                state.heap.insert(
                    heap_id,
                    StaticHeapEntry::Function {
                        prototype: None,
                        properties,
                        property_order,
                        private_properties: BTreeMap::new(),
                        private_brands: BTreeSet::new(),
                    },
                );
                registers.insert(destination, value);
            }
            "op_create_lexical_environment" | "op_create_generator_frame_environment" => {
                let destination = signed_operand(instruction, "dst")?;
                let parent = match read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "scope")?,
                )? {
                    RegisterValue::Environment(environment) => Some(environment),
                    RegisterValue::Opaque => None,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                let _symbol_table = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "symbolTable")?,
                )?;
                let _initial_value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "initialValue")?,
                )?;
                registers.insert(
                    destination,
                    RegisterValue::Environment(Rc::new(RefCell::new(StaticEnvironment {
                        kind: if descriptor.opcode == "op_create_generator_frame_environment" {
                            StaticEnvironmentKind::Var
                        } else {
                            StaticEnvironmentKind::Lexical
                        },
                        parent,
                        object_scope: None,
                        bindings: BTreeMap::new(),
                        scoped_argument_values: BTreeMap::new(),
                        scoped_argument_names: BTreeMap::new(),
                    }))),
                );
            }
            "op_push_with_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                let current_scope = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "currentScope")?,
                )?;
                let new_scope = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "newScope")?,
                )?;
                let parent = match current_scope {
                    RegisterValue::Environment(environment) => Some(environment),
                    RegisterValue::Opaque => None,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                if !matches!(
                    new_scope,
                    RegisterValue::Object(_)
                        | RegisterValue::Array(_)
                        | RegisterValue::Function { .. }
                ) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                registers.insert(
                    destination,
                    RegisterValue::Environment(Rc::new(RefCell::new(StaticEnvironment {
                        kind: StaticEnvironmentKind::With,
                        parent,
                        object_scope: Some(new_scope),
                        bindings: BTreeMap::new(),
                        scoped_argument_values: BTreeMap::new(),
                        scoped_argument_names: BTreeMap::new(),
                    }))),
                );
            }
            "op_put_to_scope" => {
                let scope =
                    read_register(function, &registers, signed_operand(instruction, "scope")?)?;
                let RegisterValue::Environment(environment) = scope else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let identifier_index = unsigned_operand(instruction, "var")?;
                let identifier = if identifier_index == u64::from(u32::MAX) {
                    None
                } else {
                    Some(identifier_string(function, identifier_index)?)
                };
                let value_register = signed_operand(instruction, "value")?;
                let value = read_register(function, &registers, value_register)?;
                let object_scope = environment.borrow().object_scope.clone();
                if let Some(object_scope) = object_scope {
                    let identifier = identifier.ok_or_else(|| {
                        imported_error("anonymous scoped argument cannot target a with object")
                    })?;
                    let assignment = static_set_property(
                        &mut state.heap,
                        &object_scope,
                        StaticPropertyKey::Name(identifier),
                        value.clone(),
                    )?;
                    apply_static_property_assignment(
                        unit,
                        functions,
                        call_arities,
                        assignment,
                        object_scope,
                        value,
                        state,
                        call_depth,
                    )?;
                } else {
                    let get_put_info = unsigned_operand(instruction, "getPutInfo")?;
                    let scoped_argument_initialization = (get_put_info >> 10) & 0b11 == 3;
                    let mut environment = environment.borrow_mut();
                    if scoped_argument_initialization {
                        let index = value_register
                            .checked_sub(i64::from(function.call_frame_first_argument_register))
                            .and_then(|index| usize::try_from(index).ok())
                            .ok_or_else(|| {
                                imported_error(
                                    "scoped argument initialization does not read an argument register",
                                )
                            })?;
                        environment
                            .scoped_argument_values
                            .insert(index, value.clone());
                        if let Some(identifier) = &identifier {
                            environment
                                .scoped_argument_names
                                .insert(identifier.clone(), index);
                        }
                    } else if let Some(identifier) = &identifier
                        && let Some(index) =
                            environment.scoped_argument_names.get(identifier).copied()
                    {
                        environment
                            .scoped_argument_values
                            .insert(index, value.clone());
                    }
                    if let Some(identifier) = identifier {
                        environment.bindings.insert(identifier, value);
                    }
                }
            }
            "op_get_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                registers.insert(
                    destination,
                    RegisterValue::Environment(callee_scope.clone()),
                );
            }
            "op_get_parent_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                let scope =
                    read_register(function, &registers, signed_operand(instruction, "scope")?)?;
                let RegisterValue::Environment(environment) = scope else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let parent = environment.borrow().parent.clone();
                registers.insert(
                    destination,
                    parent
                        .map(RegisterValue::Environment)
                        .unwrap_or(RegisterValue::Opaque),
                );
            }
            "op_resolve_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                let identifier = unsigned_operand(instruction, "var")?;
                let scope =
                    read_register(function, &registers, signed_operand(instruction, "scope")?)?;
                let identifier_text = identifier_string(function, identifier)?;
                if let RegisterValue::Environment(environment) = &scope
                    && let Some(resolved) =
                        resolve_environment(&state.heap, environment, &identifier_text)?
                {
                    registers.insert(destination, RegisterValue::Environment(resolved));
                } else if identifier_is(function, identifier, b"console")? {
                    registers.insert(destination, RegisterValue::ConsoleScope);
                } else if identifier_is(function, identifier, b"NaN")? {
                    registers.insert(destination, RegisterValue::NaNScope);
                } else if identifier_is(function, identifier, b"Infinity")? {
                    registers.insert(destination, RegisterValue::InfinityScope);
                } else if identifier_is(function, identifier, b"undefined")? {
                    registers.insert(destination, RegisterValue::UndefinedScope);
                } else if let Some(builtin) = builtin_identifier(function, identifier)? {
                    registers.insert(destination, RegisterValue::BuiltinScope(builtin));
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_resolve_scope_for_hoisting_func_decl_in_eval" => {
                let destination = signed_operand(instruction, "dst")?;
                let scope =
                    read_register(function, &registers, signed_operand(instruction, "scope")?)?;
                let property =
                    identifier_string(function, unsigned_operand(instruction, "property")?)?;
                let RegisterValue::Environment(mut current) = scope else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let mut resolved = None;
                loop {
                    let borrowed = current.borrow();
                    if borrowed.kind != StaticEnvironmentKind::With {
                        let has_binding = borrowed.bindings.contains_key(&property)
                            || borrowed.scoped_argument_names.contains_key(&property);
                        if has_binding {
                            if borrowed.kind == StaticEnvironmentKind::Var {
                                resolved = Some(current.clone());
                            }
                            break;
                        }
                        if borrowed.kind == StaticEnvironmentKind::Var {
                            resolved = Some(current.clone());
                            break;
                        }
                    }
                    let parent = borrowed.parent.clone();
                    drop(borrowed);
                    let Some(parent) = parent else {
                        break;
                    };
                    current = parent;
                }
                registers.insert(
                    destination,
                    resolved
                        .map(RegisterValue::Environment)
                        .unwrap_or(RegisterValue::Undefined),
                );
            }
            "op_get_from_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                let scope = signed_operand(instruction, "scope")?;
                let identifier = unsigned_operand(instruction, "var")?;
                if let Some(RegisterValue::Environment(environment)) = registers.get(&scope)
                    && let Some(value) = environment_binding(
                        &state.heap,
                        environment,
                        &identifier_string(function, identifier)?,
                    )?
                {
                    registers.insert(destination, value);
                } else if matches!(registers.get(&scope), Some(RegisterValue::ConsoleScope))
                    && identifier_is(function, identifier, b"console")?
                {
                    registers.insert(destination, RegisterValue::ConsoleObject);
                } else if matches!(registers.get(&scope), Some(RegisterValue::NaNScope))
                    && identifier_is(function, identifier, b"NaN")?
                {
                    registers.insert(destination, RegisterValue::NaN);
                } else if matches!(registers.get(&scope), Some(RegisterValue::InfinityScope))
                    && identifier_is(function, identifier, b"Infinity")?
                {
                    registers.insert(destination, RegisterValue::PositiveInfinity);
                } else if matches!(registers.get(&scope), Some(RegisterValue::UndefinedScope))
                    && identifier_is(function, identifier, b"undefined")?
                {
                    registers.insert(destination, RegisterValue::Undefined);
                } else if let Some(RegisterValue::BuiltinScope(expected)) = registers.get(&scope)
                    && builtin_identifier(function, identifier)? == Some(*expected)
                {
                    registers.insert(destination, RegisterValue::Builtin(*expected));
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_get_by_id" | "op_get_by_id_direct" => {
                let destination = signed_operand(instruction, "dst")?;
                let base = signed_operand(instruction, "base")?;
                let property = unsigned_operand(instruction, "property")?;
                if descriptor.opcode == "op_get_by_id"
                    && matches!(registers.get(&base), Some(RegisterValue::ConsoleObject))
                    && identifier_is(function, property, b"log")?
                {
                    registers.insert(destination, RegisterValue::ConsoleLog);
                } else if let Some(base) = registers.get(&base).cloned() {
                    let property = identifier_string(function, property)?;
                    let key = StaticPropertyKey::Name(property);
                    let value = if descriptor.opcode == "op_get_by_id_direct" {
                        static_get_own_property(&state.heap, &base, key)?
                    } else {
                        static_get_property(&state.heap, &base, key)?
                    };
                    let value = resolve_static_property_read(
                        unit,
                        functions,
                        call_arities,
                        value,
                        base,
                        state,
                        call_depth,
                    )?;
                    registers.insert(destination, value);
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_get_by_id_with_this" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let receiver = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "thisValue")?,
                )?;
                let property = StaticPropertyKey::Name(identifier_string(
                    function,
                    unsigned_operand(instruction, "property")?,
                )?);
                let value = static_get_property(&state.heap, &base, property)?;
                let value = resolve_static_property_read(
                    unit,
                    functions,
                    call_arities,
                    value,
                    receiver,
                    state,
                    call_depth,
                )?;
                registers.insert(destination, value);
            }
            "op_get_length" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let length = match base {
                    RegisterValue::Array(id) => match state.heap.get(&id) {
                        Some(StaticHeapEntry::Array { length, .. }) => i64::from(*length),
                        _ => {
                            return Err(imported_error(
                                "array references a missing static heap entry",
                            ));
                        }
                    },
                    RegisterValue::String(value) => i64::try_from(value.encode_utf16().count())
                        .map_err(|_| imported_error("string length exceeds i64"))?,
                    RegisterValue::Arguments { values, .. } => i64::try_from(values.borrow().len())
                        .map_err(|_| imported_error("argument count exceeds i64"))?,
                    RegisterValue::ScopedArguments { length, .. } => i64::try_from(length.get())
                        .map_err(|_| imported_error("argument count exceeds i64"))?,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::Integer(length)),
                );
            }
            "op_get_prototype_of" => {
                let destination = signed_operand(instruction, "dst")?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let prototype = static_prototype_value(&state.heap, &value)?;
                registers.insert(destination, prototype);
            }
            "op_instanceof" => {
                let destination = signed_operand(instruction, "dst")?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let constructor = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "constructor")?,
                )?;
                let prototype = static_get_property(
                    &state.heap,
                    &constructor,
                    StaticPropertyKey::Name("prototype".into()),
                )?;
                let RegisterValue::Object(prototype_id) = prototype.clone() else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let result = static_prototype_chain_contains(&state.heap, &value, prototype_id)?;
                registers.insert(
                    signed_operand(instruction, "hasInstanceOrPrototype")?,
                    prototype,
                );
                registers.insert(destination, RegisterValue::Boolean(result));
            }
            "op_get_property_enumerator" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let (heap_id, keys) = static_enumerable_keys(&state.heap, &base)?;
                let enumerator = if keys.is_empty() {
                    RegisterValue::Builtin(Builtin::EmptyPropertyNameEnumerator)
                } else {
                    RegisterValue::Enumerator { heap_id, keys }
                };
                registers.insert(destination, enumerator);
            }
            "op_enumerator_next" => {
                let property_name = signed_operand(instruction, "propertyName")?;
                let mode = signed_operand(instruction, "mode")?;
                let index = signed_operand(instruction, "index")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let enumerator = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "enumerator")?,
                )?;
                let RegisterValue::Enumerator { heap_id, keys } = enumerator else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                ensure_enumerator_base(&base, heap_id)?;
                let index_expression = scalar_register(function, &registers, index)?;
                if !expression_is_closed(&index_expression) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let current_index =
                    usize::try_from(evaluate(&index_expression, functions, &[], 0)?)
                        .map_err(|_| unsupported(function, instruction, descriptor.opcode))?;
                if let Some(key) = keys.get(current_index) {
                    registers.insert(property_name, RegisterValue::String(key.clone()));
                    let next_index = i64::try_from(current_index + 1)
                        .map_err(|_| imported_error("enumerator index exceeds i64"))?;
                    registers.insert(
                        index,
                        RegisterValue::Scalar(ScalarExpression::Integer(next_index)),
                    );
                    registers.insert(mode, RegisterValue::Scalar(ScalarExpression::Integer(0)));
                } else {
                    registers.insert(
                        property_name,
                        RegisterValue::Builtin(Builtin::SentinelString),
                    );
                }
            }
            "op_enumerator_get_by_val" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property_name = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "propertyName")?,
                )?;
                let enumerator = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "enumerator")?,
                )?;
                let RegisterValue::Enumerator { heap_id, .. } = enumerator else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                ensure_enumerator_base(&base, heap_id)?;
                let RegisterValue::String(property_name) = property_name else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let value = static_get_property(
                    &state.heap,
                    &base,
                    StaticPropertyKey::Name(property_name),
                )?;
                let value = resolve_static_property_read(
                    unit,
                    functions,
                    call_arities,
                    value,
                    base,
                    state,
                    call_depth,
                )?;
                registers.insert(destination, value);
            }
            "op_enumerator_in_by_val" | "op_enumerator_has_own_property" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property_name = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "propertyName")?,
                )?;
                let enumerator = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "enumerator")?,
                )?;
                let RegisterValue::Enumerator { heap_id, .. } = enumerator else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                ensure_enumerator_base(&base, heap_id)?;
                let RegisterValue::String(property_name) = property_name else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let key = StaticPropertyKey::Name(property_name);
                let present = if descriptor.opcode == "op_enumerator_has_own_property" {
                    static_has_own_property(&state.heap, &base, key)?
                } else {
                    static_has_property(&state.heap, &base, key)?
                };
                registers.insert(destination, RegisterValue::Boolean(present));
            }
            "op_enumerator_put_by_val" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property_name = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "propertyName")?,
                )?;
                let enumerator = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "enumerator")?,
                )?;
                let RegisterValue::Enumerator { heap_id, .. } = enumerator else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                ensure_enumerator_base(&base, heap_id)?;
                let RegisterValue::String(property_name) = property_name else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let assignment = static_set_property(
                    &mut state.heap,
                    &base,
                    StaticPropertyKey::Name(property_name),
                    value.clone(),
                )?;
                apply_static_property_assignment(
                    unit,
                    functions,
                    call_arities,
                    assignment,
                    base,
                    value,
                    state,
                    call_depth,
                )?;
            }
            "op_get_by_val" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let property = static_property_key(&property, functions)?;
                let value = static_get_property(&state.heap, &base, property)?;
                let value = resolve_static_property_read(
                    unit,
                    functions,
                    call_arities,
                    value,
                    base,
                    state,
                    call_depth,
                )?;
                registers.insert(destination, value);
            }
            "op_get_by_val_with_this" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let receiver = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "thisValue")?,
                )?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let property = static_property_key(&property, functions)?;
                let value = static_get_property(&state.heap, &base, property)?;
                let value = resolve_static_property_read(
                    unit,
                    functions,
                    call_arities,
                    value,
                    receiver,
                    state,
                    call_depth,
                )?;
                registers.insert(destination, value);
            }
            "op_put_by_val" | "op_put_by_val_direct" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "property")?,
                )?;
                let property = static_property_key(&property, functions)?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                if descriptor.opcode == "op_put_by_val_direct" {
                    static_put_property(&mut state.heap, &base, property, value)?;
                } else {
                    let assignment =
                        static_set_property(&mut state.heap, &base, property, value.clone())?;
                    apply_static_property_assignment(
                        unit,
                        functions,
                        call_arities,
                        assignment,
                        base,
                        value,
                        state,
                        call_depth,
                    )?;
                }
            }
            "op_put_by_id_with_this" | "op_put_by_val_with_this" => {
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let receiver = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "thisValue")?,
                )?;
                let property = if descriptor.opcode == "op_put_by_id_with_this" {
                    StaticPropertyKey::Name(identifier_string(
                        function,
                        unsigned_operand(instruction, "property")?,
                    )?)
                } else {
                    let property = read_register(
                        function,
                        &registers,
                        signed_operand(instruction, "property")?,
                    )?;
                    static_property_key(&property, functions)?
                };
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let _ecma_mode = unsigned_operand(instruction, "ecmaMode")?;
                let assignment = static_set_property_with_receiver(
                    &mut state.heap,
                    &base,
                    &receiver,
                    property,
                    value.clone(),
                )?;
                apply_static_property_assignment(
                    unit,
                    functions,
                    call_arities,
                    assignment,
                    receiver,
                    value,
                    state,
                    call_depth,
                )?;
            }
            "op_in_by_id" | "op_in_by_val" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = if descriptor.opcode == "op_in_by_id" {
                    StaticPropertyKey::Name(identifier_string(
                        function,
                        unsigned_operand(instruction, "property")?,
                    )?)
                } else {
                    let property = read_register(
                        function,
                        &registers,
                        signed_operand(instruction, "property")?,
                    )?;
                    static_property_key(&property, functions)?
                };
                let present = static_has_property(&state.heap, &base, property)?;
                registers.insert(destination, RegisterValue::Boolean(present));
            }
            "op_del_by_id" | "op_del_by_val" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let property = if descriptor.opcode == "op_del_by_id" {
                    StaticPropertyKey::Name(identifier_string(
                        function,
                        unsigned_operand(instruction, "property")?,
                    )?)
                } else {
                    let property = read_register(
                        function,
                        &registers,
                        signed_operand(instruction, "property")?,
                    )?;
                    static_property_key(&property, functions)?
                };
                static_delete_property(&mut state.heap, &base, property)?;
                registers.insert(destination, RegisterValue::Boolean(true));
            }
            "op_add" | "op_sub" | "op_mul" | "op_div" | "op_mod" | "op_pow" | "op_bitand"
            | "op_bitor" | "op_bitxor" | "op_lshift" | "op_rshift" | "op_urshift" => {
                let destination = signed_operand(instruction, "dst")?;
                let left =
                    scalar_register(function, &registers, signed_operand(instruction, "lhs")?)?;
                let right =
                    scalar_register(function, &registers, signed_operand(instruction, "rhs")?)?;
                let expression = match descriptor.opcode {
                    "op_add" => ScalarExpression::Add(Box::new(left), Box::new(right)),
                    "op_sub" => ScalarExpression::Subtract(Box::new(left), Box::new(right)),
                    "op_mul" => ScalarExpression::Multiply(Box::new(left), Box::new(right)),
                    "op_div" => ScalarExpression::Divide(Box::new(left), Box::new(right)),
                    "op_mod" => ScalarExpression::Remainder(Box::new(left), Box::new(right)),
                    "op_pow" => ScalarExpression::Power(Box::new(left), Box::new(right)),
                    "op_bitand" => ScalarExpression::BitAnd(Box::new(left), Box::new(right)),
                    "op_bitor" => ScalarExpression::BitOr(Box::new(left), Box::new(right)),
                    "op_bitxor" => ScalarExpression::BitXor(Box::new(left), Box::new(right)),
                    "op_lshift" => ScalarExpression::LeftShift(Box::new(left), Box::new(right)),
                    "op_rshift" => ScalarExpression::RightShift(Box::new(left), Box::new(right)),
                    "op_urshift" => {
                        ScalarExpression::UnsignedRightShift(Box::new(left), Box::new(right))
                    }
                    _ => unreachable!(),
                };
                registers.insert(destination, RegisterValue::Scalar(expression));
            }
            "op_eq" | "op_neq" | "op_stricteq" | "op_nstricteq" => {
                let destination = signed_operand(instruction, "dst")?;
                let left =
                    read_register(function, &registers, signed_operand(instruction, "lhs")?)?;
                let right =
                    read_register(function, &registers, signed_operand(instruction, "rhs")?)?;
                let known = known_strict_equality(&left, &right, functions)?;
                if let Some(equal) = known {
                    let equal = if matches!(descriptor.opcode, "op_neq" | "op_nstricteq") {
                        !equal
                    } else {
                        equal
                    };
                    registers.insert(destination, RegisterValue::Boolean(equal));
                    instruction_index += 1;
                    continue;
                }
                let (RegisterValue::Scalar(left), RegisterValue::Scalar(right)) = (left, right)
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let expression = if matches!(descriptor.opcode, "op_eq" | "op_stricteq") {
                    ScalarExpression::Equal(Box::new(left), Box::new(right))
                } else {
                    ScalarExpression::NotEqual(Box::new(left), Box::new(right))
                };
                registers.insert(destination, RegisterValue::BooleanScalar(expression));
            }
            "op_less" | "op_lesseq" | "op_greater" | "op_greatereq" | "op_below" | "op_beloweq" => {
                let destination = signed_operand(instruction, "dst")?;
                let left =
                    scalar_register(function, &registers, signed_operand(instruction, "lhs")?)?;
                let right =
                    scalar_register(function, &registers, signed_operand(instruction, "rhs")?)?;
                let expression = match descriptor.opcode {
                    "op_less" => ScalarExpression::Less(Box::new(left), Box::new(right)),
                    "op_lesseq" => ScalarExpression::LessEqual(Box::new(left), Box::new(right)),
                    "op_greater" => ScalarExpression::Greater(Box::new(left), Box::new(right)),
                    "op_greatereq" => {
                        ScalarExpression::GreaterEqual(Box::new(left), Box::new(right))
                    }
                    "op_below" => ScalarExpression::Below(Box::new(left), Box::new(right)),
                    "op_beloweq" => ScalarExpression::BelowEqual(Box::new(left), Box::new(right)),
                    _ => unreachable!(),
                };
                registers.insert(destination, RegisterValue::BooleanScalar(expression));
            }
            "op_eq_null"
            | "op_neq_null"
            | "op_is_empty"
            | "op_typeof_is_undefined"
            | "op_typeof_is_object"
            | "op_typeof_is_function"
            | "op_is_undefined_or_null"
            | "op_is_boolean"
            | "op_is_number"
            | "op_is_big_int"
            | "op_is_object"
            | "op_is_callable"
            | "op_is_constructor" => {
                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                let result = known_unary_predicate(descriptor.opcode, &value)
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                registers.insert(destination, RegisterValue::Boolean(result));
            }
            "op_is_cell_with_type" => {
                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                let cell_type = u8::try_from(unsigned_operand(instruction, "type")?)
                    .map_err(|_| imported_error("JSC cell type exceeds u8"))?;
                let result = known_cell_type(&value)
                    .map(|actual| actual == cell_type)
                    .unwrap_or(false);
                registers.insert(destination, RegisterValue::Boolean(result));
            }
            "op_has_structure_with_flags" => {
                const DID_PREVENT_EXTENSIONS: u32 = 1 << 20;
                const HAS_NON_CONFIGURABLE_PROPERTIES: u32 = 1 << 29;
                const HAS_NON_CONFIGURABLE_READ_ONLY_OR_ACCESSOR_PROPERTIES: u32 = 1 << 30;

                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                let flags = u32::try_from(unsigned_operand(instruction, "flags")?)
                    .map_err(|_| imported_error("structure flags exceed u32"))?;
                if flags
                    & !(DID_PREVENT_EXTENSIONS
                        | HAS_NON_CONFIGURABLE_PROPERTIES
                        | HAS_NON_CONFIGURABLE_READ_ONLY_OR_ACCESSOR_PROPERTIES)
                    != 0
                {
                    return Err(imported_error("unsupported JSC structure flag mask"));
                }
                let id = match value {
                    RegisterValue::Object(id) | RegisterValue::Array(id) => id,
                    RegisterValue::Function { heap_id, .. } => heap_id,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                let entry = state.heap.get(&id).ok_or_else(|| {
                    imported_error("structure-flag query references a missing static object")
                })?;
                let mut has_non_configurable = false;
                let mut has_non_configurable_read_only_or_accessor = false;
                let mut observe = |property: &StaticProperty| {
                    if !property.configurable {
                        has_non_configurable = true;
                        if !property.writable || property.is_accessor {
                            has_non_configurable_read_only_or_accessor = true;
                        }
                    }
                };
                match entry {
                    StaticHeapEntry::Object { properties, .. }
                    | StaticHeapEntry::Function { properties, .. } => {
                        properties.values().for_each(&mut observe);
                    }
                    StaticHeapEntry::Array {
                        elements,
                        properties,
                        ..
                    } => {
                        elements.values().for_each(&mut observe);
                        properties.values().for_each(&mut observe);
                    }
                }
                let mut actual = 0;
                if has_non_configurable {
                    actual |= HAS_NON_CONFIGURABLE_PROPERTIES;
                }
                if has_non_configurable_read_only_or_accessor {
                    actual |= HAS_NON_CONFIGURABLE_READ_ONLY_OR_ACCESSOR_PROPERTIES;
                }
                registers.insert(destination, RegisterValue::Boolean(actual & flags != 0));
            }
            "op_negate" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = scalar_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::Negate(Box::new(source))),
                );
            }
            "op_bitnot" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = scalar_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::BitNot(Box::new(source))),
                );
            }
            "op_not" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                let expression = match source {
                    RegisterValue::Scalar(expression)
                    | RegisterValue::BooleanScalar(expression) => {
                        ScalarExpression::LogicalNot(Box::new(expression))
                    }
                    RegisterValue::Boolean(value) => ScalarExpression::Integer(i64::from(!value)),
                    RegisterValue::Null
                    | RegisterValue::Undefined
                    | RegisterValue::NaN
                    | RegisterValue::NegativeZero => ScalarExpression::Integer(1),
                    RegisterValue::String(value) if value.is_empty() => {
                        ScalarExpression::Integer(1)
                    }
                    RegisterValue::String(_)
                    | RegisterValue::PositiveInfinity
                    | RegisterValue::NegativeInfinity => ScalarExpression::Integer(0),
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                registers.insert(destination, RegisterValue::BooleanScalar(expression));
            }
            "op_unsigned" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = scalar_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::Unsigned(Box::new(source))),
                );
            }
            "op_inc" | "op_dec" => {
                let register = signed_operand(instruction, "srcDst")?;
                let source = scalar_register(function, &registers, register)?;
                let one = ScalarExpression::Integer(1);
                let expression = if descriptor.opcode == "op_inc" {
                    ScalarExpression::Add(Box::new(source), Box::new(one))
                } else {
                    ScalarExpression::Subtract(Box::new(source), Box::new(one))
                };
                registers.insert(register, RegisterValue::Scalar(expression));
            }
            "op_identity_with_profile" => {
                let register = signed_operand(instruction, "srcDst")?;
                let value = read_register(function, &registers, register)?;
                registers.insert(register, value);
            }
            "op_check_tdz" => {
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "targetVirtualRegister")?,
                )?;
                if matches!(value, RegisterValue::Empty) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_check_traps" => {}
            "op_to_number" | "op_to_numeric" => {
                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                if matches!(value, RegisterValue::BigInt { .. }) {
                    if descriptor.opcode == "op_to_numeric" {
                        registers.insert(destination, value);
                        instruction_index += 1;
                        continue;
                    }
                    let thrown = RegisterValue::Error {
                        kind: 5,
                        message: "Cannot convert a BigInt value to a number".into(),
                    };
                    if let Some(target) =
                        static_exception_target(function, instruction, &instruction_indices)?
                    {
                        pending_exception = Some(thrown);
                        instruction_index = target;
                        continue;
                    }
                    abrupt = Some(thrown);
                    break;
                }
                let value = match value {
                    RegisterValue::Scalar(_)
                    | RegisterValue::NaN
                    | RegisterValue::NegativeZero
                    | RegisterValue::PositiveInfinity
                    | RegisterValue::NegativeInfinity => value,
                    RegisterValue::Boolean(value) => {
                        RegisterValue::Scalar(ScalarExpression::Integer(i64::from(value)))
                    }
                    RegisterValue::BooleanScalar(expression) => RegisterValue::Scalar(expression),
                    RegisterValue::Null => RegisterValue::Scalar(ScalarExpression::Integer(0)),
                    RegisterValue::Undefined => RegisterValue::NaN,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                registers.insert(destination, value);
            }
            "op_to_string" => {
                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                let mut text = String::new();
                append_js_string(&value, functions, &mut text)?;
                registers.insert(destination, RegisterValue::String(text.into_boxed_str()));
            }
            "op_typeof" => {
                let destination = signed_operand(instruction, "dst")?;
                let value =
                    read_register(function, &registers, signed_operand(instruction, "value")?)?;
                let type_name = known_typeof(&value)
                    .ok_or_else(|| unsupported(function, instruction, descriptor.opcode))?;
                registers.insert(destination, RegisterValue::String(type_name.into()));
            }
            "op_to_primitive" => {
                let destination = signed_operand(instruction, "dst")?;
                let source =
                    read_register(function, &registers, signed_operand(instruction, "src")?)?;
                if matches!(
                    source,
                    RegisterValue::Scalar(_)
                        | RegisterValue::BooleanScalar(_)
                        | RegisterValue::String(_)
                        | RegisterValue::Concatenation(_)
                        | RegisterValue::BigInt { .. }
                        | RegisterValue::Undefined
                        | RegisterValue::Null
                        | RegisterValue::Boolean(_)
                        | RegisterValue::NaN
                        | RegisterValue::NegativeZero
                        | RegisterValue::PositiveInfinity
                        | RegisterValue::NegativeInfinity
                ) {
                    registers.insert(destination, source);
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_to_object" => {
                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
                let message = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "message")?,
                )?;
                if matches!(value, RegisterValue::Undefined | RegisterValue::Null) {
                    let RegisterValue::String(message) = message else {
                        return Err(unsupported(function, instruction, descriptor.opcode));
                    };
                    let thrown = RegisterValue::Error { kind: 5, message };
                    if let Some(target) =
                        static_exception_target(function, instruction, &instruction_indices)?
                    {
                        pending_exception = Some(thrown);
                        instruction_index = target;
                        continue;
                    }
                    abrupt = Some(thrown);
                    break;
                }
                if matches!(
                    value,
                    RegisterValue::Function { .. }
                        | RegisterValue::RegExp { .. }
                        | RegisterValue::Object(_)
                        | RegisterValue::Array(_)
                        | RegisterValue::Arguments { .. }
                        | RegisterValue::ScopedArguments { .. }
                        | RegisterValue::InternalObject { .. }
                        | RegisterValue::AsyncFromSyncIterator { .. }
                        | RegisterValue::FulfilledPromise(_)
                        | RegisterValue::GlobalObject
                        | RegisterValue::ConsoleObject
                        | RegisterValue::Error { .. }
                ) {
                    registers.insert(destination, value);
                } else {
                    let mut properties = BTreeMap::new();
                    let mut property_order = Vec::new();
                    if let RegisterValue::String(text) = &value {
                        if text.chars().count() != text.encode_utf16().count() {
                            return Err(imported_error(
                                "boxed strings containing surrogate pairs are not yet admitted",
                            ));
                        }
                        for (index, character) in text.chars().enumerate() {
                            let name = index.to_string().into_boxed_str();
                            property_order.push(name.clone());
                            properties.insert(
                                name,
                                StaticProperty {
                                    value: RegisterValue::String(
                                        character.to_string().into_boxed_str(),
                                    ),
                                    getter: None,
                                    setter: None,
                                    is_accessor: false,
                                    writable: false,
                                    enumerable: true,
                                    configurable: false,
                                },
                            );
                        }
                        properties.insert(
                            "length".into(),
                            StaticProperty {
                                value: RegisterValue::Scalar(ScalarExpression::Integer(
                                    i64::try_from(text.encode_utf16().count()).map_err(|_| {
                                        imported_error("boxed string length exceeds i64")
                                    })?,
                                )),
                                getter: None,
                                setter: None,
                                is_accessor: false,
                                writable: false,
                                enumerable: false,
                                configurable: false,
                            },
                        );
                    } else if !is_admitted_primitive(&value) {
                        return Err(unsupported(function, instruction, descriptor.opcode));
                    }
                    let id = state.allocate_heap_id()?;
                    state.heap.insert(
                        id,
                        StaticHeapEntry::Object {
                            prototype: None,
                            properties,
                            property_order,
                            private_properties: BTreeMap::new(),
                            private_brands: BTreeSet::new(),
                        },
                    );
                    registers.insert(destination, RegisterValue::Object(id));
                }
            }
            "op_to_this" => {
                let register = signed_operand(instruction, "srcDst")?;
                let value = read_register(function, &registers, register)?;
                let ecma_mode = unsigned_operand(instruction, "ecmaMode")?;
                if ecma_mode > 1 {
                    return Err(imported_error("op_to_this has an invalid ECMAMode"));
                }
                let value = if matches!(value, RegisterValue::Environment(_)) {
                    if ecma_mode == 0 {
                        RegisterValue::Undefined
                    } else {
                        RegisterValue::GlobalObject
                    }
                } else if ecma_mode == 1
                    && matches!(value, RegisterValue::Undefined | RegisterValue::Null)
                {
                    RegisterValue::GlobalObject
                } else {
                    value
                };
                let is_object = matches!(
                    value,
                    RegisterValue::Function { .. }
                        | RegisterValue::RegExp { .. }
                        | RegisterValue::Object(_)
                        | RegisterValue::Array(_)
                        | RegisterValue::Arguments { .. }
                        | RegisterValue::ScopedArguments { .. }
                        | RegisterValue::InternalObject { .. }
                        | RegisterValue::AsyncFromSyncIterator { .. }
                        | RegisterValue::FulfilledPromise(_)
                        | RegisterValue::GlobalObject
                        | RegisterValue::ConsoleObject
                        | RegisterValue::Error { .. }
                );
                if known_js_type(&value).is_none() || (ecma_mode == 1 && !is_object) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                registers.insert(register, value);
            }
            "op_strcat" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = signed_operand(instruction, "src")?;
                let count = signed_operand(instruction, "count")?;
                let count = usize::try_from(count)
                    .map_err(|_| imported_error("string concatenation count is negative"))?;
                if count == 0 {
                    return Err(imported_error("string concatenation has no operands"));
                }
                let mut values = Vec::with_capacity(count);
                for index in 0..count {
                    let index = i64::try_from(index)
                        .map_err(|_| imported_error("string concatenation range exceeds i64"))?;
                    let value = read_register(function, &registers, source - index)?;
                    if !is_admitted_primitive(&value) {
                        return Err(unsupported(function, instruction, descriptor.opcode));
                    }
                    values.push(value);
                }
                registers.insert(destination, RegisterValue::Concatenation(values));
            }
            "op_call_direct_eval" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee =
                    read_register(function, &registers, signed_operand(instruction, "callee")?)?;
                if !matches!(callee, RegisterValue::Builtin(Builtin::Eval)) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let _this_value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "thisValue")?,
                )?;
                let _scope =
                    read_register(function, &registers, signed_operand(instruction, "scope")?)?;
                let _lexically_scoped_features =
                    unsigned_operand(instruction, "lexicallyScopedFeatures")?;
                let arguments = call_register_values(function, instruction, &registers)?;
                let argument = arguments
                    .first()
                    .cloned()
                    .unwrap_or(RegisterValue::Undefined);
                let value = if let RegisterValue::String(source) = argument {
                    match crate::application::evaluate_static_expression(source.as_bytes())
                        .and_then(register_value_from_static_application)
                    {
                        Ok(value) => value,
                        Err(error) => {
                            let thrown = RegisterValue::Error {
                                kind: 4,
                                message: error.to_string().into_boxed_str(),
                            };
                            if let Some(target) = static_exception_target(
                                function,
                                instruction,
                                &instruction_indices,
                            )? {
                                pending_exception = Some(thrown);
                                instruction_index = target;
                                continue;
                            }
                            abrupt = Some(thrown);
                            break;
                        }
                    }
                } else {
                    argument
                };
                registers.insert(destination, value);
            }
            "op_call_varargs" | "op_tail_call_varargs" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee_value =
                    read_register(function, &registers, signed_operand(instruction, "callee")?)?;
                let this_value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "thisValue")?,
                )?;
                let argument_list = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "arguments")?,
                )?;
                let first = usize::try_from(signed_operand(instruction, "firstVarArg")?)
                    .map_err(|_| unsupported(function, instruction, descriptor.opcode))?;
                let arguments = static_argument_values(&state.heap, &argument_list, first)?;
                let RegisterValue::Function {
                    call: callee,
                    environment,
                    ..
                } = callee_value.clone()
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let callee_definition = unit.functions.get(callee.index()).ok_or_else(|| {
                    imported_error(format!("f{} calls missing f{}", function.id.0, callee.0))
                })?;
                let body = lower_function(
                    unit,
                    callee_definition,
                    functions,
                    call_arities,
                    Some(&arguments),
                    environment,
                    Some(this_value),
                    Some(callee_value),
                    state,
                    call_depth + 1,
                )?;
                writes.extend(body.writes);
                if let Some(thrown) = body.abrupt {
                    if let Some(target) =
                        static_exception_target(function, instruction, &instruction_indices)?
                    {
                        pending_exception = Some(thrown);
                        instruction_index = target;
                        continue;
                    }
                    abrupt = Some(thrown);
                    break;
                }
                let value = body.result.unwrap_or(RegisterValue::Undefined);
                registers.insert(destination, value.clone());
                if descriptor.opcode == "op_tail_call_varargs" {
                    result = Some(value);
                    break;
                }
            }
            "op_construct_varargs" | "op_super_construct_varargs" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee_value =
                    read_register(function, &registers, signed_operand(instruction, "callee")?)?;
                let new_target = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "thisValue")?,
                )?;
                let argument_list = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "arguments")?,
                )?;
                let first = usize::try_from(signed_operand(instruction, "firstVarArg")?)
                    .map_err(|_| unsupported(function, instruction, descriptor.opcode))?;
                let arguments = static_argument_values(&state.heap, &argument_list, first)?;
                let RegisterValue::Function {
                    construct: Some(callee),
                    environment,
                    ..
                } = callee_value.clone()
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let callee_definition = unit.functions.get(callee.index()).ok_or_else(|| {
                    imported_error(format!(
                        "f{} constructs missing f{}",
                        function.id.0, callee.0
                    ))
                })?;
                let body = lower_function(
                    unit,
                    callee_definition,
                    functions,
                    call_arities,
                    Some(&arguments),
                    environment,
                    Some(new_target),
                    Some(callee_value),
                    state,
                    call_depth + 1,
                )?;
                writes.extend(body.writes);
                if let Some(thrown) = body.abrupt {
                    if let Some(target) =
                        static_exception_target(function, instruction, &instruction_indices)?
                    {
                        pending_exception = Some(thrown);
                        instruction_index = target;
                        continue;
                    }
                    abrupt = Some(thrown);
                    break;
                }
                registers.insert(destination, body.result.unwrap_or(RegisterValue::Undefined));
            }
            "op_call" | "op_tail_call" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee_register = signed_operand(instruction, "callee")?;
                let this_value = call_this_value(function, instruction, &registers)?;
                let callee_value = read_register(function, &registers, callee_register)?;
                if matches!(
                    callee_value,
                    RegisterValue::Builtin(Builtin::CreatePrivateSymbol)
                ) {
                    let arguments = call_register_values(function, instruction, &registers)?;
                    let [description] = arguments.as_slice() else {
                        return Err(unsupported(function, instruction, descriptor.opcode));
                    };
                    let description = match description {
                        RegisterValue::String(description) => description.clone(),
                        RegisterValue::Boolean(true) => "#private-brand".into(),
                        _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                    };
                    let identity = state.allocate_private_name_identity()?;
                    registers.insert(
                        destination,
                        RegisterValue::PrivateName {
                            identity,
                            description,
                        },
                    );
                    instruction_index += 1;
                    continue;
                }
                let RegisterValue::Function {
                    call: callee,
                    environment,
                    ..
                } = callee_value.clone()
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let arguments = call_register_values(function, instruction, &registers)?;
                let scalar_arguments = arguments
                    .iter()
                    .map(|value| match value {
                        RegisterValue::Scalar(expression) => Some(expression.clone()),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                let value = if environment.is_none()
                    && let Some(callee_function) = functions.get(&callee)
                    && let Some(arguments) = scalar_arguments
                    && arguments.len() == callee_function.parameter_count
                {
                    let expression = ScalarExpression::Call {
                        function: callee,
                        arguments,
                    };
                    match callee_function.result_kind {
                        ScalarKind::Integer => RegisterValue::Scalar(expression),
                        ScalarKind::Boolean => RegisterValue::BooleanScalar(expression),
                    }
                } else {
                    let callee_definition =
                        unit.functions.get(callee.index()).ok_or_else(|| {
                            imported_error(format!(
                                "f{} calls missing f{}",
                                function.id.0, callee.0
                            ))
                        })?;
                    let body = lower_function(
                        unit,
                        callee_definition,
                        functions,
                        call_arities,
                        Some(&arguments),
                        environment,
                        Some(this_value),
                        Some(callee_value),
                        state,
                        call_depth + 1,
                    )?;
                    writes.extend(body.writes);
                    if let Some(thrown) = body.abrupt {
                        if let Some(target) =
                            static_exception_target(function, instruction, &instruction_indices)?
                        {
                            pending_exception = Some(thrown);
                            instruction_index = target;
                            continue;
                        }
                        abrupt = Some(thrown);
                        break;
                    }
                    body.result.unwrap_or(RegisterValue::Undefined)
                };
                registers.insert(destination, value.clone());
                if descriptor.opcode == "op_tail_call" {
                    result = Some(value);
                    break;
                }
            }
            "op_construct" | "op_super_construct" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee_value =
                    read_register(function, &registers, signed_operand(instruction, "callee")?)?;
                let RegisterValue::Function {
                    construct: Some(callee),
                    environment,
                    ..
                } = callee_value.clone()
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let arguments = call_register_values(function, instruction, &registers)?;
                let new_target = if descriptor.opcode == "op_super_construct" {
                    call_this_value(function, instruction, &registers)?
                } else {
                    callee_value.clone()
                };
                let callee_definition = unit.functions.get(callee.index()).ok_or_else(|| {
                    imported_error(format!(
                        "f{} constructs missing f{}",
                        function.id.0, callee.0
                    ))
                })?;
                let body = lower_function(
                    unit,
                    callee_definition,
                    functions,
                    call_arities,
                    Some(&arguments),
                    environment,
                    Some(new_target),
                    Some(callee_value),
                    state,
                    call_depth + 1,
                )?;
                writes.extend(body.writes);
                if let Some(thrown) = body.abrupt {
                    if let Some(target) =
                        static_exception_target(function, instruction, &instruction_indices)?
                    {
                        pending_exception = Some(thrown);
                        instruction_index = target;
                        continue;
                    }
                    abrupt = Some(thrown);
                    break;
                }
                registers.insert(destination, body.result.unwrap_or(RegisterValue::Undefined));
            }
            "op_call_ignore_result" => {
                let callee = signed_operand(instruction, "callee")?;
                let callee_value = read_register(function, &registers, callee)?;
                match callee_value.clone() {
                    RegisterValue::ConsoleLog => {
                        let mut arguments =
                            call_register_values(function, instruction, &registers)?;
                        if arguments.len() > 1 {
                            return Err(imported_error(
                                "imported scalar console.log accepts at most one argument",
                            ));
                        }
                        writes.push(arguments.pop().unwrap_or(RegisterValue::ConsoleNoArgument));
                    }
                    RegisterValue::Builtin(Builtin::SetPrototypeDirectOrThrow) => {
                        let base = call_this_value(function, instruction, &registers)?;
                        let arguments = call_register_values(function, instruction, &registers)?;
                        let [prototype] = arguments.as_slice() else {
                            return Err(unsupported(function, instruction, descriptor.opcode));
                        };
                        static_set_prototype(&mut state.heap, &base, prototype)?;
                    }
                    RegisterValue::Function {
                        call: callee,
                        environment,
                        ..
                    } => {
                        let this_value = call_this_value(function, instruction, &registers)?;
                        let arguments = call_register_values(function, instruction, &registers)?;
                        let callee_definition =
                            unit.functions.get(callee.index()).ok_or_else(|| {
                                imported_error(format!(
                                    "f{} calls missing f{}",
                                    function.id.0, callee.0
                                ))
                            })?;
                        let body = lower_function(
                            unit,
                            callee_definition,
                            functions,
                            call_arities,
                            Some(&arguments),
                            environment,
                            Some(this_value),
                            Some(callee_value),
                            state,
                            call_depth + 1,
                        )?;
                        writes.extend(body.writes);
                        if let Some(thrown) = body.abrupt {
                            if let Some(target) = static_exception_target(
                                function,
                                instruction,
                                &instruction_indices,
                            )? {
                                pending_exception = Some(thrown);
                                instruction_index = target;
                                continue;
                            }
                            abrupt = Some(thrown);
                            break;
                        }
                    }
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                }
            }
            "op_yield" => {
                let _yield_point = unsigned_operand(instruction, "yieldPoint")?;
                result = Some(read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "argument")?,
                )?);
                break;
            }
            "op_ret" => {
                let value = signed_operand(instruction, "value")?;
                result = Some(read_register(function, &registers, value)?);
                break;
            }
            _ => return Err(unsupported(function, instruction, descriptor.opcode)),
        }
        instruction_index += 1;
    }
    Ok(LoweredBody {
        result,
        writes,
        abrupt,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_static_accessor(
    unit: &OwnedVisitorUnit,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    call_arities: &BTreeMap<FunctionId, usize>,
    callee: RegisterValue,
    receiver: RegisterValue,
    arguments: &[RegisterValue],
    state: &mut StaticExecutionState,
    call_depth: usize,
) -> Result<RegisterValue, LlvmError> {
    let RegisterValue::Function {
        call, environment, ..
    } = callee.clone()
    else {
        return Err(imported_error("static accessor is not callable"));
    };
    let definition = unit
        .functions
        .get(call.index())
        .ok_or_else(|| imported_error(format!("static accessor references missing f{}", call.0)))?;
    let body = lower_function(
        unit,
        definition,
        functions,
        call_arities,
        Some(arguments),
        environment,
        Some(receiver),
        Some(callee),
        state,
        call_depth + 1,
    )?;
    if !body.writes.is_empty() {
        return Err(imported_error(
            "static accessor performs an output side effect",
        ));
    }
    if body.abrupt.is_some() {
        return Err(imported_error("static accessor completes abruptly"));
    }
    Ok(body.result.unwrap_or(RegisterValue::Undefined))
}

#[allow(clippy::too_many_arguments)]
fn resolve_static_property_read(
    unit: &OwnedVisitorUnit,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    call_arities: &BTreeMap<FunctionId, usize>,
    value: RegisterValue,
    receiver: RegisterValue,
    state: &mut StaticExecutionState,
    call_depth: usize,
) -> Result<RegisterValue, LlvmError> {
    match value {
        RegisterValue::AccessorGetter(getter) => execute_static_accessor(
            unit,
            functions,
            call_arities,
            *getter,
            receiver,
            &[],
            state,
            call_depth,
        ),
        value => Ok(value),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_static_property_assignment(
    unit: &OwnedVisitorUnit,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    call_arities: &BTreeMap<FunctionId, usize>,
    assignment: StaticPropertyAssignment,
    receiver: RegisterValue,
    value: RegisterValue,
    state: &mut StaticExecutionState,
    call_depth: usize,
) -> Result<(), LlvmError> {
    if let StaticPropertyAssignment::CallSetter(setter) = assignment {
        execute_static_accessor(
            unit,
            functions,
            call_arities,
            setter,
            receiver,
            &[value],
            state,
            call_depth,
        )?;
    }
    Ok(())
}

fn static_exception_target(
    function: &VisitorFunction,
    instruction: &VisitorInstruction,
    instruction_indices: &BTreeMap<u32, usize>,
) -> Result<Option<usize>, LlvmError> {
    let Some(handler) = function.exception_handlers.iter().find(|handler| {
        handler.start <= instruction.byte_offset && instruction.byte_offset < handler.end
    }) else {
        return Ok(None);
    };
    instruction_indices
        .get(&handler.target)
        .copied()
        .map(Some)
        .ok_or_else(|| imported_error("exception handler target is not an instruction"))
}

fn branch_target_index(
    instruction: &VisitorInstruction,
    instruction_indices: &BTreeMap<u32, usize>,
) -> Result<usize, LlvmError> {
    relative_target_index(
        instruction,
        signed_operand(instruction, "targetLabel")?,
        instruction_indices,
    )
}

fn relative_target_index(
    instruction: &VisitorInstruction,
    relative_offset: impl Into<i64>,
    instruction_indices: &BTreeMap<u32, usize>,
) -> Result<usize, LlvmError> {
    let target = i64::from(instruction.byte_offset)
        .checked_add(relative_offset.into())
        .and_then(|target| u32::try_from(target).ok())
        .ok_or_else(|| imported_error("branch target leaves the instruction stream"))?;
    instruction_indices
        .get(&target)
        .copied()
        .ok_or_else(|| imported_error(format!("branch target {target} is not an instruction")))
}

fn simple_switch_offset(
    function: &VisitorFunction,
    table_index: usize,
    key: Option<i32>,
) -> Result<i32, LlvmError> {
    let table = function
        .simple_switch_tables
        .get(table_index)
        .ok_or_else(|| imported_error(format!("missing simple switch table {table_index}")))?;
    let branch_offset = key.and_then(|key| {
        if table.is_list {
            table
                .branch_offsets
                .chunks_exact(2)
                .find_map(|pair| (pair[0] == key).then_some(pair[1]))
        } else {
            let index = i64::from(key) - i64::from(table.minimum);
            usize::try_from(index)
                .ok()
                .and_then(|index| table.branch_offsets.get(index).copied())
                .filter(|offset| *offset != 0)
        }
    });
    Ok(branch_offset.unwrap_or(table.default_offset))
}

fn string_switch_offset(
    function: &VisitorFunction,
    table_index: usize,
    key: &str,
) -> Result<i32, LlvmError> {
    let table = function
        .string_switch_tables
        .get(table_index)
        .ok_or_else(|| imported_error(format!("missing string switch table {table_index}")))?;
    Ok(table
        .entries
        .iter()
        .find_map(|entry| source_text_equals_str(&entry.key, key).then_some(entry.branch_offset))
        .unwrap_or(table.default_offset))
}

fn known_switch_string(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<Option<String>, LlvmError> {
    match value {
        RegisterValue::String(value) => Ok(Some(value.to_string())),
        RegisterValue::Concatenation(_) => {
            let mut output = String::new();
            append_js_string(value, functions, &mut output)?;
            Ok(Some(output))
        }
        _ => Ok(None),
    }
}

fn known_truthiness(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<Option<bool>, LlvmError> {
    let result = match value {
        RegisterValue::Scalar(expression) | RegisterValue::BooleanScalar(expression)
            if expression_is_closed(expression) =>
        {
            Some(evaluate(expression, functions, &[], 0)? != 0)
        }
        RegisterValue::Boolean(value) => Some(*value),
        RegisterValue::Undefined
        | RegisterValue::Null
        | RegisterValue::NaN
        | RegisterValue::NegativeZero => Some(false),
        RegisterValue::String(value) => Some(!value.is_empty()),
        RegisterValue::BigInt { magnitude_be, .. } => Some(!magnitude_be.is_empty()),
        RegisterValue::PositiveInfinity
        | RegisterValue::NegativeInfinity
        | RegisterValue::Function { .. }
        | RegisterValue::RegExp { .. }
        | RegisterValue::Object(_)
        | RegisterValue::Array(_)
        | RegisterValue::ConsoleObject
        | RegisterValue::ConsoleLog
        | RegisterValue::Builtin(_)
        | RegisterValue::PrivateName { .. }
        | RegisterValue::Arguments { .. }
        | RegisterValue::ScopedArguments { .. }
        | RegisterValue::InternalObject { .. }
        | RegisterValue::AsyncFromSyncIteratorNext
        | RegisterValue::AsyncFromSyncIterator { .. }
        | RegisterValue::FulfilledPromise(_)
        | RegisterValue::GlobalObject
        | RegisterValue::ExceptionObject(_)
        | RegisterValue::Error { .. } => Some(true),
        _ => None,
    };
    Ok(result)
}

fn expression_is_closed(expression: &ScalarExpression) -> bool {
    match expression {
        ScalarExpression::Integer(_) => true,
        ScalarExpression::Parameter(_) => false,
        ScalarExpression::Negate(value)
        | ScalarExpression::BitNot(value)
        | ScalarExpression::Unsigned(value)
        | ScalarExpression::LogicalNot(value) => expression_is_closed(value),
        ScalarExpression::Add(left, right)
        | ScalarExpression::Subtract(left, right)
        | ScalarExpression::Multiply(left, right)
        | ScalarExpression::Divide(left, right)
        | ScalarExpression::Remainder(left, right)
        | ScalarExpression::Power(left, right)
        | ScalarExpression::BitAnd(left, right)
        | ScalarExpression::BitOr(left, right)
        | ScalarExpression::BitXor(left, right)
        | ScalarExpression::LeftShift(left, right)
        | ScalarExpression::RightShift(left, right)
        | ScalarExpression::UnsignedRightShift(left, right)
        | ScalarExpression::Equal(left, right)
        | ScalarExpression::NotEqual(left, right)
        | ScalarExpression::Less(left, right)
        | ScalarExpression::LessEqual(left, right)
        | ScalarExpression::Greater(left, right)
        | ScalarExpression::GreaterEqual(left, right)
        | ScalarExpression::Below(left, right)
        | ScalarExpression::BelowEqual(left, right) => {
            expression_is_closed(left) && expression_is_closed(right)
        }
        ScalarExpression::Call { arguments, .. } => arguments.iter().all(expression_is_closed),
    }
}

fn known_branch_comparison(
    opcode: &str,
    left: &RegisterValue,
    right: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<Option<bool>, LlvmError> {
    if matches!(opcode, "op_jstricteq" | "op_jnstricteq")
        && let Some(equal) = known_strict_equality(left, right, functions)?
    {
        return Ok(Some(if opcode == "op_jstricteq" {
            equal
        } else {
            !equal
        }));
    }
    let (RegisterValue::Scalar(left), RegisterValue::Scalar(right)) = (left, right) else {
        return Ok(None);
    };
    if !expression_is_closed(left) || !expression_is_closed(right) {
        return Ok(None);
    }
    let left = evaluate(left, functions, &[], 0)?;
    let right = evaluate(right, functions, &[], 0)?;
    let result = match opcode {
        "op_jeq" | "op_jstricteq" => left == right,
        "op_jneq" | "op_jnstricteq" => left != right,
        "op_jless" => left < right,
        "op_jlesseq" => left <= right,
        "op_jgreater" => left > right,
        "op_jgreatereq" => left >= right,
        "op_jnless" => left >= right,
        "op_jnlesseq" => left > right,
        "op_jngreater" => left <= right,
        "op_jngreatereq" => left < right,
        "op_jbelow" => to_uint32(left) < to_uint32(right),
        "op_jbeloweq" => to_uint32(left) <= to_uint32(right),
        _ => return Ok(None),
    };
    Ok(Some(result))
}

fn known_strict_equality(
    left: &RegisterValue,
    right: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<Option<bool>, LlvmError> {
    let result = match (left, right) {
        (RegisterValue::Undefined, RegisterValue::Undefined)
        | (RegisterValue::Null, RegisterValue::Null)
        | (RegisterValue::NegativeZero, RegisterValue::NegativeZero)
        | (RegisterValue::PositiveInfinity, RegisterValue::PositiveInfinity)
        | (RegisterValue::NegativeInfinity, RegisterValue::NegativeInfinity) => Some(true),
        (RegisterValue::NaN, _) | (_, RegisterValue::NaN) => Some(false),
        (RegisterValue::Boolean(left), RegisterValue::Boolean(right)) => Some(left == right),
        (
            RegisterValue::BigInt {
                negative: left_negative,
                magnitude_be: left_magnitude,
            },
            RegisterValue::BigInt {
                negative: right_negative,
                magnitude_be: right_magnitude,
            },
        ) => Some(left_negative == right_negative && left_magnitude == right_magnitude),
        (RegisterValue::String(left), RegisterValue::String(right)) => Some(left == right),
        (
            RegisterValue::PrivateName { identity: left, .. },
            RegisterValue::PrivateName {
                identity: right, ..
            },
        ) => Some(left == right),
        (
            RegisterValue::Function { identity: left, .. },
            RegisterValue::Function {
                identity: right, ..
            },
        ) => Some(left == right),
        (
            RegisterValue::RegExp { identity: left, .. },
            RegisterValue::RegExp {
                identity: right, ..
            },
        ) => Some(left == right),
        (RegisterValue::Object(left), RegisterValue::Object(right))
        | (RegisterValue::Array(left), RegisterValue::Array(right)) => Some(left == right),
        (
            RegisterValue::InternalObject { identity: left, .. },
            RegisterValue::InternalObject {
                identity: right, ..
            },
        ) => Some(left == right),
        (RegisterValue::Builtin(left), RegisterValue::Builtin(right)) => Some(left == right),
        (RegisterValue::GlobalObject, RegisterValue::GlobalObject) => Some(true),
        (RegisterValue::Scalar(left), RegisterValue::Scalar(right))
            if expression_is_closed(left) && expression_is_closed(right) =>
        {
            Some(evaluate(left, functions, &[], 0)? == evaluate(right, functions, &[], 0)?)
        }
        (left, right)
            if known_js_type(left)
                .zip(known_js_type(right))
                .is_some_and(|(left, right)| left != right) =>
        {
            Some(false)
        }
        _ => None,
    };
    Ok(result)
}

fn known_js_type(value: &RegisterValue) -> Option<&'static str> {
    match value {
        RegisterValue::Undefined => Some("undefined"),
        RegisterValue::Null => Some("null"),
        RegisterValue::Boolean(_) | RegisterValue::BooleanScalar(_) => Some("boolean"),
        RegisterValue::Scalar(_)
        | RegisterValue::NaN
        | RegisterValue::NegativeZero
        | RegisterValue::PositiveInfinity
        | RegisterValue::NegativeInfinity => Some("number"),
        RegisterValue::String(_) | RegisterValue::Concatenation(_) => Some("string"),
        RegisterValue::BigInt { .. } => Some("bigint"),
        RegisterValue::PrivateName { .. } => Some("symbol"),
        RegisterValue::Function { .. }
        | RegisterValue::ConsoleLog
        | RegisterValue::AsyncFromSyncIteratorNext => Some("function"),
        RegisterValue::Builtin(builtin) if builtin_is_callable(*builtin) => Some("function"),
        RegisterValue::Builtin(Builtin::SentinelString) => Some("string"),
        RegisterValue::Builtin(Builtin::EmptyPropertyNameEnumerator) => Some("object"),
        RegisterValue::RegExp { .. }
        | RegisterValue::Object(_)
        | RegisterValue::Array(_)
        | RegisterValue::Arguments { .. }
        | RegisterValue::ScopedArguments { .. }
        | RegisterValue::InternalObject { .. }
        | RegisterValue::AsyncFromSyncIterator { .. }
        | RegisterValue::FulfilledPromise(_)
        | RegisterValue::GlobalObject
        | RegisterValue::ConsoleObject
        | RegisterValue::ExceptionObject(_)
        | RegisterValue::Error { .. } => Some("object"),
        _ => None,
    }
}

fn known_pointer_equality(left: &RegisterValue, right: &RegisterValue) -> Option<bool> {
    match (left, right) {
        (RegisterValue::Builtin(left), RegisterValue::Builtin(right)) => Some(left == right),
        (
            RegisterValue::Enumerator { .. },
            RegisterValue::Builtin(Builtin::EmptyPropertyNameEnumerator),
        )
        | (
            RegisterValue::Builtin(Builtin::EmptyPropertyNameEnumerator),
            RegisterValue::Enumerator { .. },
        )
        | (RegisterValue::String(_), RegisterValue::Builtin(Builtin::SentinelString))
        | (RegisterValue::Builtin(Builtin::SentinelString), RegisterValue::String(_)) => {
            Some(false)
        }
        _ => None,
    }
}

fn call_register_values(
    function: &VisitorFunction,
    instruction: &VisitorInstruction,
    registers: &BTreeMap<i64, RegisterValue>,
) -> Result<Vec<RegisterValue>, LlvmError> {
    let count = usize::try_from(unsigned_operand(instruction, "argc")?)
        .map_err(|_| imported_error("call argument count does not fit usize"))?;
    if count == 0 {
        return Err(imported_error("JSC call has no this argument"));
    }
    let argv = i64::try_from(unsigned_operand(instruction, "argv")?)
        .map_err(|_| imported_error("call argv does not fit i64"))?;
    let this_register = -argv + i64::from(function.call_frame_this_argument_register);
    (1..count)
        .map(|index| read_register(function, registers, this_register + index as i64))
        .collect()
}

fn static_argument_values(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    arguments: &RegisterValue,
    first: usize,
) -> Result<Vec<RegisterValue>, LlvmError> {
    let values = match arguments {
        RegisterValue::Arguments { values, .. } => values.borrow().clone(),
        RegisterValue::ScopedArguments {
            environment,
            length,
        } => (0..length.get())
            .map(|index| {
                environment
                    .borrow()
                    .scoped_argument_values
                    .get(&index)
                    .cloned()
                    .unwrap_or(RegisterValue::Undefined)
            })
            .collect(),
        RegisterValue::Spread(values) => values.clone(),
        RegisterValue::Array(id) => {
            let Some(StaticHeapEntry::Array {
                elements, length, ..
            }) = heap.get(id)
            else {
                return Err(imported_error(
                    "varargs array references a missing static heap entry",
                ));
            };
            (0..*length)
                .map(|index| {
                    elements
                        .get(&index)
                        .map(StaticProperty::read)
                        .unwrap_or(RegisterValue::Undefined)
                })
                .collect()
        }
        _ => {
            return Err(imported_error(
                "varargs source is not a static argument list",
            ));
        }
    };
    values
        .get(first..)
        .map(<[RegisterValue]>::to_vec)
        .ok_or_else(|| imported_error("varargs first argument exceeds the argument list"))
}

fn call_this_value(
    function: &VisitorFunction,
    instruction: &VisitorInstruction,
    registers: &BTreeMap<i64, RegisterValue>,
) -> Result<RegisterValue, LlvmError> {
    let count = usize::try_from(unsigned_operand(instruction, "argc")?)
        .map_err(|_| imported_error("call argument count does not fit usize"))?;
    if count == 0 {
        return Err(imported_error("JSC call has no this argument"));
    }
    let argv = i64::try_from(unsigned_operand(instruction, "argv")?)
        .map_err(|_| imported_error("call argv does not fit i64"))?;
    let this_register = -argv + i64::from(function.call_frame_this_argument_register);
    read_register(function, registers, this_register)
}

fn read_register(
    function: &VisitorFunction,
    registers: &BTreeMap<i64, RegisterValue>,
    register: i64,
) -> Result<RegisterValue, LlvmError> {
    if register >= FIRST_CONSTANT_REGISTER_INDEX {
        let index = usize::try_from(register - FIRST_CONSTANT_REGISTER_INDEX)
            .map_err(|_| imported_error("constant register index does not fit usize"))?;
        let constant = function.constants.get(index).ok_or_else(|| {
            imported_error(format!(
                "f{} references missing constant k{index}",
                function.id.0
            ))
        })?;
        return register_constant_value(function, &constant.value);
    }
    registers.get(&register).cloned().ok_or_else(|| {
        imported_error(format!(
            "f{} reads undefined virtual register {register}",
            function.id.0
        ))
    })
}

fn register_constant_value(
    function: &VisitorFunction,
    constant: &VisitorConstantValue,
) -> Result<RegisterValue, LlvmError> {
    Ok(match constant {
        VisitorConstantValue::Int32(value) => {
            RegisterValue::Scalar(ScalarExpression::Integer(i64::from(*value)))
        }
        VisitorConstantValue::Float64Bits(bits) => {
            let value = f64::from_bits(*bits);
            if value.is_nan() {
                RegisterValue::NaN
            } else if value == f64::INFINITY {
                RegisterValue::PositiveInfinity
            } else if value == f64::NEG_INFINITY {
                RegisterValue::NegativeInfinity
            } else if value == 0.0 && value.is_sign_negative() {
                RegisterValue::NegativeZero
            } else if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER as f64 {
                RegisterValue::Scalar(ScalarExpression::Integer(value as i64))
            } else {
                return Err(imported_error(format!(
                    "f{} uses a non-integral scalar constant",
                    function.id.0
                )));
            }
        }
        VisitorConstantValue::Undefined => RegisterValue::Undefined,
        VisitorConstantValue::Null => RegisterValue::Null,
        VisitorConstantValue::Boolean(value) => RegisterValue::Boolean(*value),
        VisitorConstantValue::String(value) => {
            RegisterValue::String(source_text_to_string(value)?.into_boxed_str())
        }
        VisitorConstantValue::BigInt {
            negative,
            magnitude_be,
        } => RegisterValue::BigInt {
            negative: *negative,
            magnitude_be: magnitude_be.clone(),
        },
        VisitorConstantValue::RegExp { pattern, flags } => RegisterValue::RegExpTemplate {
            pattern: pattern.clone(),
            flags: *flags,
        },
        VisitorConstantValue::ImmutableArray { elements, .. } => RegisterValue::ArrayTemplate(
            elements
                .iter()
                .map(|value| register_constant_value(function, value))
                .collect::<Result<_, _>>()?,
        ),
        VisitorConstantValue::LinkTimeConstant(name) => match name.as_ref() {
            "Array" => RegisterValue::Builtin(Builtin::Array),
            "createPrivateSymbol" => RegisterValue::Builtin(Builtin::CreatePrivateSymbol),
            "setPrototypeDirectOrThrow" => {
                RegisterValue::Builtin(Builtin::SetPrototypeDirectOrThrow)
            }
            "emptyPropertyNameEnumerator" => {
                RegisterValue::Builtin(Builtin::EmptyPropertyNameEnumerator)
            }
            "Object" => RegisterValue::Builtin(Builtin::Object),
            "hasOwnPropertyFunction" => RegisterValue::Builtin(Builtin::HasOwnPropertyFunction),
            "sentinelString" => RegisterValue::Builtin(Builtin::SentinelString),
            _ => RegisterValue::Opaque,
        },
        VisitorConstantValue::Empty => RegisterValue::Empty,
        VisitorConstantValue::UnimplementedCell(_) => RegisterValue::Opaque,
    })
}

fn register_value_from_static_application(
    value: crate::application::StaticValue,
) -> Result<RegisterValue, LlvmError> {
    use crate::application::StaticValue;

    Ok(match value {
        StaticValue::Number(value) if value.is_nan() => RegisterValue::NaN,
        StaticValue::Number(value) if value == f64::INFINITY => RegisterValue::PositiveInfinity,
        StaticValue::Number(value) if value == f64::NEG_INFINITY => RegisterValue::NegativeInfinity,
        StaticValue::Number(value) if value == 0.0 && value.is_sign_negative() => {
            RegisterValue::NegativeZero
        }
        StaticValue::Number(value)
            if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER as f64 =>
        {
            RegisterValue::Scalar(ScalarExpression::Integer(value as i64))
        }
        StaticValue::Number(_) => {
            return Err(imported_error(
                "literal eval result leaves the admitted static Number domain",
            ));
        }
        StaticValue::String(value) => RegisterValue::String(value),
        StaticValue::Boolean(value) => RegisterValue::Boolean(value),
        StaticValue::Null => RegisterValue::Null,
        StaticValue::Undefined => RegisterValue::Undefined,
    })
}

fn scalar_register(
    function: &VisitorFunction,
    registers: &BTreeMap<i64, RegisterValue>,
    register: i64,
) -> Result<ScalarExpression, LlvmError> {
    let RegisterValue::Scalar(expression) = read_register(function, registers, register)? else {
        return Err(imported_error(format!(
            "f{} expected an integer in virtual register {register}",
            function.id.0
        )));
    };
    Ok(expression)
}

fn known_unary_predicate(opcode: &str, value: &RegisterValue) -> Option<bool> {
    let known_value = matches!(
        value,
        RegisterValue::Scalar(_)
            | RegisterValue::BooleanScalar(_)
            | RegisterValue::String(_)
            | RegisterValue::Concatenation(_)
            | RegisterValue::BigInt { .. }
            | RegisterValue::Function { .. }
            | RegisterValue::RegExp { .. }
            | RegisterValue::Object(_)
            | RegisterValue::Array(_)
            | RegisterValue::ConsoleObject
            | RegisterValue::ConsoleLog
            | RegisterValue::Builtin(_)
            | RegisterValue::PrivateName { .. }
            | RegisterValue::Arguments { .. }
            | RegisterValue::ScopedArguments { .. }
            | RegisterValue::InternalObject { .. }
            | RegisterValue::AsyncFromSyncIteratorNext
            | RegisterValue::AsyncFromSyncIterator { .. }
            | RegisterValue::FulfilledPromise(_)
            | RegisterValue::GlobalObject
            | RegisterValue::ExceptionObject(_)
            | RegisterValue::Error { .. }
            | RegisterValue::Undefined
            | RegisterValue::Null
            | RegisterValue::Boolean(_)
            | RegisterValue::NaN
            | RegisterValue::NegativeZero
            | RegisterValue::PositiveInfinity
            | RegisterValue::NegativeInfinity
    );
    match opcode {
        "op_is_empty" if matches!(value, RegisterValue::Empty) => Some(true),
        "op_is_empty" if known_value => Some(false),
        _ if !known_value => None,
        "op_eq_null" => Some(matches!(
            value,
            RegisterValue::Undefined | RegisterValue::Null
        )),
        "op_neq_null" => Some(!matches!(
            value,
            RegisterValue::Undefined | RegisterValue::Null
        )),
        "op_typeof_is_undefined" => Some(matches!(value, RegisterValue::Undefined)),
        "op_typeof_is_object" => Some(matches!(
            value,
            RegisterValue::Null
                | RegisterValue::RegExp { .. }
                | RegisterValue::Object(_)
                | RegisterValue::Array(_)
                | RegisterValue::Arguments { .. }
                | RegisterValue::ScopedArguments { .. }
                | RegisterValue::InternalObject { .. }
                | RegisterValue::AsyncFromSyncIterator { .. }
                | RegisterValue::FulfilledPromise(_)
                | RegisterValue::GlobalObject
                | RegisterValue::ConsoleObject
                | RegisterValue::ExceptionObject(_)
                | RegisterValue::Error { .. }
        )),
        "op_typeof_is_function" => Some(
            matches!(
                value,
                RegisterValue::Function { .. }
                    | RegisterValue::ConsoleLog
                    | RegisterValue::AsyncFromSyncIteratorNext
            ) || matches!(value, RegisterValue::Builtin(builtin) if builtin_is_callable(*builtin)),
        ),
        "op_is_undefined_or_null" => Some(matches!(
            value,
            RegisterValue::Undefined | RegisterValue::Null
        )),
        "op_is_boolean" => Some(matches!(
            value,
            RegisterValue::Boolean(_) | RegisterValue::BooleanScalar(_)
        )),
        "op_is_number" => Some(matches!(
            value,
            RegisterValue::Scalar(_)
                | RegisterValue::NaN
                | RegisterValue::NegativeZero
                | RegisterValue::PositiveInfinity
                | RegisterValue::NegativeInfinity
        )),
        "op_is_big_int" => Some(matches!(value, RegisterValue::BigInt { .. })),
        "op_is_object" => Some(
            matches!(
                value,
                RegisterValue::Function { .. }
                    | RegisterValue::RegExp { .. }
                    | RegisterValue::Object(_)
                    | RegisterValue::Array(_)
                    | RegisterValue::Arguments { .. }
                    | RegisterValue::ScopedArguments { .. }
                    | RegisterValue::InternalObject { .. }
                    | RegisterValue::AsyncFromSyncIterator { .. }
                    | RegisterValue::FulfilledPromise(_)
                    | RegisterValue::GlobalObject
                    | RegisterValue::ConsoleObject
                    | RegisterValue::ConsoleLog
                    | RegisterValue::AsyncFromSyncIteratorNext
                    | RegisterValue::ExceptionObject(_)
                    | RegisterValue::Error { .. }
            ) || matches!(value, RegisterValue::Builtin(builtin) if builtin_is_callable(*builtin)),
        ),
        "op_is_callable" => Some(
            matches!(
                value,
                RegisterValue::Function { .. }
                    | RegisterValue::ConsoleLog
                    | RegisterValue::AsyncFromSyncIteratorNext
            ) || matches!(value, RegisterValue::Builtin(builtin) if builtin_is_callable(*builtin)),
        ),
        "op_is_constructor" => match value {
            RegisterValue::Builtin(builtin) => Some(builtin_is_constructor(*builtin)),
            RegisterValue::Function { construct, .. } => Some(construct.is_some()),
            RegisterValue::ConsoleLog => Some(false),
            _ => Some(false),
        },
        _ => None,
    }
}

fn known_cell_type(value: &RegisterValue) -> Option<u8> {
    match value {
        RegisterValue::String(_) | RegisterValue::Concatenation(_) => Some(2),
        RegisterValue::BigInt { .. } => Some(3),
        RegisterValue::PrivateName { .. } => Some(4),
        RegisterValue::Function { .. } => Some(36),
        RegisterValue::ConsoleLog
        | RegisterValue::ArrayIteratorMethod
        | RegisterValue::ArrayIteratorNext
        | RegisterValue::AsyncFromSyncIteratorNext
        | RegisterValue::Builtin(_) => Some(37),
        RegisterValue::Error { .. } | RegisterValue::ExceptionObject(_) => Some(41),
        RegisterValue::Arguments { kind, .. } => Some(match kind {
            ArgumentsKind::Direct => 43,
            ArgumentsKind::Cloned => 45,
        }),
        RegisterValue::ScopedArguments { .. } => Some(44),
        RegisterValue::Array(_) | RegisterValue::ArrayTemplate(_) => Some(46),
        RegisterValue::GlobalObject => Some(62),
        RegisterValue::Environment(_) => Some(64),
        RegisterValue::RegExp { .. } | RegisterValue::RegExpTemplate { .. } => Some(72),
        RegisterValue::ArrayIterator { .. } => Some(78),
        RegisterValue::InternalObject { kind, .. } => Some(match kind {
            InternalObjectKind::Generator | InternalObjectKind::AsyncFunctionGenerator => 75,
            InternalObjectKind::AsyncGenerator => 77,
            InternalObjectKind::Promise => 87,
        }),
        RegisterValue::FulfilledPromise(_) => Some(87),
        RegisterValue::Object(_)
        | RegisterValue::AsyncFromSyncIterator { .. }
        | RegisterValue::ConsoleObject => Some(34),
        RegisterValue::Enumerator { .. } => Some(34),
        RegisterValue::Scalar(_)
        | RegisterValue::BooleanScalar(_)
        | RegisterValue::ConsoleScope
        | RegisterValue::NaNScope
        | RegisterValue::InfinityScope
        | RegisterValue::UndefinedScope
        | RegisterValue::BuiltinScope(_)
        | RegisterValue::AccessorGetter(_)
        | RegisterValue::ConsoleNoArgument
        | RegisterValue::Spread(_)
        | RegisterValue::Empty
        | RegisterValue::Undefined
        | RegisterValue::Null
        | RegisterValue::Boolean(_)
        | RegisterValue::NaN
        | RegisterValue::NegativeZero
        | RegisterValue::PositiveInfinity
        | RegisterValue::NegativeInfinity
        | RegisterValue::Opaque => None,
    }
}

fn known_typeof(value: &RegisterValue) -> Option<&'static str> {
    match value {
        RegisterValue::Undefined => Some("undefined"),
        RegisterValue::Boolean(_) | RegisterValue::BooleanScalar(_) => Some("boolean"),
        RegisterValue::Scalar(_)
        | RegisterValue::NaN
        | RegisterValue::NegativeZero
        | RegisterValue::PositiveInfinity
        | RegisterValue::NegativeInfinity => Some("number"),
        RegisterValue::String(_) | RegisterValue::Concatenation(_) => Some("string"),
        RegisterValue::BigInt { .. } => Some("bigint"),
        RegisterValue::PrivateName { .. } => Some("symbol"),
        RegisterValue::Function { .. }
        | RegisterValue::ConsoleLog
        | RegisterValue::AsyncFromSyncIteratorNext => Some("function"),
        RegisterValue::Builtin(builtin) if builtin_is_callable(*builtin) => Some("function"),
        RegisterValue::Builtin(Builtin::SentinelString) => Some("string"),
        RegisterValue::Builtin(Builtin::EmptyPropertyNameEnumerator) => Some("object"),
        RegisterValue::Null
        | RegisterValue::RegExp { .. }
        | RegisterValue::Object(_)
        | RegisterValue::Array(_)
        | RegisterValue::Arguments { .. }
        | RegisterValue::ScopedArguments { .. }
        | RegisterValue::InternalObject { .. }
        | RegisterValue::AsyncFromSyncIterator { .. }
        | RegisterValue::FulfilledPromise(_)
        | RegisterValue::GlobalObject
        | RegisterValue::ConsoleObject
        | RegisterValue::ExceptionObject(_)
        | RegisterValue::Error { .. } => Some("object"),
        _ => None,
    }
}

fn is_admitted_primitive(value: &RegisterValue) -> bool {
    matches!(
        value,
        RegisterValue::Scalar(_)
            | RegisterValue::BooleanScalar(_)
            | RegisterValue::String(_)
            | RegisterValue::Concatenation(_)
            | RegisterValue::BigInt { .. }
            | RegisterValue::Undefined
            | RegisterValue::Null
            | RegisterValue::Boolean(_)
            | RegisterValue::NaN
            | RegisterValue::NegativeZero
            | RegisterValue::PositiveInfinity
            | RegisterValue::NegativeInfinity
    )
}

fn source_text_to_string(value: &SourceText) -> Result<String, LlvmError> {
    match value {
        SourceText::Latin1(bytes) => Ok(bytes.iter().map(|byte| char::from(*byte)).collect()),
        SourceText::Utf16(code_units) => String::from_utf16(code_units)
            .map_err(|_| imported_error("string constant contains an unpaired UTF-16 surrogate")),
    }
}

fn source_text_equals_str(value: &SourceText, expected: &str) -> bool {
    match value {
        SourceText::Latin1(bytes) => expected
            .encode_utf16()
            .eq(bytes.iter().copied().map(u16::from)),
        SourceText::Utf16(code_units) => expected.encode_utf16().eq(code_units.iter().copied()),
    }
}

fn normalize_write(
    value: RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<NativeWrite, LlvmError> {
    if let RegisterValue::Scalar(expression) = value {
        evaluate(&expression, functions, &[], 0)?;
        return Ok(NativeWrite::Integer(expression));
    }
    if let RegisterValue::BooleanScalar(expression) = value {
        let value = evaluate(&expression, functions, &[], 0)?;
        if !matches!(value, 0 | 1) {
            return Err(imported_error(
                "boolean scalar is not normalized to zero or one",
            ));
        }
        return Ok(NativeWrite::Boolean(expression));
    }
    let mut text = String::new();
    append_console_string(&value, functions, &mut text)?;
    text.push('\n');
    Ok(NativeWrite::Text(text.into_bytes().into_boxed_slice()))
}

fn append_console_string(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    output: &mut String,
) -> Result<(), LlvmError> {
    match value {
        RegisterValue::Scalar(expression) => {
            let value = evaluate(expression, functions, &[], 0)?;
            write!(output, "{value}").unwrap();
        }
        RegisterValue::BooleanScalar(expression) => {
            let value = evaluate(expression, functions, &[], 0)?;
            match value {
                0 => output.push_str("false"),
                1 => output.push_str("true"),
                _ => {
                    return Err(imported_error(
                        "boolean scalar is not normalized to zero or one",
                    ));
                }
            }
        }
        RegisterValue::String(value) => output.push_str(value),
        RegisterValue::BigInt {
            negative,
            magnitude_be,
        } => {
            output.push_str(&bigint_decimal(*negative, magnitude_be));
            output.push('n');
        }
        RegisterValue::ConsoleNoArgument => {}
        RegisterValue::Concatenation(values) => {
            for value in values {
                append_js_string(value, functions, output)?;
            }
        }
        RegisterValue::Undefined => output.push_str("undefined"),
        RegisterValue::Null => output.push_str("null"),
        RegisterValue::Boolean(value) => output.push_str(if *value { "true" } else { "false" }),
        RegisterValue::NaN => output.push_str("NaN"),
        RegisterValue::NegativeZero => output.push_str("-0"),
        RegisterValue::PositiveInfinity => output.push_str("Infinity"),
        RegisterValue::NegativeInfinity => output.push_str("-Infinity"),
        _ => return Err(imported_error("console.log received a non-primitive value")),
    }
    Ok(())
}

fn append_js_string(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    output: &mut String,
) -> Result<(), LlvmError> {
    match value {
        RegisterValue::NegativeZero => output.push('0'),
        RegisterValue::BigInt {
            negative,
            magnitude_be,
        } => output.push_str(&bigint_decimal(*negative, magnitude_be)),
        RegisterValue::Concatenation(values) => {
            for value in values {
                append_js_string(value, functions, output)?;
            }
        }
        _ => append_console_string(value, functions, output)?,
    }
    Ok(())
}

fn bigint_decimal(negative: bool, magnitude_be: &[u8]) -> String {
    const RADIX: u64 = 1_000_000_000;
    if magnitude_be.is_empty() {
        return "0".into();
    }
    let mut limbs = vec![0_u32];
    for byte in magnitude_be {
        let mut carry = u64::from(*byte);
        for limb in &mut limbs {
            let value = u64::from(*limb) * 256 + carry;
            *limb = (value % RADIX) as u32;
            carry = value / RADIX;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
    }
    let mut output = if negative {
        String::from("-")
    } else {
        String::new()
    };
    let most_significant = limbs.pop().unwrap();
    write!(output, "{most_significant}").unwrap();
    for limb in limbs.iter().rev() {
        write!(output, "{limb:09}").unwrap();
    }
    output
}

fn child_function(
    unit: &OwnedVisitorUnit,
    parent: FunctionId,
    opcode: &str,
    index: u64,
    specialization: FunctionSpecialization,
) -> Result<FunctionId, LlvmError> {
    child_function_optional(unit, parent, opcode, index, specialization).ok_or_else(|| {
        imported_error(format!(
            "f{} references missing {specialization:?} child {index}",
            parent.0
        ))
    })
}

fn child_function_optional(
    unit: &OwnedVisitorUnit,
    parent: FunctionId,
    opcode: &str,
    index: u64,
    specialization: FunctionSpecialization,
) -> Option<FunctionId> {
    let relation_matches = |relation: &FunctionRelation| match (opcode, relation) {
        (
            "op_new_func"
            | "op_new_generator_func"
            | "op_new_async_func"
            | "op_new_async_generator_func",
            FunctionRelation::Declaration { index: actual },
        ) => u64::from(*actual) == index,
        (
            "op_new_func_exp"
            | "op_new_generator_func_exp"
            | "op_new_async_func_exp"
            | "op_new_async_generator_func_exp",
            FunctionRelation::Expression { index: actual },
        ) => u64::from(*actual) == index,
        _ => false,
    };
    unit.functions
        .iter()
        .find(|function| {
            function.parent == Some(parent)
                && relation_matches(&function.relation)
                && function.specialization == specialization
        })
        .map(|function| function.id)
}

fn resolve_environment(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    environment: &StaticEnvironmentRef,
    identifier: &str,
) -> Result<Option<StaticEnvironmentRef>, LlvmError> {
    let mut current = Some(environment.clone());
    while let Some(candidate) = current {
        let borrowed = candidate.borrow();
        if borrowed.bindings.contains_key(identifier) {
            drop(borrowed);
            return Ok(Some(candidate));
        }
        if let Some(object_scope) = &borrowed.object_scope {
            let id = match object_scope {
                RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
                RegisterValue::Function { heap_id, .. } => *heap_id,
                _ => {
                    return Err(imported_error(
                        "with environment references a non-object binding object",
                    ));
                }
            };
            if static_lookup_property_descriptor(
                heap,
                id,
                &StaticPropertyKey::Name(identifier.into()),
                0,
            )?
            .is_some()
            {
                drop(borrowed);
                return Ok(Some(candidate));
            }
        }
        current = borrowed.parent.clone();
    }
    Ok(None)
}

fn environment_binding(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    environment: &StaticEnvironmentRef,
    identifier: &str,
) -> Result<Option<RegisterValue>, LlvmError> {
    let Some(resolved) = resolve_environment(heap, environment, identifier)? else {
        return Ok(None);
    };
    let borrowed = resolved.borrow();
    if let Some(index) = borrowed.scoped_argument_names.get(identifier)
        && let Some(value) = borrowed.scoped_argument_values.get(index)
    {
        return Ok(Some(value.clone()));
    }
    if let Some(value) = borrowed.bindings.get(identifier) {
        return Ok(Some(value.clone()));
    }
    let object_scope = borrowed.object_scope.clone();
    drop(borrowed);
    object_scope
        .map(|object| {
            static_get_property(heap, &object, StaticPropertyKey::Name(identifier.into()))
        })
        .transpose()
}

fn identifier_is(
    function: &VisitorFunction,
    index: u64,
    expected: &[u8],
) -> Result<bool, LlvmError> {
    let index = usize::try_from(index)
        .map_err(|_| imported_error("identifier index does not fit usize"))?;
    let identifier = function.identifiers.get(index).ok_or_else(|| {
        imported_error(format!(
            "f{} references missing identifier {index}",
            function.id.0
        ))
    })?;
    Ok(matches!(identifier, SourceText::Latin1(bytes) if bytes.as_ref() == expected))
}

fn builtin_identifier(
    function: &VisitorFunction,
    index: u64,
) -> Result<Option<Builtin>, LlvmError> {
    if identifier_is(function, index, b"Array")? {
        Ok(Some(Builtin::Array))
    } else if identifier_is(function, index, b"Object")? {
        Ok(Some(Builtin::Object))
    } else if identifier_is(function, index, b"eval")? {
        Ok(Some(Builtin::Eval))
    } else {
        Ok(None)
    }
}

fn identifier_string(function: &VisitorFunction, index: u64) -> Result<Box<str>, LlvmError> {
    let index = usize::try_from(index)
        .map_err(|_| imported_error("identifier index does not fit usize"))?;
    let identifier = function.identifiers.get(index).ok_or_else(|| {
        imported_error(format!(
            "f{} references missing identifier {index}",
            function.id.0
        ))
    })?;
    Ok(source_text_to_string(identifier)?.into_boxed_str())
}

fn static_property_key(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<StaticPropertyKey, LlvmError> {
    match value {
        RegisterValue::Scalar(expression) => {
            let value = evaluate(expression, functions, &[], 0)?;
            if let Ok(index) = u32::try_from(value)
                && index != u32::MAX
            {
                Ok(StaticPropertyKey::Index(index))
            } else {
                Ok(StaticPropertyKey::Name(value.to_string().into_boxed_str()))
            }
        }
        RegisterValue::String(value) => Ok(value
            .parse::<u32>()
            .ok()
            .filter(|index| *index != u32::MAX && index.to_string().as_str() == value.as_ref())
            .map_or_else(
                || StaticPropertyKey::Name(value.clone()),
                StaticPropertyKey::Index,
            )),
        RegisterValue::NegativeZero => Ok(StaticPropertyKey::Index(0)),
        RegisterValue::Boolean(value) => Ok(StaticPropertyKey::Name(
            (if *value { "true" } else { "false" }).into(),
        )),
        RegisterValue::Null => Ok(StaticPropertyKey::Name("null".into())),
        RegisterValue::Undefined => Ok(StaticPropertyKey::Name("undefined".into())),
        _ => Err(imported_error("property key is not statically known")),
    }
}

fn static_define_property_attributes(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<StaticDefinePropertyAttributes, LlvmError> {
    let RegisterValue::Scalar(expression) = value else {
        return Err(imported_error(
            "define-property attributes are not a static integer",
        ));
    };
    let raw = u32::try_from(evaluate(expression, functions, &[], 0)?)
        .map_err(|_| imported_error("define-property attributes exceed u32"))?;
    if raw & !0x1ff != 0 || raw & ((1 << 7) | (1 << 8)) != 0 {
        return Err(imported_error(
            "data-property attributes contain unsupported bits",
        ));
    }
    let tri_state = |shift: u32| match (raw >> shift) & 0b11_u32 {
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        2 => Ok(None),
        _ => Err(imported_error(
            "define-property attributes contain an invalid tri-state",
        )),
    };
    Ok(StaticDefinePropertyAttributes {
        configurable: tri_state(0)?,
        enumerable: tri_state(2)?,
        writable: tri_state(4)?,
        has_value: raw & (1 << 6) != 0,
    })
}

fn static_define_accessor_attributes(
    value: &RegisterValue,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<StaticDefineAccessorAttributes, LlvmError> {
    let RegisterValue::Scalar(expression) = value else {
        return Err(imported_error(
            "define-accessor attributes are not a static integer",
        ));
    };
    let raw = u32::try_from(evaluate(expression, functions, &[], 0)?)
        .map_err(|_| imported_error("define-accessor attributes exceed u32"))?;
    if raw & !0x1ff != 0 || raw & (1 << 6) != 0 || (raw >> 4) & 0b11 != 0b10 {
        return Err(imported_error(
            "accessor-property attributes contain unsupported bits",
        ));
    }
    let tri_state = |shift: u32| match (raw >> shift) & 0b11_u32 {
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        2 => Ok(None),
        _ => Err(imported_error(
            "define-accessor attributes contain an invalid tri-state",
        )),
    };
    Ok(StaticDefineAccessorAttributes {
        configurable: tri_state(0)?,
        enumerable: tri_state(2)?,
        has_get: raw & (1 << 7) != 0,
        has_set: raw & (1 << 8) != 0,
    })
}

fn static_enumerable_keys(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
) -> Result<(u32, Vec<Box<str>>), LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => return Err(imported_error("enumeration base is not a static object")),
    };
    let entry = heap
        .get(&id)
        .ok_or_else(|| imported_error("enumeration references a missing static heap entry"))?;
    let keys = match entry {
        StaticHeapEntry::Object {
            properties,
            property_order,
            ..
        } => ordered_property_keys(properties, property_order)?,
        StaticHeapEntry::Array {
            elements,
            properties,
            property_order,
            ..
        } => {
            let mut keys = elements
                .iter()
                .filter(|(_, property)| property.enumerable)
                .map(|(index, _)| index.to_string().into_boxed_str())
                .collect::<Vec<_>>();
            keys.extend(ordered_property_keys(properties, property_order)?);
            keys
        }
        StaticHeapEntry::Function {
            properties,
            property_order,
            ..
        } => ordered_property_keys(properties, property_order)?,
    };
    Ok((id, keys))
}

fn ordered_property_keys(
    properties: &BTreeMap<Box<str>, StaticProperty>,
    property_order: &[Box<str>],
) -> Result<Vec<Box<str>>, LlvmError> {
    let mut index_keys = properties
        .iter()
        .filter(|(_, property)| property.enumerable)
        .filter_map(|(name, _)| canonical_array_index(name).map(|index| (index, name.clone())))
        .collect::<Vec<_>>();
    index_keys.sort_by_key(|(index, _)| *index);
    let mut keys = index_keys
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>();
    for name in property_order {
        if canonical_array_index(name).is_none()
            && properties
                .get(name)
                .is_some_and(|property| property.enumerable)
        {
            keys.push(name.clone());
        }
    }
    let expected = properties
        .iter()
        .filter(|(name, property)| canonical_array_index(name).is_none() && property.enumerable)
        .count();
    let enumerable_count = properties
        .values()
        .filter(|property| property.enumerable)
        .count();
    if keys.len() != enumerable_count || property_order.len() < expected {
        return Err(imported_error(
            "static property insertion order is incomplete",
        ));
    }
    Ok(keys)
}

fn canonical_array_index(name: &str) -> Option<u32> {
    name.parse::<u32>()
        .ok()
        .filter(|index| *index != u32::MAX && index.to_string() == name)
}

fn ensure_enumerator_base(base: &RegisterValue, expected_heap_id: u32) -> Result<(), LlvmError> {
    match base {
        RegisterValue::Object(heap_id) | RegisterValue::Array(heap_id)
            if *heap_id == expected_heap_id =>
        {
            Ok(())
        }
        RegisterValue::Function { heap_id, .. } if *heap_id == expected_heap_id => Ok(()),
        _ => Err(imported_error("enumerator base identity changed")),
    }
}

fn static_prototype_id(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    value: &RegisterValue,
) -> Result<Option<u32>, LlvmError> {
    let id = match value {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "prototype lookup base is not a static object",
            ));
        }
    };
    match heap
        .get(&id)
        .ok_or_else(|| imported_error("prototype lookup references a missing static heap entry"))?
    {
        StaticHeapEntry::Object { prototype, .. } | StaticHeapEntry::Array { prototype, .. } => {
            Ok(*prototype)
        }
        StaticHeapEntry::Function { .. } => Ok(None),
    }
}

fn static_prototype_value(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    value: &RegisterValue,
) -> Result<RegisterValue, LlvmError> {
    match value {
        RegisterValue::Function { heap_id, .. } => match heap.get(heap_id).ok_or_else(|| {
            imported_error("prototype lookup references a missing static function")
        })? {
            StaticHeapEntry::Function { prototype, .. } => {
                Ok(prototype.clone().unwrap_or(RegisterValue::Null))
            }
            _ => Err(imported_error(
                "function references a non-function static heap entry",
            )),
        },
        RegisterValue::Object(_) | RegisterValue::Array(_) => static_prototype_id(heap, value)?
            .map(|id| static_heap_value(heap, id))
            .transpose()
            .map(|value| value.unwrap_or(RegisterValue::Null)),
        RegisterValue::InternalObject { prototype, .. } => {
            Ok(prototype.as_deref().cloned().unwrap_or(RegisterValue::Null))
        }
        _ => Err(imported_error(
            "prototype lookup base is not a static object",
        )),
    }
}

fn static_set_prototype(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    prototype: &RegisterValue,
) -> Result<(), LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "prototype mutation base is not a static object",
            ));
        }
    };
    let entry = heap
        .get_mut(&id)
        .ok_or_else(|| imported_error("prototype mutation references a missing static entry"))?;
    match entry {
        StaticHeapEntry::Object {
            prototype: target, ..
        }
        | StaticHeapEntry::Array {
            prototype: target, ..
        } => {
            *target = match prototype {
                RegisterValue::Object(id) | RegisterValue::Array(id) => Some(*id),
                RegisterValue::Null => None,
                _ => {
                    return Err(imported_error(
                        "object prototype is neither an object nor null",
                    ));
                }
            };
        }
        StaticHeapEntry::Function {
            prototype: target, ..
        } => {
            if !matches!(
                prototype,
                RegisterValue::Function { .. }
                    | RegisterValue::Object(_)
                    | RegisterValue::Array(_)
                    | RegisterValue::Null
            ) {
                return Err(imported_error(
                    "function prototype is neither an object nor null",
                ));
            }
            *target = (!matches!(prototype, RegisterValue::Null)).then(|| prototype.clone());
        }
    }
    Ok(())
}

fn static_heap_value(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    id: u32,
) -> Result<RegisterValue, LlvmError> {
    match heap
        .get(&id)
        .ok_or_else(|| imported_error("static heap value references a missing entry"))?
    {
        StaticHeapEntry::Object { .. } => Ok(RegisterValue::Object(id)),
        StaticHeapEntry::Array { .. } => Ok(RegisterValue::Array(id)),
        StaticHeapEntry::Function { .. } => Err(imported_error(
            "function heap identity cannot be reconstructed without its executable",
        )),
    }
}

fn static_prototype_chain_contains(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    value: &RegisterValue,
    expected: u32,
) -> Result<bool, LlvmError> {
    let mut current = static_prototype_id(heap, value)?;
    for _ in 0..=64 {
        let Some(id) = current else {
            return Ok(false);
        };
        if id == expected {
            return Ok(true);
        }
        current = match heap.get(&id).ok_or_else(|| {
            imported_error("prototype chain references a missing static heap entry")
        })? {
            StaticHeapEntry::Object { prototype, .. }
            | StaticHeapEntry::Array { prototype, .. } => *prototype,
            StaticHeapEntry::Function { .. } => None,
        };
    }
    Err(imported_error("static prototype chain exceeds 64 objects"))
}

fn static_get_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<RegisterValue, LlvmError> {
    if matches!(base, RegisterValue::Array(_))
        && matches!(&key, StaticPropertyKey::Name(name) if name.as_ref() == "Symbol.iterator")
    {
        return Ok(RegisterValue::ArrayIteratorMethod);
    }
    if let RegisterValue::Arguments { values, .. } = base {
        let values = values.borrow();
        return match key {
            StaticPropertyKey::Index(index) => Ok(values
                .get(index as usize)
                .cloned()
                .unwrap_or(RegisterValue::Undefined)),
            StaticPropertyKey::Name(name) if name.as_ref() == "length" => {
                Ok(RegisterValue::Scalar(ScalarExpression::Integer(
                    i64::try_from(values.len())
                        .map_err(|_| imported_error("argument count exceeds i64"))?,
                )))
            }
            StaticPropertyKey::Name(name) => canonical_array_index(&name)
                .and_then(|index| values.get(index as usize).cloned())
                .map(Ok)
                .unwrap_or(Ok(RegisterValue::Undefined)),
        };
    }
    if let RegisterValue::ScopedArguments {
        environment,
        length,
    } = base
    {
        let index = match &key {
            StaticPropertyKey::Index(index) => Some(*index),
            StaticPropertyKey::Name(name) => canonical_array_index(name),
        };
        if let Some(index) = index {
            return Ok(environment
                .borrow()
                .scoped_argument_values
                .get(&(index as usize))
                .cloned()
                .unwrap_or(RegisterValue::Undefined));
        }
        return match key {
            StaticPropertyKey::Name(name) if name.as_ref() == "length" => {
                Ok(RegisterValue::Scalar(ScalarExpression::Integer(
                    i64::try_from(length.get())
                        .map_err(|_| imported_error("argument count exceeds i64"))?,
                )))
            }
            _ => Ok(RegisterValue::Undefined),
        };
    }
    if let RegisterValue::Error { kind, message } = base {
        return match key {
            StaticPropertyKey::Name(name) if name.as_ref() == "name" => {
                Ok(RegisterValue::String(static_error_name(*kind).into()))
            }
            StaticPropertyKey::Name(name) if name.as_ref() == "message" => {
                Ok(RegisterValue::String(message.clone()))
            }
            _ => Ok(RegisterValue::Undefined),
        };
    }
    let (id, is_array) = match base {
        RegisterValue::Object(id) => (*id, false),
        RegisterValue::Array(id) => (*id, true),
        RegisterValue::Function { heap_id, .. } => (*heap_id, false),
        _ => return Err(imported_error("property read base is not a static object")),
    };
    if let Some(value) = static_lookup_property(heap, id, &key, 0)? {
        return Ok(value);
    }
    match key {
        StaticPropertyKey::Name(name) => inherited_static_property(&name, is_array),
        StaticPropertyKey::Index(_) => Ok(RegisterValue::Undefined),
    }
}

fn static_get_own_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<RegisterValue, LlvmError> {
    if matches!(
        base,
        RegisterValue::Arguments { .. }
            | RegisterValue::ScopedArguments { .. }
            | RegisterValue::Error { .. }
    ) {
        return static_get_property(heap, base, key);
    }
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "direct property read base is not a static object",
            ));
        }
    };
    let entry = heap.get(&id).ok_or_else(|| {
        imported_error("direct property read references a missing static heap entry")
    })?;
    let value = match (entry, key) {
        (StaticHeapEntry::Object { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.get(&name).map(StaticProperty::read)
        }
        (StaticHeapEntry::Object { properties, .. }, StaticPropertyKey::Index(index)) => properties
            .get(index.to_string().as_str())
            .map(StaticProperty::read),
        (StaticHeapEntry::Array { length, .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            Some(RegisterValue::Scalar(ScalarExpression::Integer(i64::from(
                *length,
            ))))
        }
        (StaticHeapEntry::Array { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.get(&name).map(StaticProperty::read)
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            elements.get(&index).map(StaticProperty::read)
        }
        (StaticHeapEntry::Function { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.get(&name).map(StaticProperty::read)
        }
        (StaticHeapEntry::Function { properties, .. }, StaticPropertyKey::Index(index)) => {
            properties
                .get(index.to_string().as_str())
                .map(StaticProperty::read)
        }
    };
    Ok(value.unwrap_or(RegisterValue::Undefined))
}

fn private_name_identity(value: &RegisterValue) -> Result<u32, LlvmError> {
    match value {
        RegisterValue::PrivateName { identity, .. } => Ok(*identity),
        _ => Err(imported_error("private property key is not a private name")),
    }
}

fn static_private_properties<'a>(
    heap: &'a BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
) -> Result<&'a BTreeMap<u32, RegisterValue>, LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "private property base is not a static object",
            ));
        }
    };
    match heap
        .get(&id)
        .ok_or_else(|| imported_error("private property references a missing static heap entry"))?
    {
        StaticHeapEntry::Object {
            private_properties, ..
        }
        | StaticHeapEntry::Array {
            private_properties, ..
        }
        | StaticHeapEntry::Function {
            private_properties, ..
        } => Ok(private_properties),
    }
}

fn static_private_properties_mut<'a>(
    heap: &'a mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
) -> Result<&'a mut BTreeMap<u32, RegisterValue>, LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "private property base is not a static object",
            ));
        }
    };
    match heap
        .get_mut(&id)
        .ok_or_else(|| imported_error("private property references a missing static heap entry"))?
    {
        StaticHeapEntry::Object {
            private_properties, ..
        }
        | StaticHeapEntry::Array {
            private_properties, ..
        }
        | StaticHeapEntry::Function {
            private_properties, ..
        } => Ok(private_properties),
    }
}

fn static_put_private_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    property: &RegisterValue,
    value: RegisterValue,
    put_kind: u32,
) -> Result<(), LlvmError> {
    let identity = private_name_identity(property)?;
    let properties = static_private_properties_mut(heap, base)?;
    match put_kind {
        1 => {
            let existing = properties
                .get_mut(&identity)
                .ok_or_else(|| imported_error("private field set failed its brand check"))?;
            *existing = value;
        }
        2 => {
            if properties.insert(identity, value).is_some() {
                return Err(imported_error(
                    "private field definition encountered an existing field",
                ));
            }
        }
        _ => {
            return Err(imported_error(format!(
                "private-field put kind {put_kind} is not admitted"
            )));
        }
    }
    Ok(())
}

fn static_get_private_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    property: &RegisterValue,
) -> Result<RegisterValue, LlvmError> {
    let identity = private_name_identity(property)?;
    static_private_properties(heap, base)?
        .get(&identity)
        .cloned()
        .ok_or_else(|| imported_error("private field read failed its brand check"))
}

fn static_has_private_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    property: &RegisterValue,
) -> Result<bool, LlvmError> {
    let identity = private_name_identity(property)?;
    Ok(static_private_properties(heap, base)?.contains_key(&identity))
}

fn static_private_brands<'a>(
    heap: &'a BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
) -> Result<&'a BTreeSet<u32>, LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => return Err(imported_error("private brand base is not a static object")),
    };
    match heap
        .get(&id)
        .ok_or_else(|| imported_error("private brand references a missing static heap entry"))?
    {
        StaticHeapEntry::Object { private_brands, .. }
        | StaticHeapEntry::Array { private_brands, .. }
        | StaticHeapEntry::Function { private_brands, .. } => Ok(private_brands),
    }
}

fn static_private_brands_mut<'a>(
    heap: &'a mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
) -> Result<&'a mut BTreeSet<u32>, LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => return Err(imported_error("private brand base is not a static object")),
    };
    match heap
        .get_mut(&id)
        .ok_or_else(|| imported_error("private brand references a missing static heap entry"))?
    {
        StaticHeapEntry::Object { private_brands, .. }
        | StaticHeapEntry::Array { private_brands, .. }
        | StaticHeapEntry::Function { private_brands, .. } => Ok(private_brands),
    }
}

fn static_set_private_brand(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    brand: &RegisterValue,
) -> Result<(), LlvmError> {
    let identity = private_name_identity(brand)?;
    if !static_private_brands_mut(heap, base)?.insert(identity) {
        return Err(imported_error(
            "private brand initialization encountered an existing brand",
        ));
    }
    Ok(())
}

fn static_has_private_brand(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    brand: &RegisterValue,
) -> Result<bool, LlvmError> {
    let identity = private_name_identity(brand)?;
    Ok(static_private_brands(heap, base)?.contains(&identity))
}

fn static_lookup_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    id: u32,
    key: &StaticPropertyKey,
    depth: usize,
) -> Result<Option<RegisterValue>, LlvmError> {
    Ok(static_lookup_property_descriptor(heap, id, key, depth)?.map(|property| property.read()))
}

fn static_lookup_property_descriptor(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    id: u32,
    key: &StaticPropertyKey,
    depth: usize,
) -> Result<Option<StaticProperty>, LlvmError> {
    if depth > 64 {
        return Err(imported_error("static prototype chain exceeds 64 objects"));
    }
    let entry = heap
        .get(&id)
        .ok_or_else(|| imported_error("property lookup references a missing static heap entry"))?;
    let (value, prototype) = match (entry, key) {
        (
            StaticHeapEntry::Object {
                properties,
                prototype,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => (properties.get(name).cloned(), *prototype),
        (
            StaticHeapEntry::Object {
                properties,
                prototype,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => (
            properties.get(index.to_string().as_str()).cloned(),
            *prototype,
        ),
        (StaticHeapEntry::Array { length, .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Ok(Some(StaticProperty {
                value: RegisterValue::Scalar(ScalarExpression::Integer(i64::from(*length))),
                getter: None,
                setter: None,
                is_accessor: false,
                writable: true,
                enumerable: false,
                configurable: false,
            }));
        }
        (
            StaticHeapEntry::Array {
                properties,
                prototype,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => (properties.get(name).cloned(), *prototype),
        (
            StaticHeapEntry::Array {
                elements,
                prototype,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => (elements.get(index).cloned(), *prototype),
        (StaticHeapEntry::Function { properties, .. }, StaticPropertyKey::Name(name)) => {
            (properties.get(name).cloned(), None)
        }
        (StaticHeapEntry::Function { properties, .. }, StaticPropertyKey::Index(index)) => {
            (properties.get(index.to_string().as_str()).cloned(), None)
        }
    };
    if value.is_some() {
        return Ok(value);
    }
    prototype.map_or(Ok(None), |prototype| {
        static_lookup_property_descriptor(heap, prototype, key, depth + 1)
    })
}

enum StaticPropertyAssignment {
    Stored,
    CallSetter(RegisterValue),
}

enum StaticAccessorUpdate {
    Preserve,
    Set(Option<RegisterValue>),
}

fn static_set_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
    value: RegisterValue,
) -> Result<StaticPropertyAssignment, LlvmError> {
    if let RegisterValue::Arguments { values, .. } = base {
        let index = match key {
            StaticPropertyKey::Index(index) => Some(index),
            StaticPropertyKey::Name(name) => canonical_array_index(&name),
        };
        let Some(index) = index else {
            return Err(imported_error(
                "arguments object write requires an array-index property",
            ));
        };
        let index = usize::try_from(index)
            .map_err(|_| imported_error("arguments object index exceeds usize"))?;
        let mut values = values.borrow_mut();
        if index >= values.len() {
            values.resize(index + 1, RegisterValue::Undefined);
        }
        values[index] = value;
        return Ok(StaticPropertyAssignment::Stored);
    }
    if let RegisterValue::ScopedArguments {
        environment,
        length,
    } = base
    {
        let index = match key {
            StaticPropertyKey::Index(index) => Some(index),
            StaticPropertyKey::Name(name) => canonical_array_index(&name),
        };
        let Some(index) = index else {
            return Err(imported_error(
                "scoped arguments object write requires an array-index property",
            ));
        };
        let index = usize::try_from(index)
            .map_err(|_| imported_error("arguments object index exceeds usize"))?;
        environment
            .borrow_mut()
            .scoped_argument_values
            .insert(index, value.clone());
        let name = {
            let environment = environment.borrow();
            environment
                .scoped_argument_names
                .iter()
                .find_map(|(name, mapped)| (*mapped == index).then(|| name.clone()))
        };
        if let Some(name) = name {
            environment.borrow_mut().bindings.insert(name, value);
        }
        length.set(length.get().max(index.saturating_add(1)));
        return Ok(StaticPropertyAssignment::Stored);
    }
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => return Err(imported_error("property write base is not a static object")),
    };
    if let Some(property) = static_lookup_property_descriptor(heap, id, &key, 0)?
        && property.is_accessor
    {
        return property
            .setter
            .map(StaticPropertyAssignment::CallSetter)
            .ok_or_else(|| imported_error("static accessor property has no setter"));
    }
    static_put_property(heap, base, key, value)?;
    Ok(StaticPropertyAssignment::Stored)
}

fn static_set_property_with_receiver(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    receiver: &RegisterValue,
    key: StaticPropertyKey,
    value: RegisterValue,
) -> Result<StaticPropertyAssignment, LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => return Err(imported_error("super property base is not a static object")),
    };
    if let Some(property) = static_lookup_property_descriptor(heap, id, &key, 0)?
        && property.is_accessor
    {
        return property
            .setter
            .map(StaticPropertyAssignment::CallSetter)
            .ok_or_else(|| imported_error("static super accessor has no setter"));
    }
    static_set_property(heap, receiver, key, value)
}

fn static_error_name(kind: u32) -> &'static str {
    match kind {
        0 => "Error",
        1 => "EvalError",
        2 => "RangeError",
        3 => "ReferenceError",
        4 => "SyntaxError",
        5 => "TypeError",
        6 => "URIError",
        7 => "AggregateError",
        8 => "SuppressedError",
        9 => "OutOfMemoryError",
        _ => "Error",
    }
}

fn static_put_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
    value: RegisterValue,
) -> Result<(), LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => return Err(imported_error("property write base is not a static object")),
    };
    let entry = heap
        .get_mut(&id)
        .ok_or_else(|| imported_error("property write references a missing static heap entry"))?;
    match (entry, key) {
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            if name.as_ref() == "__proto__" {
                return Err(imported_error("static __proto__ mutation is not admitted"));
            }
            if !properties.contains_key(&name) {
                property_order.push(name.clone());
            }
            static_assign_property(properties, name, value)?;
        }
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            let name = index.to_string().into_boxed_str();
            if !properties.contains_key(&name) {
                property_order.push(name.clone());
            }
            static_assign_property(properties, name, value)?;
        }
        (
            StaticHeapEntry::Array {
                elements, length, ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            if let Some(property) = elements.get_mut(&index) {
                if !property.writable {
                    return Err(imported_error("static array element is not writable"));
                }
                property.value = value;
            } else {
                elements.insert(index, StaticProperty::assigned(value));
            }
            *length = (*length).max(index.saturating_add(1));
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Err(imported_error(
                "static array length mutation is not admitted",
            ));
        }
        (
            StaticHeapEntry::Array {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            if !properties.contains_key(&name) {
                property_order.push(name.clone());
            }
            static_assign_property(properties, name, value)?;
        }
        (
            StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            if !properties.contains_key(&name) {
                property_order.push(name.clone());
            }
            static_assign_property(properties, name, value)?;
        }
        (
            StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            let name = index.to_string().into_boxed_str();
            if !properties.contains_key(&name) {
                property_order.push(name.clone());
            }
            static_assign_property(properties, name, value)?;
        }
    }
    Ok(())
}

fn static_assign_property(
    properties: &mut BTreeMap<Box<str>, StaticProperty>,
    name: Box<str>,
    value: RegisterValue,
) -> Result<(), LlvmError> {
    if let Some(property) = properties.get_mut(&name) {
        if !property.writable {
            return Err(imported_error(format!(
                "static property {name} is not writable"
            )));
        }
        property.value = value;
    } else {
        properties.insert(name, StaticProperty::assigned(value));
    }
    Ok(())
}

fn static_accessor_value(value: RegisterValue) -> Result<Option<RegisterValue>, LlvmError> {
    match value {
        RegisterValue::Function { .. } => Ok(Some(value)),
        RegisterValue::Undefined => Ok(None),
        _ => Err(imported_error(
            "accessor is neither a function nor undefined",
        )),
    }
}

fn static_put_accessor(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
    attributes: u64,
    getter: StaticAccessorUpdate,
    setter: StaticAccessorUpdate,
) -> Result<(), LlvmError> {
    const READ_ONLY: u64 = 1;
    const DONT_ENUM: u64 = 1 << 1;
    const DONT_DELETE: u64 = 1 << 2;
    const ACCESSOR: u64 = 1 << 4;
    if attributes & ACCESSOR == 0
        || attributes & !(READ_ONLY | DONT_ENUM | DONT_DELETE | ACCESSOR) != 0
    {
        return Err(imported_error(format!(
            "unsupported accessor property attributes 0x{attributes:x}"
        )));
    }
    let enumerable = attributes & DONT_ENUM == 0;
    let configurable = attributes & DONT_DELETE == 0;
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "accessor definition base is not a static object",
            ));
        }
    };
    let entry = heap.get_mut(&id).ok_or_else(|| {
        imported_error("accessor definition references a missing static heap entry")
    })?;
    let update = |existing: Option<&mut StaticProperty>| -> Result<StaticProperty, LlvmError> {
        if let Some(existing) = existing {
            if !existing.configurable
                && (!existing.is_accessor || existing.enumerable != enumerable)
            {
                return Err(imported_error(
                    "invalid redefinition of a non-configurable static accessor",
                ));
            }
            let existing_getter = existing
                .is_accessor
                .then(|| existing.getter.clone())
                .flatten();
            let existing_setter = existing
                .is_accessor
                .then(|| existing.setter.clone())
                .flatten();
            let getter = match &getter {
                StaticAccessorUpdate::Preserve => existing_getter,
                StaticAccessorUpdate::Set(value) => value.clone(),
            };
            let setter = match &setter {
                StaticAccessorUpdate::Preserve => existing_setter,
                StaticAccessorUpdate::Set(value) => value.clone(),
            };
            return Ok(StaticProperty::accessor(
                getter,
                setter,
                enumerable,
                existing.configurable && configurable,
            ));
        }
        let getter = match &getter {
            StaticAccessorUpdate::Preserve => None,
            StaticAccessorUpdate::Set(value) => value.clone(),
        };
        let setter = match &setter {
            StaticAccessorUpdate::Preserve => None,
            StaticAccessorUpdate::Set(value) => value.clone(),
        };
        Ok(StaticProperty::accessor(
            getter,
            setter,
            enumerable,
            configurable,
        ))
    };
    match (entry, key) {
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            }
            | StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            let is_new = !properties.contains_key(&name);
            let property = update(properties.get_mut(&name))?;
            if is_new {
                property_order.push(name.clone());
            }
            properties.insert(name, property);
        }
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            }
            | StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            let name = index.to_string().into_boxed_str();
            let is_new = !properties.contains_key(&name);
            let property = update(properties.get_mut(&name))?;
            if is_new {
                property_order.push(name.clone());
            }
            properties.insert(name, property);
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            let property = update(elements.get_mut(&index))?;
            elements.insert(index, property);
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Err(imported_error("array length cannot become an accessor"));
        }
        (
            StaticHeapEntry::Array {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            let is_new = !properties.contains_key(&name);
            let property = update(properties.get_mut(&name))?;
            if is_new {
                property_order.push(name.clone());
            }
            properties.insert(name, property);
        }
    }
    Ok(())
}

fn static_define_accessor_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
    getter: Option<RegisterValue>,
    setter: Option<RegisterValue>,
    attributes: StaticDefineAccessorAttributes,
) -> Result<(), LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "accessor-property definition base is not a static object",
            ));
        }
    };
    let update = |existing: Option<&StaticProperty>| -> Result<StaticProperty, LlvmError> {
        if let Some(existing) = existing
            && !existing.configurable
        {
            if !existing.is_accessor
                || attributes.configurable == Some(true)
                || attributes
                    .enumerable
                    .is_some_and(|enumerable| enumerable != existing.enumerable)
                || (attributes.has_get && getter != existing.getter)
                || (attributes.has_set && setter != existing.setter)
            {
                return Err(imported_error(
                    "invalid redefinition of a non-configurable static accessor",
                ));
            }
        }
        let existing_accessor = existing.filter(|property| property.is_accessor);
        Ok(StaticProperty::accessor(
            if attributes.has_get {
                getter.clone()
            } else {
                existing_accessor.and_then(|property| property.getter.clone())
            },
            if attributes.has_set {
                setter.clone()
            } else {
                existing_accessor.and_then(|property| property.setter.clone())
            },
            attributes
                .enumerable
                .or_else(|| existing.map(|property| property.enumerable))
                .unwrap_or(false),
            attributes
                .configurable
                .or_else(|| existing.map(|property| property.configurable))
                .unwrap_or(false),
        ))
    };
    let entry = heap.get_mut(&id).ok_or_else(|| {
        imported_error("accessor-property definition references a missing static heap entry")
    })?;
    match (entry, key) {
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            }
            | StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            let is_new = !properties.contains_key(&name);
            let property = update(properties.get(&name))?;
            if is_new {
                property_order.push(name.clone());
            }
            properties.insert(name, property);
        }
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            }
            | StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            let name = index.to_string().into_boxed_str();
            let is_new = !properties.contains_key(&name);
            let property = update(properties.get(&name))?;
            if is_new {
                property_order.push(name.clone());
            }
            properties.insert(name, property);
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            let property = update(elements.get(&index))?;
            elements.insert(index, property);
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Err(imported_error("array length cannot become an accessor"));
        }
        (
            StaticHeapEntry::Array {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            let is_new = !properties.contains_key(&name);
            let property = update(properties.get(&name))?;
            if is_new {
                property_order.push(name.clone());
            }
            properties.insert(name, property);
        }
    }
    Ok(())
}

fn static_define_data_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
    value: RegisterValue,
    attributes: StaticDefinePropertyAttributes,
) -> Result<(), LlvmError> {
    if !attributes.has_value {
        return Err(imported_error(
            "data-property definition omits its value field",
        ));
    }
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "data-property definition base is not a static object",
            ));
        }
    };
    let entry = heap.get_mut(&id).ok_or_else(|| {
        imported_error("data-property definition references a missing static heap entry")
    })?;
    let define = |properties: &mut BTreeMap<Box<str>, StaticProperty>,
                  property_order: &mut Vec<Box<str>>,
                  name: Box<str>,
                  value: RegisterValue|
     -> Result<(), LlvmError> {
        if let Some(existing) = properties.get_mut(&name) {
            if !existing.configurable {
                if attributes.configurable == Some(true)
                    || attributes
                        .enumerable
                        .is_some_and(|enumerable| enumerable != existing.enumerable)
                    || (!existing.writable
                        && (attributes.writable == Some(true) || existing.value != value))
                {
                    return Err(imported_error(format!(
                        "invalid redefinition of non-configurable static property {name}"
                    )));
                }
            }
            existing.value = value;
            existing.getter = None;
            existing.setter = None;
            existing.is_accessor = false;
            if let Some(configurable) = attributes.configurable {
                existing.configurable = configurable;
            }
            if let Some(enumerable) = attributes.enumerable {
                existing.enumerable = enumerable;
            }
            if let Some(writable) = attributes.writable {
                existing.writable = writable;
            }
        } else {
            property_order.push(name.clone());
            properties.insert(
                name,
                StaticProperty {
                    value,
                    getter: None,
                    setter: None,
                    is_accessor: false,
                    configurable: attributes.configurable.unwrap_or(false),
                    enumerable: attributes.enumerable.unwrap_or(false),
                    writable: attributes.writable.unwrap_or(false),
                },
            );
        }
        Ok(())
    };
    match (entry, key) {
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            }
            | StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => define(properties, property_order, name, value),
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            }
            | StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => define(
            properties,
            property_order,
            index.to_string().into_boxed_str(),
            value,
        ),
        (
            StaticHeapEntry::Array {
                elements, length, ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            if let Some(existing) = elements.get_mut(&index) {
                if !existing.configurable
                    && (attributes.configurable == Some(true)
                        || attributes
                            .enumerable
                            .is_some_and(|enumerable| enumerable != existing.enumerable)
                        || (!existing.writable
                            && (attributes.writable == Some(true) || existing.value != value)))
                {
                    return Err(imported_error(
                        "invalid redefinition of a non-configurable static array element",
                    ));
                }
                existing.value = value;
                existing.getter = None;
                existing.setter = None;
                existing.is_accessor = false;
                if let Some(configurable) = attributes.configurable {
                    existing.configurable = configurable;
                }
                if let Some(enumerable) = attributes.enumerable {
                    existing.enumerable = enumerable;
                }
                if let Some(writable) = attributes.writable {
                    existing.writable = writable;
                }
            } else {
                elements.insert(
                    index,
                    StaticProperty {
                        value,
                        getter: None,
                        setter: None,
                        is_accessor: false,
                        configurable: attributes.configurable.unwrap_or(false),
                        enumerable: attributes.enumerable.unwrap_or(false),
                        writable: attributes.writable.unwrap_or(false),
                    },
                );
                *length = (*length).max(index.saturating_add(1));
            }
            Ok(())
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            Err(imported_error(
                "static array length definition is not admitted",
            ))
        }
        (
            StaticHeapEntry::Array {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => define(properties, property_order, name, value),
    }
}

fn static_remove_property(
    properties: &mut BTreeMap<Box<str>, StaticProperty>,
    name: &str,
) -> Result<bool, LlvmError> {
    if properties
        .get(name)
        .is_some_and(|property| !property.configurable)
    {
        return Err(imported_error(format!(
            "static property {name} is not configurable"
        )));
    }
    Ok(properties.remove(name).is_some())
}

fn static_has_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<bool, LlvmError> {
    let (id, is_array) = match base {
        RegisterValue::Object(id) => (*id, false),
        RegisterValue::Array(id) => (*id, true),
        RegisterValue::Function { heap_id, .. } => (*heap_id, false),
        _ => {
            return Err(imported_error(
                "property membership base is not a static object",
            ));
        }
    };
    if static_lookup_property(heap, id, &key, 0)?.is_some() {
        return Ok(true);
    }
    match key {
        StaticPropertyKey::Name(name) => missing_static_property(&name, is_array).map(|_| false),
        StaticPropertyKey::Index(_) => Ok(false),
    }
}

fn static_has_own_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<bool, LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "own-property membership base is not a static object",
            ));
        }
    };
    let entry = heap.get(&id).ok_or_else(|| {
        imported_error("own-property membership references a missing static heap entry")
    })?;
    Ok(match (entry, key) {
        (StaticHeapEntry::Object { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.contains_key(&name)
        }
        (StaticHeapEntry::Object { properties, .. }, StaticPropertyKey::Index(index)) => {
            properties.contains_key(index.to_string().as_str())
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            elements.contains_key(&index)
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            true
        }
        (StaticHeapEntry::Array { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.contains_key(&name)
        }
        (StaticHeapEntry::Function { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.contains_key(&name)
        }
        (StaticHeapEntry::Function { properties, .. }, StaticPropertyKey::Index(index)) => {
            properties.contains_key(index.to_string().as_str())
        }
    })
}

fn static_delete_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<(), LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
        RegisterValue::Function { heap_id, .. } => *heap_id,
        _ => {
            return Err(imported_error(
                "property delete base is not a static object",
            ));
        }
    };
    let entry = heap
        .get_mut(&id)
        .ok_or_else(|| imported_error("property delete references a missing static heap entry"))?;
    match (entry, key) {
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            if static_remove_property(properties, &name)? {
                property_order.retain(|ordered| ordered != &name);
            }
        }
        (
            StaticHeapEntry::Object {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            let name = index.to_string();
            if static_remove_property(properties, name.as_str())? {
                property_order.retain(|ordered| ordered.as_ref() != name);
            }
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            if elements
                .get(&index)
                .is_some_and(|property| !property.configurable)
            {
                return Err(imported_error("static array element is not configurable"));
            }
            elements.remove(&index);
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Err(imported_error("array length is not configurable"));
        }
        (
            StaticHeapEntry::Array {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            if static_remove_property(properties, &name)? {
                property_order.retain(|ordered| ordered != &name);
            }
        }
        (
            StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Name(name),
        ) => {
            if static_remove_property(properties, &name)? {
                property_order.retain(|ordered| ordered != &name);
            }
        }
        (
            StaticHeapEntry::Function {
                properties,
                property_order,
                ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            let name = index.to_string();
            if static_remove_property(properties, name.as_str())? {
                property_order.retain(|ordered| ordered.as_ref() != name);
            }
        }
    }
    Ok(())
}

fn missing_static_property(name: &str, is_array: bool) -> Result<RegisterValue, LlvmError> {
    const OBJECT_PROTOTYPE_PROPERTIES: &[&str] = &[
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
        "__proto__",
        "constructor",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toLocaleString",
        "toString",
        "valueOf",
    ];
    if is_array || OBJECT_PROTOTYPE_PROPERTIES.contains(&name) {
        return Err(imported_error(format!(
            "static property read may observe a prototype member named {name}"
        )));
    }
    Ok(RegisterValue::Undefined)
}

fn inherited_static_property(name: &str, is_array: bool) -> Result<RegisterValue, LlvmError> {
    if name == "hasOwnProperty" {
        return Ok(RegisterValue::Builtin(Builtin::HasOwnPropertyFunction));
    }
    missing_static_property(name, is_array)
}

fn signed_operand(instruction: &VisitorInstruction, name: &str) -> Result<i64, LlvmError> {
    instruction
        .operands
        .iter()
        .find(|operand| operand.manifest_id.rsplit('.').next() == Some(name))
        .and_then(|operand| match operand.value {
            OperandValue::Signed(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| {
            imported_error(format!(
                "opcode {} has no signed {name}",
                instruction.opcode_id
            ))
        })
}

fn unsigned_operand(instruction: &VisitorInstruction, name: &str) -> Result<u64, LlvmError> {
    instruction
        .operands
        .iter()
        .find(|operand| operand.manifest_id.rsplit('.').next() == Some(name))
        .and_then(|operand| match operand.value {
            OperandValue::Unsigned(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| {
            imported_error(format!(
                "opcode {} has no unsigned {name}",
                instruction.opcode_id
            ))
        })
}

fn boolean_operand(instruction: &VisitorInstruction, name: &str) -> Result<bool, LlvmError> {
    instruction
        .operands
        .iter()
        .find(|operand| operand.manifest_id.rsplit('.').next() == Some(name))
        .and_then(|operand| match operand.value {
            OperandValue::Boolean(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| {
            imported_error(format!(
                "opcode {} has no boolean {name}",
                instruction.opcode_id
            ))
        })
}

fn unsupported(
    function: &VisitorFunction,
    instruction: &VisitorInstruction,
    opcode: &str,
) -> LlvmError {
    imported_error(format!(
        "f{} cannot yet lower {opcode} at byte {}",
        function.id.0, instruction.byte_offset
    ))
}

fn evaluate(
    expression: &ScalarExpression,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    arguments: &[i64],
    depth: usize,
) -> Result<i64, LlvmError> {
    if depth > 64 {
        return Err(imported_error("closed scalar call graph exceeds 64 frames"));
    }
    let result = match expression {
        ScalarExpression::Integer(value) => *value,
        ScalarExpression::Parameter(index) => *arguments
            .get(*index)
            .ok_or_else(|| imported_error(format!("missing scalar argument {index}")))?,
        ScalarExpression::Add(left, right) => evaluate(left, functions, arguments, depth)?
            .checked_add(evaluate(right, functions, arguments, depth)?)
            .ok_or_else(|| imported_error("integer addition exceeds i64"))?,
        ScalarExpression::Subtract(left, right) => evaluate(left, functions, arguments, depth)?
            .checked_sub(evaluate(right, functions, arguments, depth)?)
            .ok_or_else(|| imported_error("integer subtraction exceeds i64"))?,
        ScalarExpression::Multiply(left, right) => {
            let left = evaluate(left, functions, arguments, depth)?;
            let right = evaluate(right, functions, arguments, depth)?;
            if (left == 0 && right < 0) || (right == 0 && left < 0) {
                return Err(imported_error(
                    "scalar multiplication produces negative zero",
                ));
            }
            left.checked_mul(right)
                .ok_or_else(|| imported_error("integer multiplication exceeds i64"))?
        }
        ScalarExpression::Divide(left, right) => {
            let left = evaluate(left, functions, arguments, depth)?;
            let right = evaluate(right, functions, arguments, depth)?;
            if right == 0 || (left == 0 && right < 0) || left % right != 0 {
                return Err(imported_error(
                    "scalar division leaves the admitted integer domain",
                ));
            }
            left.checked_div(right)
                .ok_or_else(|| imported_error("integer division exceeds i64"))?
        }
        ScalarExpression::Remainder(left, right) => {
            let left = evaluate(left, functions, arguments, depth)?;
            let right = evaluate(right, functions, arguments, depth)?;
            let remainder = left
                .checked_rem(right)
                .ok_or_else(|| imported_error("invalid integer remainder"))?;
            if remainder == 0 && left < 0 {
                return Err(imported_error("scalar remainder produces negative zero"));
            }
            remainder
        }
        ScalarExpression::Power(left, right) => {
            let left = evaluate(left, functions, arguments, depth)?;
            let right = evaluate(right, functions, arguments, depth)?;
            let exponent = u32::try_from(right).map_err(|_| {
                imported_error("integer power exponent leaves the admitted non-negative domain")
            })?;
            left.checked_pow(exponent)
                .ok_or_else(|| imported_error("integer power exceeds i64"))?
        }
        ScalarExpression::BitAnd(left, right) => i64::from(
            to_int32(evaluate(left, functions, arguments, depth)?)
                & to_int32(evaluate(right, functions, arguments, depth)?),
        ),
        ScalarExpression::BitOr(left, right) => i64::from(
            to_int32(evaluate(left, functions, arguments, depth)?)
                | to_int32(evaluate(right, functions, arguments, depth)?),
        ),
        ScalarExpression::BitXor(left, right) => i64::from(
            to_int32(evaluate(left, functions, arguments, depth)?)
                ^ to_int32(evaluate(right, functions, arguments, depth)?),
        ),
        ScalarExpression::LeftShift(left, right) => {
            let left = to_uint32(evaluate(left, functions, arguments, depth)?);
            let shift = to_uint32(evaluate(right, functions, arguments, depth)?) & 31;
            i64::from(left.wrapping_shl(shift) as i32)
        }
        ScalarExpression::RightShift(left, right) => {
            let left = to_int32(evaluate(left, functions, arguments, depth)?);
            let shift = to_uint32(evaluate(right, functions, arguments, depth)?) & 31;
            i64::from(left >> shift)
        }
        ScalarExpression::UnsignedRightShift(left, right) => {
            let left = to_uint32(evaluate(left, functions, arguments, depth)?);
            let shift = to_uint32(evaluate(right, functions, arguments, depth)?) & 31;
            i64::from(left >> shift)
        }
        ScalarExpression::Negate(value) => {
            let value = evaluate(value, functions, arguments, depth)?;
            if value == 0 {
                return Err(imported_error("scalar negation produces negative zero"));
            }
            value
                .checked_neg()
                .ok_or_else(|| imported_error("integer negation exceeds i64"))?
        }
        ScalarExpression::BitNot(value) => {
            i64::from(!to_int32(evaluate(value, functions, arguments, depth)?))
        }
        ScalarExpression::Unsigned(value) => {
            i64::from(to_uint32(evaluate(value, functions, arguments, depth)?))
        }
        ScalarExpression::Equal(left, right) => i64::from(
            evaluate(left, functions, arguments, depth)?
                == evaluate(right, functions, arguments, depth)?,
        ),
        ScalarExpression::NotEqual(left, right) => i64::from(
            evaluate(left, functions, arguments, depth)?
                != evaluate(right, functions, arguments, depth)?,
        ),
        ScalarExpression::Less(left, right) => i64::from(
            evaluate(left, functions, arguments, depth)?
                < evaluate(right, functions, arguments, depth)?,
        ),
        ScalarExpression::LessEqual(left, right) => i64::from(
            evaluate(left, functions, arguments, depth)?
                <= evaluate(right, functions, arguments, depth)?,
        ),
        ScalarExpression::Greater(left, right) => i64::from(
            evaluate(left, functions, arguments, depth)?
                > evaluate(right, functions, arguments, depth)?,
        ),
        ScalarExpression::GreaterEqual(left, right) => i64::from(
            evaluate(left, functions, arguments, depth)?
                >= evaluate(right, functions, arguments, depth)?,
        ),
        ScalarExpression::Below(left, right) => i64::from(
            to_uint32(evaluate(left, functions, arguments, depth)?)
                < to_uint32(evaluate(right, functions, arguments, depth)?),
        ),
        ScalarExpression::BelowEqual(left, right) => i64::from(
            to_uint32(evaluate(left, functions, arguments, depth)?)
                <= to_uint32(evaluate(right, functions, arguments, depth)?),
        ),
        ScalarExpression::LogicalNot(value) => {
            i64::from(evaluate(value, functions, arguments, depth)? == 0)
        }
        ScalarExpression::Call {
            function,
            arguments: call_arguments,
        } => {
            let function = functions.get(function).ok_or_else(|| {
                imported_error(format!("call references missing f{}", function.0))
            })?;
            if function.parameter_count != call_arguments.len() {
                return Err(imported_error(format!(
                    "f{} expects {} argument(s), received {}",
                    function.id.0,
                    function.parameter_count,
                    call_arguments.len()
                )));
            }
            let values = call_arguments
                .iter()
                .map(|argument| evaluate(argument, functions, arguments, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            evaluate(&function.result, functions, &values, depth + 1)?
        }
    };
    if result.unsigned_abs() > MAX_SAFE_INTEGER as u64 {
        return Err(imported_error(
            "integer result exceeds JavaScript's safe range",
        ));
    }
    Ok(result)
}

fn to_uint32(value: i64) -> u32 {
    value.rem_euclid(1_i64 << 32) as u32
}

fn to_int32(value: i64) -> i32 {
    to_uint32(value) as i32
}

fn emit_application(
    target: TargetLayout,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    writes: &[NativeWrite],
) -> Result<String, LlvmError> {
    let mut output = String::new();
    output.push_str("; Hare imported scalar application\n");
    output.push_str("source_filename = \"hare-imported-application\"\n");
    let _ = writeln!(output, "target datalayout = \"{}\"", target.data_layout);
    let _ = writeln!(output, "target triple = \"{}\"\n", target.triple);
    for (index, write) in writes.iter().enumerate() {
        let NativeWrite::Text(bytes) = write else {
            continue;
        };
        let _ = write!(
            output,
            "@.hare.write.{index} = private unnamed_addr constant [{} x i8] c\"",
            bytes.len()
        );
        for byte in bytes {
            match byte {
                b' '..=b'~' if !matches!(byte, b'"' | b'\\') => {
                    output.push(char::from(*byte));
                }
                _ => {
                    let _ = write!(output, "\\{byte:02X}");
                }
            }
        }
        output.push_str("\", align 1\n");
    }
    if writes
        .iter()
        .any(|write| matches!(write, NativeWrite::Text(_)))
    {
        output.push('\n');
    }
    if writes
        .iter()
        .any(|write| matches!(write, NativeWrite::Boolean(_)))
    {
        output.push_str(
            "@.hare.boolean.true = private unnamed_addr constant [5 x i8] c\"true\\0A\", align 1\n",
        );
        output.push_str(
            "@.hare.boolean.false = private unnamed_addr constant [6 x i8] c\"false\\0A\", align 1\n\n",
        );
    }
    output.push_str("declare i64 @Bun__Hare__writeStdout(ptr, i64) nounwind\n");
    output.push_str("declare i64 @Bun__Hare__writeInt64Line(i64) nounwind\n\n");

    for function in functions.values() {
        let parameters = (0..function.parameter_count)
            .map(|index| format!("i64 %arg{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "define internal i64 @hare.fn.{}({parameters}) nounwind {{\nentry:",
            function.id.0
        );
        let mut emitter = ExpressionEmitter::new(&mut output);
        let result = emitter.emit(&function.result)?;
        let _ = writeln!(output, "  ret i64 {result}\n}}\n");
    }

    output.push_str(
        "define i32 @Bun__Hare__nativeApplicationEntry(i32 %argc, ptr %argv) nounwind {\nentry:\n",
    );
    let mut emitter = ExpressionEmitter::new(&mut output);
    let mut success = "true".to_string();
    for (index, write) in writes.iter().enumerate() {
        let write_ok = match write {
            NativeWrite::Integer(expression) => {
                let value = emitter.emit(expression)?;
                let written = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {written} = call i64 @Bun__Hare__writeInt64Line(i64 {value})"
                );
                let write_ok = emitter.temporary();
                let _ = writeln!(emitter.output, "  {write_ok} = icmp sge i64 {written}, 0");
                write_ok
            }
            NativeWrite::Boolean(expression) => {
                let value = emitter.emit(expression)?;
                let condition = emitter.temporary();
                let _ = writeln!(emitter.output, "  {condition} = icmp ne i64 {value}, 0");
                let pointer = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {pointer} = select i1 {condition}, ptr @.hare.boolean.true, ptr @.hare.boolean.false"
                );
                let length = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {length} = select i1 {condition}, i64 5, i64 6"
                );
                let written = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {written} = call i64 @Bun__Hare__writeStdout(ptr {pointer}, i64 {length})"
                );
                let write_ok = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {write_ok} = icmp eq i64 {written}, {length}"
                );
                write_ok
            }
            NativeWrite::Text(bytes) => {
                let written = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {written} = call i64 @Bun__Hare__writeStdout(ptr @.hare.write.{index}, i64 {})",
                    bytes.len()
                );
                let write_ok = emitter.temporary();
                let _ = writeln!(
                    emitter.output,
                    "  {write_ok} = icmp eq i64 {written}, {}",
                    bytes.len()
                );
                write_ok
            }
        };
        if success == "true" {
            success = write_ok;
        } else {
            let combined = emitter.temporary();
            let _ = writeln!(
                emitter.output,
                "  {combined} = and i1 {success}, {write_ok}"
            );
            success = combined;
        }
    }
    let _ = writeln!(
        emitter.output,
        "  %status = select i1 {success}, i32 0, i32 1\n  ret i32 %status\n}}"
    );
    Ok(output)
}

struct ExpressionEmitter<'a> {
    output: &'a mut String,
    next_temporary: usize,
}

impl<'a> ExpressionEmitter<'a> {
    fn new(output: &'a mut String) -> Self {
        Self {
            output,
            next_temporary: 0,
        }
    }

    fn temporary(&mut self) -> String {
        let temporary = format!("%v{}", self.next_temporary);
        self.next_temporary += 1;
        temporary
    }

    fn emit(&mut self, expression: &ScalarExpression) -> Result<String, LlvmError> {
        match expression {
            ScalarExpression::Integer(value) => Ok(value.to_string()),
            ScalarExpression::Parameter(index) => Ok(format!("%arg{index}")),
            ScalarExpression::Negate(value) => {
                let value = self.emit(value)?;
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = sub i64 0, {value}");
                Ok(result)
            }
            ScalarExpression::BitNot(value) => {
                let value = self.emit_i32(value)?;
                let inverted = self.temporary();
                let _ = writeln!(self.output, "  {inverted} = xor i32 {value}, -1");
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = sext i32 {inverted} to i64");
                Ok(result)
            }
            ScalarExpression::Unsigned(value) => {
                let value = self.emit_i32(value)?;
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = zext i32 {value} to i64");
                Ok(result)
            }
            ScalarExpression::LogicalNot(value) => {
                let value = self.emit(value)?;
                let condition = self.temporary();
                let _ = writeln!(self.output, "  {condition} = icmp eq i64 {value}, 0");
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = zext i1 {condition} to i64");
                Ok(result)
            }
            ScalarExpression::Equal(left, right)
            | ScalarExpression::NotEqual(left, right)
            | ScalarExpression::Less(left, right)
            | ScalarExpression::LessEqual(left, right)
            | ScalarExpression::Greater(left, right)
            | ScalarExpression::GreaterEqual(left, right) => {
                let left = self.emit(left)?;
                let right = self.emit(right)?;
                let predicate = match expression {
                    ScalarExpression::Equal(_, _) => "eq",
                    ScalarExpression::NotEqual(_, _) => "ne",
                    ScalarExpression::Less(_, _) => "slt",
                    ScalarExpression::LessEqual(_, _) => "sle",
                    ScalarExpression::Greater(_, _) => "sgt",
                    ScalarExpression::GreaterEqual(_, _) => "sge",
                    _ => unreachable!(),
                };
                let condition = self.temporary();
                let _ = writeln!(
                    self.output,
                    "  {condition} = icmp {predicate} i64 {left}, {right}"
                );
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = zext i1 {condition} to i64");
                Ok(result)
            }
            ScalarExpression::Below(left, right) | ScalarExpression::BelowEqual(left, right) => {
                let left = self.emit_i32(left)?;
                let right = self.emit_i32(right)?;
                let predicate = if matches!(expression, ScalarExpression::Below(_, _)) {
                    "ult"
                } else {
                    "ule"
                };
                let condition = self.temporary();
                let _ = writeln!(
                    self.output,
                    "  {condition} = icmp {predicate} i32 {left}, {right}"
                );
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = zext i1 {condition} to i64");
                Ok(result)
            }
            ScalarExpression::Call {
                function,
                arguments,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.emit(argument).map(|value| format!("i64 {value}")))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                let result = self.temporary();
                let _ = writeln!(
                    self.output,
                    "  {result} = call i64 @hare.fn.{}({arguments})",
                    function.0
                );
                Ok(result)
            }
            ScalarExpression::Power(left, right) => {
                let ScalarExpression::Integer(exponent) = right.as_ref() else {
                    return Err(imported_error(
                        "native integer power requires a closed constant exponent",
                    ));
                };
                let exponent = u32::try_from(*exponent).map_err(|_| {
                    imported_error("native integer power exponent must be non-negative")
                })?;
                if exponent > 64 {
                    return Err(imported_error(
                        "native integer power exponent exceeds the unrolled limit",
                    ));
                }
                let base = self.emit(left)?;
                let mut result = "1".to_string();
                for _ in 0..exponent {
                    let product = self.temporary();
                    let _ = writeln!(self.output, "  {product} = mul i64 {result}, {base}");
                    result = product;
                }
                Ok(result)
            }
            ScalarExpression::Add(left, right)
            | ScalarExpression::Subtract(left, right)
            | ScalarExpression::Multiply(left, right)
            | ScalarExpression::Divide(left, right)
            | ScalarExpression::Remainder(left, right) => {
                let left = self.emit(left)?;
                let right = self.emit(right)?;
                let operation = match expression {
                    ScalarExpression::Add(_, _) => "add",
                    ScalarExpression::Subtract(_, _) => "sub",
                    ScalarExpression::Multiply(_, _) => "mul",
                    ScalarExpression::Divide(_, _) => "sdiv",
                    ScalarExpression::Remainder(_, _) => "srem",
                    _ => unreachable!(),
                };
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = {operation} i64 {left}, {right}");
                Ok(result)
            }
            ScalarExpression::BitAnd(left, right)
            | ScalarExpression::BitOr(left, right)
            | ScalarExpression::BitXor(left, right) => {
                let left = self.emit_i32(left)?;
                let right = self.emit_i32(right)?;
                let operation = match expression {
                    ScalarExpression::BitAnd(_, _) => "and",
                    ScalarExpression::BitOr(_, _) => "or",
                    ScalarExpression::BitXor(_, _) => "xor",
                    _ => unreachable!(),
                };
                let raw = self.temporary();
                let _ = writeln!(self.output, "  {raw} = {operation} i32 {left}, {right}");
                let result = self.temporary();
                let _ = writeln!(self.output, "  {result} = sext i32 {raw} to i64");
                Ok(result)
            }
            ScalarExpression::LeftShift(left, right)
            | ScalarExpression::RightShift(left, right)
            | ScalarExpression::UnsignedRightShift(left, right) => {
                let left = self.emit_i32(left)?;
                let right = self.emit_i32(right)?;
                let shift = self.temporary();
                let _ = writeln!(self.output, "  {shift} = and i32 {right}, 31");
                let operation = match expression {
                    ScalarExpression::LeftShift(_, _) => "shl",
                    ScalarExpression::RightShift(_, _) => "ashr",
                    ScalarExpression::UnsignedRightShift(_, _) => "lshr",
                    _ => unreachable!(),
                };
                let raw = self.temporary();
                let _ = writeln!(self.output, "  {raw} = {operation} i32 {left}, {shift}");
                let result = self.temporary();
                let extension = if matches!(expression, ScalarExpression::UnsignedRightShift(_, _))
                {
                    "zext"
                } else {
                    "sext"
                };
                let _ = writeln!(self.output, "  {result} = {extension} i32 {raw} to i64");
                Ok(result)
            }
        }
    }

    fn emit_i32(&mut self, expression: &ScalarExpression) -> Result<String, LlvmError> {
        let value = self.emit(expression)?;
        let result = self.temporary();
        let _ = writeln!(self.output, "  {result} = trunc i64 {value} to i32");
        Ok(result)
    }
}

fn imported_error(message: impl Into<String>) -> LlvmError {
    LlvmError::ImportedApplication(message.into().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use hare_frontend::ImportedValueKind;
    use hare_ir::{
        ConstantSourceRepresentation, HARE_IR_SCHEMA_VERSION, InputKind, SourceId, SourceRecord,
        VisitorConstant, VisitorOperand,
    };

    use super::*;

    #[test]
    fn imported_application_rejects_an_empty_unit() {
        let unit = OwnedVisitorUnit {
            schema_version: HARE_IR_SCHEMA_VERSION,
            bun_revision: hare_ir::PINNED_BUN_REVISION.into(),
            webkit_revision: hare_ir::PINNED_WEBKIT_REVISION.into(),
            input_kind: InputKind::ModuleProgram,
            sources: vec![SourceRecord {
                id: SourceId(0),
                public_name: "probe.js".into(),
                text: SourceText::Latin1(Box::default()),
                start_line: 1,
                start_column: 0,
            }],
            functions: Vec::new(),
            definition_coverage: BTreeMap::new(),
            structurally_complete: false,
        };
        assert!(matches!(
            compile_imported_scalar_application(&unit, TargetLayout::host().unwrap()),
            Err(LlvmError::ImportedApplication(_))
        ));
    }

    #[test]
    fn closed_scalar_graph_emits_a_direct_native_call() {
        let function = ScalarFunction {
            id: FunctionId(1),
            parameter_count: 1,
            result_kind: ScalarKind::Integer,
            result: ScalarExpression::Add(
                Box::new(ScalarExpression::Parameter(0)),
                Box::new(ScalarExpression::Integer(1)),
            ),
        };
        let functions = BTreeMap::from([(function.id, function)]);
        let expression = ScalarExpression::Call {
            function: FunctionId(1),
            arguments: vec![ScalarExpression::Integer(41)],
        };
        let writes = [NativeWrite::Integer(expression.clone())];
        assert_eq!(evaluate(&expression, &functions, &[], 0).unwrap(), 42);
        let ir = emit_application(TargetLayout::host().unwrap(), &functions, &writes).unwrap();
        assert!(ir.contains("define internal i64 @hare.fn.1(i64 %arg0)"));
        assert!(ir.contains(" = add i64 %arg0, 1"));
        assert!(ir.contains("call i64 @hare.fn.1(i64 41)"));
        assert!(ir.contains("@Bun__Hare__writeInt64Line"));
    }

    #[test]
    fn closed_scalar_graph_rejects_negative_zero_edges() {
        let functions = BTreeMap::new();
        for expression in [
            ScalarExpression::Negate(Box::new(ScalarExpression::Integer(0))),
            ScalarExpression::Multiply(
                Box::new(ScalarExpression::Integer(0)),
                Box::new(ScalarExpression::Integer(-1)),
            ),
            ScalarExpression::Divide(
                Box::new(ScalarExpression::Integer(0)),
                Box::new(ScalarExpression::Integer(-1)),
            ),
            ScalarExpression::Remainder(
                Box::new(ScalarExpression::Integer(-4)),
                Box::new(ScalarExpression::Integer(2)),
            ),
        ] {
            assert!(matches!(
                evaluate(&expression, &functions, &[], 0),
                Err(LlvmError::ImportedApplication(_))
            ));
        }
    }

    #[test]
    fn bitwise_graph_uses_javascript_i32_widths() {
        let functions = BTreeMap::new();
        let expressions = [
            (
                ScalarExpression::BitAnd(
                    Box::new(ScalarExpression::Integer(29)),
                    Box::new(ScalarExpression::Integer(15)),
                ),
                13,
            ),
            (
                ScalarExpression::LeftShift(
                    Box::new(ScalarExpression::Integer(1)),
                    Box::new(ScalarExpression::Integer(31)),
                ),
                -2_147_483_648,
            ),
            (
                ScalarExpression::UnsignedRightShift(
                    Box::new(ScalarExpression::Integer(-2)),
                    Box::new(ScalarExpression::Integer(1)),
                ),
                2_147_483_647,
            ),
            (
                ScalarExpression::BitNot(Box::new(ScalarExpression::Integer(0))),
                -1,
            ),
        ];
        for (expression, expected) in &expressions {
            assert_eq!(evaluate(expression, &functions, &[], 0).unwrap(), *expected);
        }
        let writes = expressions
            .into_iter()
            .map(|(expression, _)| NativeWrite::Integer(expression))
            .collect::<Vec<_>>();
        let ir = emit_application(TargetLayout::host().unwrap(), &functions, &writes).unwrap();
        assert!(ir.contains(" = trunc i64 "));
        assert!(ir.contains(" = lshr i32 "));
        assert!(ir.contains(" = zext i32 "));
    }

    fn instruction(opcode: &str, values: &[(&str, OperandValue)]) -> VisitorInstruction {
        let descriptor = hare_frontend::inventories()
            .into_iter()
            .flatten()
            .find(|descriptor| descriptor.opcode == opcode)
            .unwrap();
        let operands = descriptor
            .operands
            .iter()
            .map(|operand| VisitorOperand {
                manifest_id: operand.manifest_id.into(),
                role: operand.role,
                value: values
                    .iter()
                    .find(|(name, _)| *name == operand.name)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_else(|| match operand.value_kind {
                        ImportedValueKind::Signed => OperandValue::Signed(0),
                        ImportedValueKind::Unsigned => OperandValue::Unsigned(0),
                        ImportedValueKind::Boolean => OperandValue::Boolean(false),
                    }),
            })
            .collect();
        VisitorInstruction {
            byte_offset: 0,
            opcode_id: u32::from(descriptor.opcode_id),
            encoded_size: 1,
            opcode_id_bytes: 1,
            width_bytes: 1,
            operands,
        }
    }

    fn test_function(
        constants: Vec<VisitorConstant>,
        instructions: Vec<VisitorInstruction>,
    ) -> VisitorFunction {
        VisitorFunction {
            id: FunctionId(0),
            parent: None,
            relation: FunctionRelation::Root,
            specialization: FunctionSpecialization::Module,
            source: SourceId(0),
            parse_mode: 0,
            script_mode: 0,
            code_type: 0,
            lexical_features: 0,
            code_features: 0,
            num_parameters: 1,
            num_vars: 32,
            num_callee_locals: 32,
            this_register: 5,
            scope_register: -1,
            call_frame_callee_register: 3,
            call_frame_this_argument_register: 5,
            call_frame_first_argument_register: 6,
            constants,
            identifiers: Vec::new(),
            simple_switch_tables: Vec::new(),
            string_switch_tables: Vec::new(),
            exception_handlers: Vec::new(),
            instruction_bytes: instructions.len() as u32,
            instructions,
        }
    }

    fn test_unit(functions: Vec<VisitorFunction>) -> OwnedVisitorUnit {
        OwnedVisitorUnit {
            schema_version: HARE_IR_SCHEMA_VERSION,
            bun_revision: hare_ir::PINNED_BUN_REVISION.into(),
            webkit_revision: hare_ir::PINNED_WEBKIT_REVISION.into(),
            input_kind: InputKind::ModuleProgram,
            sources: vec![SourceRecord {
                id: SourceId(0),
                public_name: "opcode-case.js".into(),
                text: SourceText::Latin1(Box::default()),
                start_line: 1,
                start_column: 0,
            }],
            functions,
            definition_coverage: BTreeMap::new(),
            structurally_complete: true,
        }
    }

    fn lower_test_function(
        function: VisitorFunction,
        explicit_arguments: Option<&[RegisterValue]>,
    ) -> (LoweredBody, StaticExecutionState) {
        let mut state = StaticExecutionState::default();
        let body =
            lower_test_function_with_context(function, explicit_arguments, None, None, &mut state);
        (body, state)
    }

    fn lower_test_function_with_context(
        function: VisitorFunction,
        explicit_arguments: Option<&[RegisterValue]>,
        explicit_environment: Option<StaticEnvironmentRef>,
        explicit_callee: Option<RegisterValue>,
        state: &mut StaticExecutionState,
    ) -> LoweredBody {
        let unit = test_unit(vec![function.clone()]);
        lower_function(
            &unit,
            &function,
            &BTreeMap::new(),
            &BTreeMap::new(),
            explicit_arguments,
            explicit_environment,
            None,
            explicit_callee,
            state,
            0,
        )
        .unwrap()
    }

    #[test]
    fn async_from_sync_array_iterator_preserves_value_and_done_order() {
        let array_template = FIRST_CONSTANT_REGISTER_INDEX;
        let undefined = FIRST_CONSTANT_REGISTER_INDEX + 1;
        let function = test_function(
            vec![
                VisitorConstant {
                    value: VisitorConstantValue::ImmutableArray {
                        elements: vec![
                            VisitorConstantValue::Int32(40),
                            VisitorConstantValue::Int32(2),
                        ],
                        indexing_type: 0,
                    },
                    source_representation: ConstantSourceRepresentation::Other,
                },
                VisitorConstant {
                    value: VisitorConstantValue::Undefined,
                    source_representation: ConstantSourceRepresentation::Other,
                },
            ],
            vec![
                instruction(
                    "op_new_array_buffer",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("immutableButterfly", OperandValue::Signed(array_template)),
                    ],
                ),
                instruction(
                    "op_async_iterator_open",
                    &[
                        ("iterator", OperandValue::Signed(-3)),
                        ("next", OperandValue::Signed(-4)),
                        ("symbolIterator", OperandValue::Signed(undefined)),
                        ("iterable", OperandValue::Signed(-2)),
                    ],
                ),
                instruction(
                    "op_async_iterator_next",
                    &[
                        ("dst", OperandValue::Signed(-5)),
                        ("next", OperandValue::Signed(-4)),
                        ("iterator", OperandValue::Signed(-3)),
                        ("driver", OperandValue::Signed(undefined)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-5))]),
            ],
        );
        let (body, state) = lower_test_function(function, None);
        let Some(RegisterValue::FulfilledPromise(result)) = body.result else {
            panic!("async-from-sync next did not return a fulfilled promise");
        };
        assert_eq!(
            static_get_property(
                &state.heap,
                &result,
                StaticPropertyKey::Name("value".into())
            )
            .unwrap(),
            RegisterValue::Scalar(ScalarExpression::Integer(40))
        );
        assert_eq!(
            static_get_property(&state.heap, &result, StaticPropertyKey::Name("done".into()))
                .unwrap(),
            RegisterValue::Boolean(false)
        );
    }

    #[test]
    fn direct_arguments_reads_observe_indexed_writes() {
        let forty_two = FIRST_CONSTANT_REGISTER_INDEX;
        let function = test_function(
            vec![VisitorConstant {
                value: VisitorConstantValue::Int32(42),
                source_representation: ConstantSourceRepresentation::Integer,
            }],
            vec![
                instruction(
                    "op_create_direct_arguments",
                    &[("dst", OperandValue::Signed(-2))],
                ),
                instruction(
                    "op_put_to_arguments",
                    &[
                        ("arguments", OperandValue::Signed(-2)),
                        ("index", OperandValue::Unsigned(0)),
                        ("value", OperandValue::Signed(forty_two)),
                    ],
                ),
                instruction(
                    "op_get_from_arguments",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("arguments", OperandValue::Signed(-2)),
                        ("index", OperandValue::Unsigned(0)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-3))]),
            ],
        );
        let (body, _) = lower_test_function(
            function,
            Some(&[RegisterValue::Scalar(ScalarExpression::Integer(41))]),
        );
        assert_eq!(
            body.result,
            Some(RegisterValue::Scalar(ScalarExpression::Integer(42)))
        );
    }

    #[test]
    fn scoped_arguments_and_named_parameter_share_storage() {
        let symbol_table = FIRST_CONSTANT_REGISTER_INDEX;
        let undefined = FIRST_CONSTANT_REGISTER_INDEX + 1;
        let zero = FIRST_CONSTANT_REGISTER_INDEX + 2;
        let forty_two = FIRST_CONSTANT_REGISTER_INDEX + 3;
        let mut function = test_function(
            vec![
                VisitorConstant {
                    value: VisitorConstantValue::UnimplementedCell("SymbolTable".into()),
                    source_representation: ConstantSourceRepresentation::Other,
                },
                VisitorConstant {
                    value: VisitorConstantValue::Undefined,
                    source_representation: ConstantSourceRepresentation::Other,
                },
                VisitorConstant {
                    value: VisitorConstantValue::Int32(0),
                    source_representation: ConstantSourceRepresentation::Integer,
                },
                VisitorConstant {
                    value: VisitorConstantValue::Int32(42),
                    source_representation: ConstantSourceRepresentation::Integer,
                },
            ],
            vec![
                instruction(
                    "op_create_lexical_environment",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("scope", OperandValue::Signed(-1)),
                        ("symbolTable", OperandValue::Signed(symbol_table)),
                        ("initialValue", OperandValue::Signed(undefined)),
                    ],
                ),
                instruction(
                    "op_put_to_scope",
                    &[
                        ("scope", OperandValue::Signed(-2)),
                        ("var", OperandValue::Unsigned(0)),
                        ("value", OperandValue::Signed(6)),
                        ("getPutInfo", OperandValue::Unsigned(3 << 10)),
                    ],
                ),
                instruction(
                    "op_create_scoped_arguments",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("scope", OperandValue::Signed(-2)),
                    ],
                ),
                instruction(
                    "op_put_by_val",
                    &[
                        ("base", OperandValue::Signed(-3)),
                        ("property", OperandValue::Signed(zero)),
                        ("value", OperandValue::Signed(forty_two)),
                    ],
                ),
                instruction(
                    "op_get_from_scope",
                    &[
                        ("dst", OperandValue::Signed(-4)),
                        ("scope", OperandValue::Signed(-2)),
                        ("var", OperandValue::Unsigned(0)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-4))]),
            ],
        );
        function
            .identifiers
            .push(SourceText::Latin1(b"value".to_vec().into_boxed_slice()));
        let (body, _) = lower_test_function(
            function,
            Some(&[RegisterValue::Scalar(ScalarExpression::Integer(41))]),
        );
        assert_eq!(
            body.result,
            Some(RegisterValue::Scalar(ScalarExpression::Integer(42)))
        );
    }

    #[test]
    fn bigint_constants_keep_arbitrary_precision_and_type() {
        let bigint = FIRST_CONSTANT_REGISTER_INDEX;
        let function = test_function(
            vec![VisitorConstant {
                value: VisitorConstantValue::BigInt {
                    negative: true,
                    magnitude_be: vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 1].into_boxed_slice(),
                },
                source_representation: ConstantSourceRepresentation::Other,
            }],
            vec![
                instruction(
                    "op_is_big_int",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("operand", OperandValue::Signed(bigint)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-2))]),
            ],
        );
        let (body, _) = lower_test_function(function, None);
        assert_eq!(body.result, Some(RegisterValue::Boolean(true)));
        assert_eq!(
            bigint_decimal(true, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            "-4722366482869645213697"
        );
        assert_eq!(
            normalize_write(
                RegisterValue::BigInt {
                    negative: false,
                    magnitude_be: Box::new([42]),
                },
                &BTreeMap::new(),
            )
            .unwrap(),
            NativeWrite::Text(b"42n\n".to_vec().into_boxed_slice())
        );
    }

    #[test]
    fn tier1_internal_promise_and_generator_allocations_preserve_semantics() {
        let mut state = StaticExecutionState::default();
        state.heap.insert(
            1,
            StaticHeapEntry::Object {
                prototype: None,
                properties: BTreeMap::new(),
                property_order: Vec::new(),
                private_properties: BTreeMap::new(),
                private_brands: BTreeSet::new(),
            },
        );
        state.heap.insert(
            0,
            StaticHeapEntry::Function {
                prototype: None,
                properties: BTreeMap::from([(
                    "prototype".into(),
                    StaticProperty::assigned(RegisterValue::Object(1)),
                )]),
                property_order: vec!["prototype".into()],
                private_properties: BTreeMap::new(),
                private_brands: BTreeSet::new(),
            },
        );
        state.next_heap_id = 2;
        let callee = RegisterValue::Function {
            call: FunctionId(0),
            construct: Some(FunctionId(0)),
            environment: None,
            identity: 0,
            heap_id: 0,
        };
        let promise = test_function(
            Vec::new(),
            vec![
                instruction(
                    "op_create_promise",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("callee", OperandValue::Signed(3)),
                    ],
                ),
                instruction(
                    "op_get_prototype_of",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("value", OperandValue::Signed(-2)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-3))]),
            ],
        );
        let body = lower_test_function_with_context(promise, None, None, Some(callee), &mut state);
        assert_eq!(body.result, Some(RegisterValue::Object(1)));

        let generator = test_function(
            Vec::new(),
            vec![
                instruction("op_new_generator", &[("dst", OperandValue::Signed(-2))]),
                instruction(
                    "op_is_object",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("operand", OperandValue::Signed(-2)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-3))]),
            ],
        );
        let body = lower_test_function_with_context(generator, None, None, None, &mut state);
        assert_eq!(body.result, Some(RegisterValue::Boolean(true)));
    }

    #[test]
    fn tier1_internal_array_species_uses_default_array_constructor() {
        let length = FIRST_CONSTANT_REGISTER_INDEX;
        let function = test_function(
            vec![VisitorConstant {
                value: VisitorConstantValue::Int32(3),
                source_representation: ConstantSourceRepresentation::Integer,
            }],
            vec![
                instruction(
                    "op_new_array_with_size",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("length", OperandValue::Signed(length)),
                    ],
                ),
                instruction(
                    "op_new_array_with_species",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("length", OperandValue::Signed(length)),
                        ("array", OperandValue::Signed(-2)),
                    ],
                ),
                instruction(
                    "op_get_length",
                    &[
                        ("dst", OperandValue::Signed(-4)),
                        ("base", OperandValue::Signed(-3)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-4))]),
            ],
        );
        let (body, _) = lower_test_function(function, None);
        assert_eq!(
            body.result,
            Some(RegisterValue::Scalar(ScalarExpression::Integer(3)))
        );
    }

    #[test]
    fn tier1_internal_object_conversion_and_accessor_flags_match_jsc() {
        let text = FIRST_CONSTANT_REGISTER_INDEX;
        let message = FIRST_CONSTANT_REGISTER_INDEX + 1;
        let boxed = test_function(
            vec![
                VisitorConstant {
                    value: VisitorConstantValue::String(SourceText::Latin1(
                        b"hare".to_vec().into_boxed_slice(),
                    )),
                    source_representation: ConstantSourceRepresentation::Other,
                },
                VisitorConstant {
                    value: VisitorConstantValue::String(SourceText::Latin1(
                        b"Cannot convert value to object"
                            .to_vec()
                            .into_boxed_slice(),
                    )),
                    source_representation: ConstantSourceRepresentation::Other,
                },
            ],
            vec![
                instruction(
                    "op_to_object",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("operand", OperandValue::Signed(text)),
                        ("message", OperandValue::Signed(message)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-2))]),
            ],
        );
        let (body, state) = lower_test_function(boxed, None);
        let boxed = body.result.unwrap();
        assert_eq!(
            static_get_property(
                &state.heap,
                &boxed,
                StaticPropertyKey::Name("length".into())
            )
            .unwrap(),
            RegisterValue::Scalar(ScalarExpression::Integer(4))
        );
        assert_eq!(
            static_get_property(&state.heap, &boxed, StaticPropertyKey::Index(0)).unwrap(),
            RegisterValue::String("h".into())
        );

        let property = FIRST_CONSTANT_REGISTER_INDEX;
        let undefined = FIRST_CONSTANT_REGISTER_INDEX + 1;
        let attributes = FIRST_CONSTANT_REGISTER_INDEX + 2;
        let accessor = test_function(
            vec![
                VisitorConstant {
                    value: VisitorConstantValue::String(SourceText::Latin1(
                        b"value".to_vec().into_boxed_slice(),
                    )),
                    source_representation: ConstantSourceRepresentation::Other,
                },
                VisitorConstant {
                    value: VisitorConstantValue::Undefined,
                    source_representation: ConstantSourceRepresentation::Other,
                },
                VisitorConstant {
                    value: VisitorConstantValue::Int32((2 << 4) | (1 << 7)),
                    source_representation: ConstantSourceRepresentation::Integer,
                },
            ],
            vec![
                instruction("op_new_object", &[("dst", OperandValue::Signed(-2))]),
                instruction(
                    "op_define_accessor_property",
                    &[
                        ("base", OperandValue::Signed(-2)),
                        ("property", OperandValue::Signed(property)),
                        ("getter", OperandValue::Signed(undefined)),
                        ("setter", OperandValue::Signed(undefined)),
                        ("attributes", OperandValue::Signed(attributes)),
                    ],
                ),
                instruction(
                    "op_has_structure_with_flags",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("operand", OperandValue::Signed(-2)),
                        ("flags", OperandValue::Unsigned(1 << 30)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-3))]),
            ],
        );
        let (body, state) = lower_test_function(accessor, None);
        assert_eq!(body.result, Some(RegisterValue::Boolean(true)));
        let Some(StaticHeapEntry::Object { properties, .. }) = state.heap.get(&0) else {
            panic!("accessor base was not allocated as an object");
        };
        let property = properties.get("value").unwrap();
        assert!(property.is_accessor);
        assert!(!property.configurable);
    }

    #[test]
    fn tier1_internal_property_key_and_identity_preserve_values() {
        let value = FIRST_CONSTANT_REGISTER_INDEX;
        let function = test_function(
            vec![VisitorConstant {
                value: VisitorConstantValue::Boolean(true),
                source_representation: ConstantSourceRepresentation::Other,
            }],
            vec![
                instruction(
                    "op_to_property_key_or_number",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("src", OperandValue::Signed(value)),
                    ],
                ),
                instruction(
                    "op_identity_with_profile",
                    &[("srcDst", OperandValue::Signed(-2))],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-2))]),
            ],
        );
        let (body, _) = lower_test_function(function, None);
        assert_eq!(body.result, Some(RegisterValue::String("true".into())));
    }

    #[test]
    fn tier1_internal_scope_operations_keep_var_and_callee_scope_identity() {
        let root = Rc::new(RefCell::new(StaticEnvironment {
            kind: StaticEnvironmentKind::Var,
            parent: None,
            object_scope: None,
            bindings: BTreeMap::new(),
            scoped_argument_values: BTreeMap::new(),
            scoped_argument_names: BTreeMap::new(),
        }));
        let symbol_table = FIRST_CONSTANT_REGISTER_INDEX;
        let undefined = FIRST_CONSTANT_REGISTER_INDEX + 1;
        let value = FIRST_CONSTANT_REGISTER_INDEX + 2;
        let constants = vec![
            VisitorConstant {
                value: VisitorConstantValue::UnimplementedCell("SymbolTable".into()),
                source_representation: ConstantSourceRepresentation::Other,
            },
            VisitorConstant {
                value: VisitorConstantValue::Undefined,
                source_representation: ConstantSourceRepresentation::Other,
            },
            VisitorConstant {
                value: VisitorConstantValue::Int32(42),
                source_representation: ConstantSourceRepresentation::Integer,
            },
        ];
        let mut with_scope = test_function(
            constants.clone(),
            vec![
                instruction(
                    "op_create_lexical_environment",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("scope", OperandValue::Signed(-1)),
                        ("symbolTable", OperandValue::Signed(symbol_table)),
                        ("initialValue", OperandValue::Signed(undefined)),
                    ],
                ),
                instruction("op_new_object", &[("dst", OperandValue::Signed(-3))]),
                instruction(
                    "op_put_by_id",
                    &[
                        ("base", OperandValue::Signed(-3)),
                        ("property", OperandValue::Unsigned(0)),
                        ("value", OperandValue::Signed(value)),
                    ],
                ),
                instruction(
                    "op_push_with_scope",
                    &[
                        ("dst", OperandValue::Signed(-4)),
                        ("currentScope", OperandValue::Signed(-2)),
                        ("newScope", OperandValue::Signed(-3)),
                    ],
                ),
                instruction(
                    "op_get_from_scope",
                    &[
                        ("dst", OperandValue::Signed(-5)),
                        ("scope", OperandValue::Signed(-4)),
                        ("var", OperandValue::Unsigned(0)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-5))]),
            ],
        );
        with_scope
            .identifiers
            .push(SourceText::Latin1(b"value".to_vec().into_boxed_slice()));
        let mut state = StaticExecutionState::default();
        let body = lower_test_function_with_context(
            with_scope,
            None,
            Some(root.clone()),
            None,
            &mut state,
        );
        assert_eq!(
            body.result,
            Some(RegisterValue::Scalar(ScalarExpression::Integer(42)))
        );

        let mut hoist = test_function(
            constants.clone(),
            vec![
                instruction(
                    "op_create_lexical_environment",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("scope", OperandValue::Signed(-1)),
                        ("symbolTable", OperandValue::Signed(symbol_table)),
                        ("initialValue", OperandValue::Signed(undefined)),
                    ],
                ),
                instruction("op_new_object", &[("dst", OperandValue::Signed(-3))]),
                instruction(
                    "op_push_with_scope",
                    &[
                        ("dst", OperandValue::Signed(-4)),
                        ("currentScope", OperandValue::Signed(-2)),
                        ("newScope", OperandValue::Signed(-3)),
                    ],
                ),
                instruction(
                    "op_resolve_scope_for_hoisting_func_decl_in_eval",
                    &[
                        ("dst", OperandValue::Signed(-5)),
                        ("scope", OperandValue::Signed(-4)),
                        ("property", OperandValue::Unsigned(0)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-5))]),
            ],
        );
        hoist
            .identifiers
            .push(SourceText::Latin1(b"value".to_vec().into_boxed_slice()));
        let body =
            lower_test_function_with_context(hoist, None, Some(root.clone()), None, &mut state);
        let Some(RegisterValue::Environment(resolved)) = body.result else {
            panic!("eval hoist did not resolve a variable environment");
        };
        assert!(Rc::ptr_eq(&resolved, &root));

        let generator_frame = test_function(
            constants,
            vec![
                instruction(
                    "op_create_generator_frame_environment",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("scope", OperandValue::Signed(-1)),
                        ("symbolTable", OperandValue::Signed(symbol_table)),
                        ("initialValue", OperandValue::Signed(undefined)),
                    ],
                ),
                instruction(
                    "op_get_parent_scope",
                    &[
                        ("dst", OperandValue::Signed(-3)),
                        ("scope", OperandValue::Signed(-2)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-3))]),
            ],
        );
        let body = lower_test_function_with_context(
            generator_frame,
            None,
            Some(root.clone()),
            None,
            &mut state,
        );
        let Some(RegisterValue::Environment(parent)) = body.result else {
            panic!("generator frame did not preserve its parent scope");
        };
        assert!(Rc::ptr_eq(&parent, &root));

        let get_scope = test_function(
            Vec::new(),
            vec![
                instruction("op_new_object", &[("dst", OperandValue::Signed(-2))]),
                instruction(
                    "op_push_with_scope",
                    &[
                        ("dst", OperandValue::Signed(-1)),
                        ("currentScope", OperandValue::Signed(-1)),
                        ("newScope", OperandValue::Signed(-2)),
                    ],
                ),
                instruction("op_get_scope", &[("dst", OperandValue::Signed(-3))]),
                instruction("op_ret", &[("value", OperandValue::Signed(-3))]),
            ],
        );
        let body =
            lower_test_function_with_context(get_scope, None, Some(root.clone()), None, &mut state);
        let Some(RegisterValue::Environment(scope)) = body.result else {
            panic!("get_scope did not return the callee scope");
        };
        assert!(Rc::ptr_eq(&scope, &root));
    }

    #[test]
    fn tier1_internal_predicates_and_yield_preserve_values() {
        let undefined = FIRST_CONSTANT_REGISTER_INDEX;
        let function = test_function(
            vec![VisitorConstant {
                value: VisitorConstantValue::Undefined,
                source_representation: ConstantSourceRepresentation::Other,
            }],
            vec![
                instruction(
                    "op_is_undefined_or_null",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("operand", OperandValue::Signed(undefined)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-2))]),
            ],
        );
        let (body, _) = lower_test_function(function, None);
        assert_eq!(body.result, Some(RegisterValue::Boolean(true)));

        let callable = RegisterValue::Function {
            call: FunctionId(0),
            construct: None,
            environment: None,
            identity: 0,
            heap_id: 0,
        };
        let function = test_function(
            Vec::new(),
            vec![
                instruction(
                    "op_identity_with_profile",
                    &[("srcDst", OperandValue::Signed(3))],
                ),
                instruction(
                    "op_is_callable",
                    &[
                        ("dst", OperandValue::Signed(-2)),
                        ("operand", OperandValue::Signed(3)),
                    ],
                ),
                instruction("op_ret", &[("value", OperandValue::Signed(-2))]),
            ],
        );
        let mut state = StaticExecutionState::default();
        let body =
            lower_test_function_with_context(function, None, None, Some(callable), &mut state);
        assert_eq!(body.result, Some(RegisterValue::Boolean(true)));

        let value = FIRST_CONSTANT_REGISTER_INDEX;
        let function = test_function(
            vec![VisitorConstant {
                value: VisitorConstantValue::Int32(42),
                source_representation: ConstantSourceRepresentation::Integer,
            }],
            vec![instruction(
                "op_yield",
                &[
                    ("yieldPoint", OperandValue::Unsigned(0)),
                    ("argument", OperandValue::Signed(value)),
                ],
            )],
        );
        let (body, _) = lower_test_function(function, None);
        assert_eq!(
            body.result,
            Some(RegisterValue::Scalar(ScalarExpression::Integer(42)))
        );
    }
}
