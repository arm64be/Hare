use std::collections::BTreeMap;
use std::fmt::Write as _;

use hare_ir::{
    FIRST_CONSTANT_REGISTER_INDEX, FunctionId, FunctionRelation, OperandValue, OwnedVisitorUnit,
    SourceText, VisitorConstantValue, VisitorFunction, VisitorInstruction,
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
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScalarFunction {
    id: FunctionId,
    parameter_count: usize,
    result_kind: ScalarKind,
    result: ScalarExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RegisterValue {
    Scalar(ScalarExpression),
    String(Box<str>),
    Concatenation(Vec<Self>),
    BooleanScalar(ScalarExpression),
    Function(FunctionId),
    ConsoleScope,
    NaNScope,
    InfinityScope,
    ConsoleObject,
    ConsoleLog,
    ConsoleNoArgument,
    BuiltinScope(Builtin),
    Builtin(Builtin),
    Object(u32),
    Array(u32),
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
        properties: BTreeMap<Box<str>, RegisterValue>,
    },
    Array {
        elements: BTreeMap<u32, RegisterValue>,
        properties: BTreeMap<Box<str>, RegisterValue>,
        length: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StaticPropertyKey {
    Index(u32),
    Name(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoweredBody {
    result: Option<(ScalarKind, ScalarExpression)>,
    writes: Vec<RegisterValue>,
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

    let mut functions = BTreeMap::new();
    for function in unit.functions.iter().skip(1) {
        let body = lower_function(unit, function, &functions)?;
        if !body.writes.is_empty() {
            return Err(imported_error(format!(
                "f{} performs output inside a callable function",
                function.id.0
            )));
        }
        let (result_kind, result) = body.result.ok_or_else(|| {
            imported_error(format!("f{} has no scalar return value", function.id.0))
        })?;
        functions.insert(
            function.id,
            ScalarFunction {
                id: function.id,
                parameter_count: function.num_parameters.saturating_sub(1) as usize,
                result_kind,
                result,
            },
        );
    }

    let root_body = lower_function(unit, root, &functions)?;
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

fn lower_function(
    unit: &OwnedVisitorUnit,
    function: &VisitorFunction,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
) -> Result<LoweredBody, LlvmError> {
    let mut registers = BTreeMap::new();
    registers.insert(function.scope_register as i64, RegisterValue::Opaque);
    registers.insert(function.this_register as i64, RegisterValue::Opaque);
    registers.insert(
        function.call_frame_this_argument_register as i64,
        RegisterValue::Undefined,
    );
    let parameter_count = function.num_parameters.saturating_sub(1) as usize;
    for index in 0..parameter_count {
        registers.insert(
            i64::from(function.call_frame_first_argument_register) + index as i64,
            RegisterValue::Scalar(ScalarExpression::Parameter(index)),
        );
    }

    let mut writes = Vec::new();
    let mut result = None;
    let mut heap = BTreeMap::new();
    let mut next_heap_id = 0_u32;
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
            "op_new_object" => {
                let destination = signed_operand(instruction, "dst")?;
                let id = next_heap_id;
                next_heap_id = next_heap_id
                    .checked_add(1)
                    .ok_or_else(|| imported_error("static heap id overflow"))?;
                heap.insert(
                    id,
                    StaticHeapEntry::Object {
                        properties: BTreeMap::new(),
                    },
                );
                registers.insert(destination, RegisterValue::Object(id));
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
                    elements.insert(index, read_register(function, &registers, register)?);
                }
                let length = u32::try_from(count)
                    .map_err(|_| imported_error("array literal length exceeds u32"))?;
                let id = next_heap_id;
                next_heap_id = next_heap_id
                    .checked_add(1)
                    .ok_or_else(|| imported_error("static heap id overflow"))?;
                heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        elements,
                        properties: BTreeMap::new(),
                        length,
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
                let id = next_heap_id;
                next_heap_id = next_heap_id
                    .checked_add(1)
                    .ok_or_else(|| imported_error("static heap id overflow"))?;
                heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        elements: BTreeMap::new(),
                        properties: BTreeMap::new(),
                        length,
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
                }) = heap.get(&id)
                else {
                    return Err(imported_error("spread references a missing static array"));
                };
                let values = (0..*length)
                    .map(|index| {
                        elements
                            .get(&index)
                            .cloned()
                            .unwrap_or(RegisterValue::Undefined)
                    })
                    .collect();
                registers.insert(destination, RegisterValue::Spread(values));
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
                    .map(|(index, value)| (index as u32, value))
                    .collect();
                let id = next_heap_id;
                next_heap_id = next_heap_id
                    .checked_add(1)
                    .ok_or_else(|| imported_error("static heap id overflow"))?;
                heap.insert(
                    id,
                    StaticHeapEntry::Array {
                        elements,
                        properties: BTreeMap::new(),
                        length,
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
                static_put_property(&mut heap, &base, StaticPropertyKey::Name(property), value)?;
            }
            "op_new_func" | "op_new_func_exp" => {
                let destination = signed_operand(instruction, "dst")?;
                let index = unsigned_operand(instruction, "functionDecl")?;
                let child = child_function(unit, function.id, descriptor.opcode, index)?;
                registers.insert(destination, RegisterValue::Function(child));
            }
            "op_resolve_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                let identifier = unsigned_operand(instruction, "var")?;
                if identifier_is(function, identifier, b"console")? {
                    registers.insert(destination, RegisterValue::ConsoleScope);
                } else if identifier_is(function, identifier, b"NaN")? {
                    registers.insert(destination, RegisterValue::NaNScope);
                } else if identifier_is(function, identifier, b"Infinity")? {
                    registers.insert(destination, RegisterValue::InfinityScope);
                } else if let Some(builtin) = builtin_identifier(function, identifier)? {
                    registers.insert(destination, RegisterValue::BuiltinScope(builtin));
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_get_from_scope" => {
                let destination = signed_operand(instruction, "dst")?;
                let scope = signed_operand(instruction, "scope")?;
                let identifier = unsigned_operand(instruction, "var")?;
                if matches!(registers.get(&scope), Some(RegisterValue::ConsoleScope))
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
                } else if let Some(RegisterValue::BuiltinScope(expected)) = registers.get(&scope)
                    && builtin_identifier(function, identifier)? == Some(*expected)
                {
                    registers.insert(destination, RegisterValue::Builtin(*expected));
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_get_by_id" => {
                let destination = signed_operand(instruction, "dst")?;
                let base = signed_operand(instruction, "base")?;
                let property = unsigned_operand(instruction, "property")?;
                if matches!(registers.get(&base), Some(RegisterValue::ConsoleObject))
                    && identifier_is(function, property, b"log")?
                {
                    registers.insert(destination, RegisterValue::ConsoleLog);
                } else if let Some(base) = registers.get(&base) {
                    let property = identifier_string(function, property)?;
                    let value =
                        static_get_property(&heap, base, StaticPropertyKey::Name(property))?;
                    registers.insert(destination, value);
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_get_length" => {
                let destination = signed_operand(instruction, "dst")?;
                let base =
                    read_register(function, &registers, signed_operand(instruction, "base")?)?;
                let length = match base {
                    RegisterValue::Array(id) => match heap.get(&id) {
                        Some(StaticHeapEntry::Array { length, .. }) => i64::from(*length),
                        _ => {
                            return Err(imported_error(
                                "array references a missing static heap entry",
                            ));
                        }
                    },
                    RegisterValue::String(value) => i64::try_from(value.encode_utf16().count())
                        .map_err(|_| imported_error("string length exceeds i64"))?,
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                };
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::Integer(length)),
                );
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
                let value = static_get_property(&heap, &base, property)?;
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
                static_put_property(&mut heap, &base, property, value)?;
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
                let present = static_has_property(&heap, &base, property)?;
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
                static_delete_property(&mut heap, &base, property)?;
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
            "op_eq" | "op_neq" | "op_stricteq" | "op_nstricteq" | "op_less" | "op_lesseq"
            | "op_greater" | "op_greatereq" | "op_below" | "op_beloweq" => {
                let destination = signed_operand(instruction, "dst")?;
                let left =
                    scalar_register(function, &registers, signed_operand(instruction, "lhs")?)?;
                let right =
                    scalar_register(function, &registers, signed_operand(instruction, "rhs")?)?;
                let expression = match descriptor.opcode {
                    "op_eq" | "op_stricteq" => {
                        ScalarExpression::Equal(Box::new(left), Box::new(right))
                    }
                    "op_neq" | "op_nstricteq" => {
                        ScalarExpression::NotEqual(Box::new(left), Box::new(right))
                    }
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
                let _cell_type = unsigned_operand(instruction, "type")?;
                let is_known_non_cell = matches!(
                    value,
                    RegisterValue::Scalar(_)
                        | RegisterValue::BooleanScalar(_)
                        | RegisterValue::Undefined
                        | RegisterValue::Null
                        | RegisterValue::Boolean(_)
                        | RegisterValue::NaN
                        | RegisterValue::NegativeZero
                        | RegisterValue::PositiveInfinity
                        | RegisterValue::NegativeInfinity
                );
                if !is_known_non_cell {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                registers.insert(destination, RegisterValue::Boolean(false));
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
            "op_to_number" | "op_to_numeric" => {
                let destination = signed_operand(instruction, "dst")?;
                let value = read_register(
                    function,
                    &registers,
                    signed_operand(instruction, "operand")?,
                )?;
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
            "op_call" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee_register = signed_operand(instruction, "callee")?;
                let RegisterValue::Function(callee) =
                    read_register(function, &registers, callee_register)?
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let arguments = scalar_call_arguments(function, instruction, &registers)?;
                let callee_function = functions.get(&callee).ok_or_else(|| {
                    imported_error(format!(
                        "f{} calls f{} before its scalar result is available",
                        function.id.0, callee.0
                    ))
                })?;
                let expression = ScalarExpression::Call {
                    function: callee,
                    arguments,
                };
                let value = match callee_function.result_kind {
                    ScalarKind::Integer => RegisterValue::Scalar(expression),
                    ScalarKind::Boolean => RegisterValue::BooleanScalar(expression),
                };
                registers.insert(destination, value);
            }
            "op_call_ignore_result" => {
                let callee = signed_operand(instruction, "callee")?;
                if !matches!(
                    read_register(function, &registers, callee)?,
                    RegisterValue::ConsoleLog
                ) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let mut arguments = call_register_values(function, instruction, &registers)?;
                if arguments.len() > 1 {
                    return Err(imported_error(
                        "imported scalar console.log accepts at most one argument",
                    ));
                }
                writes.push(arguments.pop().unwrap_or(RegisterValue::ConsoleNoArgument));
            }
            "op_ret" => {
                let value = signed_operand(instruction, "value")?;
                match read_register(function, &registers, value)? {
                    RegisterValue::Scalar(expression) => {
                        result = Some((ScalarKind::Integer, expression));
                    }
                    RegisterValue::BooleanScalar(expression) => {
                        result = Some((ScalarKind::Boolean, expression));
                    }
                    RegisterValue::Boolean(value) if function.id.0 != 0 => {
                        result = Some((
                            ScalarKind::Boolean,
                            ScalarExpression::Integer(i64::from(value)),
                        ));
                    }
                    RegisterValue::Empty
                    | RegisterValue::Undefined
                    | RegisterValue::Null
                    | RegisterValue::Boolean(_)
                    | RegisterValue::String(_)
                    | RegisterValue::Concatenation(_)
                    | RegisterValue::NaN
                    | RegisterValue::NegativeZero
                    | RegisterValue::PositiveInfinity
                    | RegisterValue::NegativeInfinity
                    | RegisterValue::Opaque
                        if function.id.0 == 0 => {}
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                }
                break;
            }
            _ => return Err(unsupported(function, instruction, descriptor.opcode)),
        }
        instruction_index += 1;
    }
    Ok(LoweredBody { result, writes })
}

fn branch_target_index(
    instruction: &VisitorInstruction,
    instruction_indices: &BTreeMap<u32, usize>,
) -> Result<usize, LlvmError> {
    let target = i64::from(instruction.byte_offset)
        .checked_add(signed_operand(instruction, "targetLabel")?)
        .and_then(|target| u32::try_from(target).ok())
        .ok_or_else(|| imported_error("branch target leaves the instruction stream"))?;
    instruction_indices
        .get(&target)
        .copied()
        .ok_or_else(|| imported_error(format!("branch target {target} is not an instruction")))
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
        RegisterValue::PositiveInfinity
        | RegisterValue::NegativeInfinity
        | RegisterValue::Function(_)
        | RegisterValue::Object(_)
        | RegisterValue::Array(_)
        | RegisterValue::ConsoleObject
        | RegisterValue::ConsoleLog
        | RegisterValue::Builtin(_) => Some(true),
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

fn known_pointer_equality(left: &RegisterValue, right: &RegisterValue) -> Option<bool> {
    match (left, right) {
        (RegisterValue::Builtin(left), RegisterValue::Builtin(right)) => Some(left == right),
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

fn scalar_call_arguments(
    function: &VisitorFunction,
    instruction: &VisitorInstruction,
    registers: &BTreeMap<i64, RegisterValue>,
) -> Result<Vec<ScalarExpression>, LlvmError> {
    call_register_values(function, instruction, registers)?
        .into_iter()
        .map(|value| {
            let RegisterValue::Scalar(expression) = value else {
                return Err(imported_error("native scalar call received a non-integer"));
            };
            Ok(expression)
        })
        .collect()
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
        return Ok(match &constant.value {
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
            VisitorConstantValue::LinkTimeConstant(name) => match name.as_ref() {
                "Array" => RegisterValue::Builtin(Builtin::Array),
                "Object" => RegisterValue::Builtin(Builtin::Object),
                _ => RegisterValue::Opaque,
            },
            VisitorConstantValue::Empty => RegisterValue::Empty,
            VisitorConstantValue::UnimplementedCell(_) => RegisterValue::Opaque,
        });
    }
    registers.get(&register).cloned().ok_or_else(|| {
        imported_error(format!(
            "f{} reads undefined virtual register {register}",
            function.id.0
        ))
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
            | RegisterValue::Function(_)
            | RegisterValue::Object(_)
            | RegisterValue::Array(_)
            | RegisterValue::ConsoleObject
            | RegisterValue::ConsoleLog
            | RegisterValue::Builtin(_)
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
                | RegisterValue::Object(_)
                | RegisterValue::Array(_)
                | RegisterValue::ConsoleObject
        )),
        "op_typeof_is_function" => Some(matches!(
            value,
            RegisterValue::Function(_) | RegisterValue::ConsoleLog | RegisterValue::Builtin(_)
        )),
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
        "op_is_big_int" => Some(false),
        "op_is_object" => Some(matches!(
            value,
            RegisterValue::Function(_)
                | RegisterValue::Object(_)
                | RegisterValue::Array(_)
                | RegisterValue::ConsoleObject
                | RegisterValue::ConsoleLog
                | RegisterValue::Builtin(_)
        )),
        "op_is_callable" => Some(matches!(
            value,
            RegisterValue::Function(_) | RegisterValue::ConsoleLog | RegisterValue::Builtin(_)
        )),
        "op_is_constructor" => match value {
            RegisterValue::Builtin(_) => Some(true),
            RegisterValue::Function(_) | RegisterValue::ConsoleLog => None,
            _ => Some(false),
        },
        _ => None,
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
        RegisterValue::Function(_) | RegisterValue::ConsoleLog | RegisterValue::Builtin(_) => {
            Some("function")
        }
        RegisterValue::Null
        | RegisterValue::Object(_)
        | RegisterValue::Array(_)
        | RegisterValue::ConsoleObject => Some("object"),
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
        RegisterValue::Concatenation(values) => {
            for value in values {
                append_js_string(value, functions, output)?;
            }
        }
        _ => append_console_string(value, functions, output)?,
    }
    Ok(())
}

fn child_function(
    unit: &OwnedVisitorUnit,
    parent: FunctionId,
    opcode: &str,
    index: u64,
) -> Result<FunctionId, LlvmError> {
    let relation_matches = |relation: &FunctionRelation| match (opcode, relation) {
        ("op_new_func", FunctionRelation::Declaration { index: actual }) => {
            u64::from(*actual) == index
        }
        ("op_new_func_exp", FunctionRelation::Expression { index: actual }) => {
            u64::from(*actual) == index
        }
        _ => false,
    };
    unit.functions
        .iter()
        .find(|function| function.parent == Some(parent) && relation_matches(&function.relation))
        .map(|function| function.id)
        .ok_or_else(|| imported_error(format!("f{} references missing child {index}", parent.0)))
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

fn static_get_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<RegisterValue, LlvmError> {
    let (id, is_array) = match base {
        RegisterValue::Object(id) => (*id, false),
        RegisterValue::Array(id) => (*id, true),
        _ => return Err(imported_error("property read base is not a static object")),
    };
    let entry = heap
        .get(&id)
        .ok_or_else(|| imported_error("property read references a missing static heap entry"))?;
    match (entry, key) {
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Name(name)) => properties
            .get(&name)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| missing_static_property(&name, is_array)),
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Index(index)) => {
            let name = index.to_string();
            properties
                .get(name.as_str())
                .cloned()
                .map(Ok)
                .unwrap_or(Ok(RegisterValue::Undefined))
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => Ok(elements
            .get(&index)
            .cloned()
            .unwrap_or(RegisterValue::Undefined)),
        (
            StaticHeapEntry::Array {
                properties, length, ..
            },
            StaticPropertyKey::Name(name),
        ) if name.as_ref() == "length" => Ok(RegisterValue::Scalar(ScalarExpression::Integer(
            i64::from(*length),
        ))),
        (StaticHeapEntry::Array { properties, .. }, StaticPropertyKey::Name(name)) => properties
            .get(&name)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| missing_static_property(&name, is_array)),
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
        _ => return Err(imported_error("property write base is not a static object")),
    };
    let entry = heap
        .get_mut(&id)
        .ok_or_else(|| imported_error("property write references a missing static heap entry"))?;
    match (entry, key) {
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Name(name)) => {
            if name.as_ref() == "__proto__" {
                return Err(imported_error("static __proto__ mutation is not admitted"));
            }
            properties.insert(name, value);
        }
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Index(index)) => {
            properties.insert(index.to_string().into_boxed_str(), value);
        }
        (
            StaticHeapEntry::Array {
                elements, length, ..
            },
            StaticPropertyKey::Index(index),
        ) => {
            elements.insert(index, value);
            *length = (*length).max(index.saturating_add(1));
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Err(imported_error(
                "static array length mutation is not admitted",
            ));
        }
        (StaticHeapEntry::Array { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.insert(name, value);
        }
    }
    Ok(())
}

fn static_has_property(
    heap: &BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<bool, LlvmError> {
    let (id, is_array) = match base {
        RegisterValue::Object(id) => (*id, false),
        RegisterValue::Array(id) => (*id, true),
        _ => {
            return Err(imported_error(
                "property membership base is not a static object",
            ));
        }
    };
    let entry = heap.get(&id).ok_or_else(|| {
        imported_error("property membership references a missing static heap entry")
    })?;
    match (entry, key) {
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Name(name)) => {
            if properties.contains_key(&name) {
                Ok(true)
            } else {
                missing_static_property(&name, is_array).map(|_| false)
            }
        }
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Index(index)) => {
            Ok(properties.contains_key(index.to_string().as_str()))
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            Ok(elements.contains_key(&index))
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            Ok(true)
        }
        (StaticHeapEntry::Array { properties, .. }, StaticPropertyKey::Name(name)) => {
            if properties.contains_key(&name) {
                Ok(true)
            } else {
                missing_static_property(&name, is_array).map(|_| false)
            }
        }
    }
}

fn static_delete_property(
    heap: &mut BTreeMap<u32, StaticHeapEntry>,
    base: &RegisterValue,
    key: StaticPropertyKey,
) -> Result<(), LlvmError> {
    let id = match base {
        RegisterValue::Object(id) | RegisterValue::Array(id) => *id,
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
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Name(name)) => {
            properties.remove(&name);
        }
        (StaticHeapEntry::Object { properties }, StaticPropertyKey::Index(index)) => {
            properties.remove(index.to_string().as_str());
        }
        (StaticHeapEntry::Array { elements, .. }, StaticPropertyKey::Index(index)) => {
            elements.remove(&index);
        }
        (StaticHeapEntry::Array { .. }, StaticPropertyKey::Name(name))
            if name.as_ref() == "length" =>
        {
            return Err(imported_error("array length is not configurable"));
        }
        (StaticHeapEntry::Array { properties, .. }, StaticPropertyKey::Name(name)) => {
            properties.remove(&name);
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
    use hare_ir::{HARE_IR_SCHEMA_VERSION, InputKind, SourceId, SourceRecord};

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
}
