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
    Negate(Box<Self>),
    Call {
        function: FunctionId,
        arguments: Vec<Self>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScalarFunction {
    id: FunctionId,
    parameter_count: usize,
    result: ScalarExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RegisterValue {
    Scalar(ScalarExpression),
    Function(FunctionId),
    ConsoleScope,
    ConsoleObject,
    ConsoleLog,
    Undefined,
    Opaque,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoweredBody {
    result: Option<ScalarExpression>,
    writes: Vec<ScalarExpression>,
}

/// Compile the first executable Tier 1 slice directly from the owned JSC
/// visitor unit. This slice is deliberately fail-closed: it accepts a closed
/// integer call graph and `console.log(integer)` only after evaluating every
/// reachable call with its closed arguments inside the exact safe-integer
/// domain. Unsupported visitor operations never fall through to runtime JSC.
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
        let body = lower_function(unit, function)?;
        if !body.writes.is_empty() {
            return Err(imported_error(format!(
                "f{} performs output inside a callable function",
                function.id.0
            )));
        }
        let result = body.result.ok_or_else(|| {
            imported_error(format!("f{} has no scalar return value", function.id.0))
        })?;
        functions.insert(
            function.id,
            ScalarFunction {
                id: function.id,
                parameter_count: function.num_parameters.saturating_sub(1) as usize,
                result,
            },
        );
    }

    let root_body = lower_function(unit, root)?;
    if root_body.writes.is_empty() {
        return Err(imported_error(
            "root function performs no admitted native output",
        ));
    }
    for expression in &root_body.writes {
        evaluate(expression, &functions, &[], 0)?;
    }
    emit_application(target, &functions, &root_body.writes)
}

fn lower_function(
    unit: &OwnedVisitorUnit,
    function: &VisitorFunction,
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
    for instruction in &function.instructions {
        if hare_frontend::cache_descriptor(instruction.opcode_id).is_some() {
            continue;
        }
        let descriptor = hare_frontend::descriptor(instruction.opcode_id)
            .ok_or_else(|| imported_error(format!("unknown opcode {}", instruction.opcode_id)))?;
        match descriptor.opcode {
            "op_enter" => {}
            "op_mov" => {
                let destination = signed_operand(instruction, "dst")?;
                let source = signed_operand(instruction, "src")?;
                let value = read_register(function, &registers, source)?;
                registers.insert(destination, value);
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
                } else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
            }
            "op_add" | "op_sub" | "op_mul" | "op_div" | "op_mod" => {
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
                    _ => unreachable!(),
                };
                registers.insert(destination, RegisterValue::Scalar(expression));
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
            "op_call" => {
                let destination = signed_operand(instruction, "dst")?;
                let callee_register = signed_operand(instruction, "callee")?;
                let RegisterValue::Function(callee) =
                    read_register(function, &registers, callee_register)?
                else {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                };
                let arguments = call_arguments(function, instruction, &registers)?;
                registers.insert(
                    destination,
                    RegisterValue::Scalar(ScalarExpression::Call {
                        function: callee,
                        arguments,
                    }),
                );
            }
            "op_call_ignore_result" => {
                let callee = signed_operand(instruction, "callee")?;
                if !matches!(
                    read_register(function, &registers, callee)?,
                    RegisterValue::ConsoleLog
                ) {
                    return Err(unsupported(function, instruction, descriptor.opcode));
                }
                let mut arguments = call_arguments(function, instruction, &registers)?;
                if arguments.len() != 1 {
                    return Err(imported_error(
                        "imported scalar console.log requires exactly one argument",
                    ));
                }
                writes.push(arguments.remove(0));
            }
            "op_ret" => {
                let value = signed_operand(instruction, "value")?;
                match read_register(function, &registers, value)? {
                    RegisterValue::Scalar(expression) => result = Some(expression),
                    RegisterValue::Undefined | RegisterValue::Opaque if function.id.0 == 0 => {}
                    _ => return Err(unsupported(function, instruction, descriptor.opcode)),
                }
            }
            _ => return Err(unsupported(function, instruction, descriptor.opcode)),
        }
    }
    Ok(LoweredBody { result, writes })
}

fn call_arguments(
    function: &VisitorFunction,
    instruction: &VisitorInstruction,
    registers: &BTreeMap<i64, RegisterValue>,
) -> Result<Vec<ScalarExpression>, LlvmError> {
    let count = usize::try_from(unsigned_operand(instruction, "argc")?)
        .map_err(|_| imported_error("call argument count does not fit usize"))?;
    if count == 0 {
        return Err(imported_error("JSC call has no this argument"));
    }
    let argv = i64::try_from(unsigned_operand(instruction, "argv")?)
        .map_err(|_| imported_error("call argv does not fit i64"))?;
    let this_register = -argv + i64::from(function.call_frame_this_argument_register);
    (1..count)
        .map(|index| scalar_register(function, registers, this_register + index as i64))
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
                if !value.is_finite()
                    || value.fract() != 0.0
                    || value.abs() > MAX_SAFE_INTEGER as f64
                    || (value == 0.0 && value.is_sign_negative())
                {
                    return Err(imported_error(format!(
                        "f{} uses a non-integral scalar constant",
                        function.id.0
                    )));
                }
                RegisterValue::Scalar(ScalarExpression::Integer(value as i64))
            }
            VisitorConstantValue::Undefined => RegisterValue::Undefined,
            VisitorConstantValue::Empty | VisitorConstantValue::UnimplementedCell(_) => {
                RegisterValue::Opaque
            }
            _ => {
                return Err(imported_error(format!(
                    "f{} uses a non-scalar constant",
                    function.id.0
                )));
            }
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
        ScalarExpression::Negate(value) => {
            let value = evaluate(value, functions, arguments, depth)?;
            if value == 0 {
                return Err(imported_error("scalar negation produces negative zero"));
            }
            value
                .checked_neg()
                .ok_or_else(|| imported_error("integer negation exceeds i64"))?
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

fn emit_application(
    target: TargetLayout,
    functions: &BTreeMap<FunctionId, ScalarFunction>,
    writes: &[ScalarExpression],
) -> Result<String, LlvmError> {
    let mut output = String::new();
    output.push_str("; Hare imported scalar application\n");
    output.push_str("source_filename = \"hare-imported-application\"\n");
    let _ = writeln!(output, "target datalayout = \"{}\"", target.data_layout);
    let _ = writeln!(output, "target triple = \"{}\"\n", target.triple);
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
    for expression in writes {
        let value = emitter.emit(expression)?;
        let written = emitter.temporary();
        let _ = writeln!(
            emitter.output,
            "  {written} = call i64 @Bun__Hare__writeInt64Line(i64 {value})"
        );
        let write_ok = emitter.temporary();
        let _ = writeln!(emitter.output, "  {write_ok} = icmp sge i64 {written}, 0");
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
        }
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
            result: ScalarExpression::Add(
                Box::new(ScalarExpression::Parameter(0)),
                Box::new(ScalarExpression::Integer(1)),
            ),
        };
        let functions = BTreeMap::from([(function.id, function)]);
        let writes = [ScalarExpression::Call {
            function: FunctionId(1),
            arguments: vec![ScalarExpression::Integer(41)],
        }];
        assert_eq!(evaluate(&writes[0], &functions, &[], 0).unwrap(), 42);
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
}
