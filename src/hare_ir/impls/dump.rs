use std::fmt::Write as _;

use crate::{OperandValue, OwnedVisitorUnit, SourceText, ValidationError, VisitorConstantValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DumpIdentity<'a> {
    pub target: &'a str,
    pub profile: &'a str,
}

/// Renders a deterministic diagnostic view of an owned visitor unit.
///
/// This is deliberately one-way: Hare never parses this text and never uses it
/// as an application artifact or cache input.
pub fn render_visitor_dump(
    unit: &OwnedVisitorUnit,
    identity: DumpIdentity<'_>,
) -> Result<String, ValidationError> {
    unit.validate()?;
    let mut output = String::new();
    writeln!(
        output,
        "hare-dump schema={} bun={} webkit={} target={} profile={}",
        unit.schema_version,
        unit.bun_revision,
        unit.webkit_revision,
        escaped(identity.target),
        escaped(identity.profile)
    )
    .unwrap();
    writeln!(
        output,
        "input={:?} structurally_complete={}",
        unit.input_kind, unit.structurally_complete
    )
    .unwrap();

    for source in &unit.sources {
        write!(
            output,
            "source s{} name={} line={} column={} ",
            source.id.0,
            escaped(&normalized_public_name(&source.public_name)),
            source.start_line,
            source.start_column
        )
        .unwrap();
        match &source.text {
            SourceText::Latin1(bytes) => {
                write!(output, "latin1=").unwrap();
                for byte in bytes {
                    write!(output, "{byte:02x}").unwrap();
                }
            }
            SourceText::Utf16(code_units) => {
                write!(output, "utf16=").unwrap();
                for code_unit in code_units {
                    write!(output, "{code_unit:04x}").unwrap();
                }
            }
        }
        output.push('\n');
    }

    for function in &unit.functions {
        writeln!(
            output,
            "function f{} parent={} relation={:?} specialization={:?} source=s{} parse={} script={} code={} lexical=0x{:08x} features=0x{:08x} params={} vars={} locals={} this={} scope={} call_this={} call_arg0={} bytes={}",
            function.id.0,
            function
                .parent
                .map_or_else(|| "-".into(), |parent| format!("f{}", parent.0)),
            function.relation,
            function.specialization,
            function.source.0,
            function.parse_mode,
            function.script_mode,
            function.code_type,
            function.lexical_features,
            function.code_features,
            function.num_parameters,
            function.num_vars,
            function.num_callee_locals,
            function.this_register,
            function.scope_register,
            function.call_frame_this_argument_register,
            function.call_frame_first_argument_register,
            function.instruction_bytes
        )
        .unwrap();
        for (index, constant) in function.constants.iter().enumerate() {
            writeln!(
                output,
                "  constant k{index} source={:?} value={}",
                constant.source_representation,
                render_constant_value(&constant.value)
            )
            .unwrap();
        }
        for (index, identifier) in function.identifiers.iter().enumerate() {
            writeln!(
                output,
                "  identifier id{index} value={}",
                render_source_text(identifier)
            )
            .unwrap();
        }
        for (index, table) in function.simple_switch_tables.iter().enumerate() {
            write!(
                output,
                "  simple-switch t{index} minimum={} default={} list={} offsets=",
                table.minimum, table.default_offset, table.is_list
            )
            .unwrap();
            for (offset_index, offset) in table.branch_offsets.iter().enumerate() {
                if offset_index != 0 {
                    output.push(',');
                }
                write!(output, "{offset}").unwrap();
            }
            output.push('\n');
        }
        for (index, table) in function.string_switch_tables.iter().enumerate() {
            writeln!(
                output,
                "  string-switch t{index} minimum_length={} maximum_length={} default={} entries={}",
                table.minimum_length,
                table.maximum_length,
                table.default_offset,
                table.declared_entry_count
            )
            .unwrap();
            for entry in &table.entries {
                writeln!(
                    output,
                    "    string-switch-entry key={} offset={} index={}",
                    render_source_text(&entry.key),
                    entry.branch_offset,
                    entry.index_in_table
                )
                .unwrap();
            }
        }
        for (index, handler) in function.exception_handlers.iter().enumerate() {
            writeln!(
                output,
                "  exception-handler h{index} start={} end={} target={} kind={:?}",
                handler.start, handler.end, handler.target, handler.kind
            )
            .unwrap();
        }
        for instruction in &function.instructions {
            writeln!(
                output,
                "  instruction offset={} opcode={} size={} opcode_width={} operand_width={}",
                instruction.byte_offset,
                instruction.opcode_id,
                instruction.encoded_size,
                instruction.opcode_id_bytes,
                instruction.width_bytes
            )
            .unwrap();
            for operand in &instruction.operands {
                writeln!(
                    output,
                    "    operand id={} role={:?} value={}",
                    escaped(&operand.manifest_id),
                    operand.role,
                    render_operand_value(&operand.value)
                )
                .unwrap();
            }
        }
    }

    for (definition, state) in &unit.definition_coverage {
        writeln!(
            output,
            "coverage id={} state={state:?}",
            escaped(definition)
        )
        .unwrap();
    }
    Ok(output)
}

fn render_constant_value(value: &VisitorConstantValue) -> String {
    match value {
        VisitorConstantValue::Empty => "empty".into(),
        VisitorConstantValue::Undefined => "undefined".into(),
        VisitorConstantValue::Null => "null".into(),
        VisitorConstantValue::Boolean(value) => format!("boolean:{value}"),
        VisitorConstantValue::Int32(value) => format!("int32:{value}"),
        VisitorConstantValue::Float64Bits(value) => format!("float64-bits:{value:016x}"),
        VisitorConstantValue::String(value) => format!("string:{}", render_source_text(value)),
        VisitorConstantValue::RegExp { pattern, flags } => {
            format!("regexp:{} flags=0x{flags:08x}", render_source_text(pattern))
        }
        VisitorConstantValue::ImmutableArray {
            elements,
            indexing_type,
        } => format!(
            "immutable-array:indexing=0x{indexing_type:08x} [{}]",
            elements
                .iter()
                .map(render_constant_value)
                .collect::<Vec<_>>()
                .join(",")
        ),
        VisitorConstantValue::LinkTimeConstant(name) => {
            format!("link-time-constant:{}", escaped(name))
        }
        VisitorConstantValue::UnimplementedCell(kind) => {
            format!("unimplemented-cell:{}", escaped(kind))
        }
    }
}

fn render_source_text(value: &SourceText) -> String {
    match value {
        SourceText::Latin1(bytes) => {
            let mut result = String::from("latin1:");
            for byte in bytes {
                write!(result, "{byte:02x}").unwrap();
            }
            result
        }
        SourceText::Utf16(code_units) => {
            let mut result = String::from("utf16:");
            for code_unit in code_units {
                write!(result, "{code_unit:04x}").unwrap();
            }
            result
        }
    }
}

fn normalized_public_name(name: &str) -> String {
    if name.starts_with("/$bunfs/") || (!name.starts_with('/') && !is_windows_absolute(name)) {
        return name.into();
    }
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);
    format!("<absolute>/{basename}")
}

fn is_windows_absolute(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn escaped(value: &str) -> String {
    format!("\"{}\"", value.escape_default())
}

fn render_operand_value(value: &OperandValue) -> String {
    match value {
        OperandValue::Signed(value) => format!("signed:{value}"),
        OperandValue::Unsigned(value) => format!("unsigned:{value}"),
        OperandValue::Boolean(value) => format!("boolean:{value}"),
        OperandValue::Bytes(bytes) => {
            let mut result = String::from("bytes:");
            for byte in bytes {
                write!(result, "{byte:02x}").unwrap();
            }
            result
        }
        OperandValue::Utf16(code_units) => {
            let mut result = String::from("utf16:");
            for code_unit in code_units {
                write!(result, "{code_unit:04x}").unwrap();
            }
            result
        }
        OperandValue::StableId(value) => format!("stable:{value}"),
        OperandValue::ValidatedAbsent => "validated-absent".into(),
        OperandValue::Excluded => "excluded".into(),
    }
}
