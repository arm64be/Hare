use core::ffi::c_void;

use bun_core::String as BunString;
use bun_options_types::Format;
use hare_ir::{
    ConstantSourceRepresentation, FunctionId, FunctionRelation, FunctionSpecialization,
    HareImportError, ImportBuilder, ImportError, InputKind, OperandRole, OperandValue,
    OwnedVisitorUnit, ParserDiagnostic, SourceId, SourceRecord, SourceText, VisitorConstant,
    VisitorConstantValue,
};

const NO_PARENT: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy)]
struct CxxImportResult {
    status: u32,
    detail: u32,
}

unsafe extern "C" {
    fn Bun__Hare__importModuleFromSource(
        source_provider_url: *mut BunString,
        input_code: *const u8,
        input_source_code_size: usize,
        visitor_context: *mut c_void,
        result: *mut CxxImportResult,
    );

    fn Bun__Hare__importProgramFromSource(
        source_provider_url: *mut BunString,
        input_code: *const u8,
        input_source_code_size: usize,
        visitor_context: *mut c_void,
        result: *mut CxxImportResult,
    );
}

struct BridgeContext {
    builder: Option<ImportBuilder>,
    visitor_error: Option<ImportError>,
    parser_diagnostic: Option<ParserDiagnostic>,
}

impl BridgeContext {
    fn reject(&mut self, message: impl Into<Box<str>>) -> u32 {
        if self.visitor_error.is_none() {
            self.visitor_error = Some(ImportError::VisitorRejected(message.into()));
        }
        0
    }
}

fn callback_boundary(
    context: *mut c_void,
    callback: impl FnOnce(&mut BridgeContext) -> Result<(), ImportError>,
) -> u32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: only `import_structural_for_hare` creates this pointer and C++
    // invokes callbacks synchronously before the stack-owned context is moved.
    let context = unsafe { &mut *context.cast::<BridgeContext>() };
    if context.visitor_error.is_some() {
        return 0;
    }

    #[cfg(panic = "unwind")]
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(context)))
        .unwrap_or_else(|_| Err(ImportError::VisitorRejected("Rust visitor panic".into())));
    #[cfg(not(panic = "unwind"))]
    let result = callback(context);

    match result {
        Ok(()) => 1,
        Err(error) => {
            context.visitor_error = Some(error);
            0
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorBeginFunction(
    context: *mut c_void,
    function_id: u32,
    parent_id: u32,
    relation_kind: u32,
    relation_index: u32,
    specialization: u32,
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
    call_frame_this_argument_register: i32,
    call_frame_first_argument_register: i32,
    instruction_bytes: u32,
) -> u32 {
    callback_boundary(context, |context| {
        let relation = match relation_kind {
            0 => FunctionRelation::Root,
            1 => FunctionRelation::Declaration {
                index: relation_index,
            },
            2 => FunctionRelation::Expression {
                index: relation_index,
            },
            3 => FunctionRelation::FunctionConstructorSpecialization {
                order: relation_index,
            },
            _ => {
                return Err(ImportError::VisitorRejected(
                    "invalid function relation".into(),
                ));
            }
        };
        let specialization = match specialization {
            0 => FunctionSpecialization::Program,
            1 => FunctionSpecialization::Module,
            2 => FunctionSpecialization::Eval,
            3 => FunctionSpecialization::Call,
            4 => FunctionSpecialization::Construct,
            5 => FunctionSpecialization::FunctionConstructorCall,
            6 => FunctionSpecialization::FunctionConstructorConstruct,
            _ => {
                return Err(ImportError::VisitorRejected(
                    "invalid function specialization".into(),
                ));
            }
        };
        let Some(builder) = context.builder.as_mut() else {
            return Err(ImportError::VisitorRejected(
                "missing import builder".into(),
            ));
        };
        builder.begin_function(
            FunctionId(function_id),
            (parent_id != NO_PARENT).then_some(FunctionId(parent_id)),
            relation,
            specialization,
            SourceId(0),
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
            call_frame_this_argument_register,
            call_frame_first_argument_register,
            instruction_bytes,
        )
    })
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorInstruction(
    context: *mut c_void,
    byte_offset: u32,
    opcode_id: u32,
    encoded_size: u32,
    opcode_id_bytes: u32,
    width_bytes: u32,
) -> u32 {
    callback_boundary(context, |context| {
        let Some(builder) = context.builder.as_mut() else {
            return Err(ImportError::VisitorRejected(
                "missing import builder".into(),
            ));
        };
        builder.instruction(
            byte_offset,
            opcode_id,
            encoded_size,
            opcode_id_bytes,
            width_bytes,
        )
    })
}

fn constant_source_representation(value: u32) -> Result<ConstantSourceRepresentation, ImportError> {
    match value {
        0 => Ok(ConstantSourceRepresentation::Other),
        1 => Ok(ConstantSourceRepresentation::Integer),
        2 => Ok(ConstantSourceRepresentation::Double),
        3 => Ok(ConstantSourceRepresentation::LinkTimeConstant),
        _ => Err(ImportError::VisitorRejected(
            "invalid constant source representation".into(),
        )),
    }
}

unsafe fn copy_source_text(
    data: *const c_void,
    len: usize,
    is_latin1: u32,
) -> Result<SourceText, ImportError> {
    if len != 0 && data.is_null() {
        return Err(ImportError::VisitorRejected("null text span".into()));
    }
    match is_latin1 {
        1 => Ok(SourceText::Latin1(if len == 0 {
            Box::default()
        } else {
            // SAFETY: the C++ visitor provides a callback-scoped span and this
            // function copies it before returning.
            unsafe { core::slice::from_raw_parts(data.cast::<u8>(), len) }.into()
        })),
        0 => Ok(SourceText::Utf16(if len == 0 {
            Box::default()
        } else {
            // SAFETY: same contract as above, with `len` UTF-16 code units.
            unsafe { core::slice::from_raw_parts(data.cast::<u16>(), len) }.into()
        })),
        _ => Err(ImportError::VisitorRejected(
            "invalid copied-text encoding".into(),
        )),
    }
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorConstantScalar(
    context: *mut c_void,
    index: u32,
    kind: u32,
    source_representation: u32,
    payload: u64,
) -> u32 {
    callback_boundary(context, |context| {
        let source_representation = constant_source_representation(source_representation)?;
        let value = match kind {
            0 => VisitorConstantValue::Empty,
            1 => VisitorConstantValue::Undefined,
            2 => VisitorConstantValue::Null,
            3 => VisitorConstantValue::Boolean(payload != 0),
            4 => VisitorConstantValue::Int32(payload as u32 as i32),
            5 => VisitorConstantValue::Float64Bits(payload),
            _ => {
                return Err(ImportError::VisitorRejected(
                    "invalid scalar constant kind".into(),
                ));
            }
        };
        context
            .builder
            .as_mut()
            .ok_or_else(|| ImportError::VisitorRejected("missing import builder".into()))?
            .constant(
                index,
                VisitorConstant {
                    value,
                    source_representation,
                },
            )
    })
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorConstantText(
    context: *mut c_void,
    index: u32,
    kind: u32,
    source_representation: u32,
    data: *const c_void,
    len: usize,
    is_latin1: u32,
) -> u32 {
    callback_boundary(context, |context| {
        let source_representation = constant_source_representation(source_representation)?;
        // SAFETY: C++ keeps the source span alive for this synchronous callback.
        let text = unsafe { copy_source_text(data, len, is_latin1)? };
        let value = match kind {
            6 => VisitorConstantValue::String(text),
            7 => {
                let SourceText::Latin1(bytes) = text else {
                    return Err(ImportError::VisitorRejected(
                        "unimplemented-cell name must be Latin-1".into(),
                    ));
                };
                VisitorConstantValue::UnimplementedCell(
                    core::str::from_utf8(&bytes)
                        .map_err(|_| {
                            ImportError::VisitorRejected(
                                "unimplemented-cell name is not ASCII".into(),
                            )
                        })?
                        .into(),
                )
            }
            8 => {
                let SourceText::Latin1(bytes) = text else {
                    return Err(ImportError::VisitorRejected(
                        "link-time constant name must be Latin-1".into(),
                    ));
                };
                VisitorConstantValue::LinkTimeConstant(
                    core::str::from_utf8(&bytes)
                        .map_err(|_| {
                            ImportError::VisitorRejected(
                                "link-time constant name is not ASCII".into(),
                            )
                        })?
                        .into(),
                )
            }
            _ => {
                return Err(ImportError::VisitorRejected(
                    "invalid text constant kind".into(),
                ));
            }
        };
        context
            .builder
            .as_mut()
            .ok_or_else(|| ImportError::VisitorRejected("missing import builder".into()))?
            .constant(
                index,
                VisitorConstant {
                    value,
                    source_representation,
                },
            )
    })
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorIdentifier(
    context: *mut c_void,
    index: u32,
    data: *const c_void,
    len: usize,
    is_latin1: u32,
) -> u32 {
    callback_boundary(context, |context| {
        // SAFETY: C++ keeps the identifier span alive for this synchronous callback.
        let text = unsafe { copy_source_text(data, len, is_latin1)? };
        context
            .builder
            .as_mut()
            .ok_or_else(|| ImportError::VisitorRejected("missing import builder".into()))?
            .identifier(index, text)
    })
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorOperand(
    context: *mut c_void,
    manifest_id_data: *const u8,
    manifest_id_len: usize,
    role: u32,
    value_kind: u32,
    signed_value: i64,
    unsigned_value: u64,
) -> u32 {
    if context.is_null() || manifest_id_data.is_null() || manifest_id_len == 0 {
        return 0;
    }
    callback_boundary(context, |context| {
        // SAFETY: generated C++ passes a static ASCII manifest-ID span and the
        // callback copies it before returning.
        let manifest_id = unsafe { core::slice::from_raw_parts(manifest_id_data, manifest_id_len) };
        let manifest_id = core::str::from_utf8(manifest_id)
            .map_err(|_| ImportError::VisitorRejected("invalid operand manifest ID".into()))?
            .into();
        let role = match role {
            0 => OperandRole::ValueUse,
            1 => OperandRole::ValueDefinition,
            2 => OperandRole::ValueUseDefinition,
            3 => OperandRole::RegisterRangeUse,
            4 => OperandRole::ConstantOrRegister,
            5 => OperandRole::ControlTarget,
            6 => OperandRole::ArgumentCount,
            7 => OperandRole::ArgumentIndex,
            8 => OperandRole::ArgumentRangeBase,
            9 => OperandRole::IdentifierIndex,
            10 => OperandRole::FunctionIndex,
            11 => OperandRole::SwitchTableIndex,
            12 => OperandRole::BitVectorIndex,
            13 => OperandRole::FrameSlotBase,
            14 => OperandRole::ElementOrFieldIndex,
            15 => OperandRole::LexicalFeatureFlags,
            16 => OperandRole::PropertyAttributes,
            17 => OperandRole::StructureFlags,
            18 => OperandRole::ScopeDepth,
            19 => OperandRole::ScopeSlotIndex,
            20 => OperandRole::SymbolTableOrScopeDepth,
            21 => OperandRole::ResumePoint,
            22 => OperandRole::ModeOrFlags,
            23 => OperandRole::BooleanControl,
            24 => OperandRole::Count,
            _ => return Err(ImportError::VisitorRejected("invalid operand role".into())),
        };
        let value = match value_kind {
            0 => OperandValue::Signed(signed_value),
            1 => OperandValue::Unsigned(unsigned_value),
            2 => OperandValue::Boolean(unsigned_value != 0),
            _ => {
                return Err(ImportError::VisitorRejected(
                    "invalid operand value kind".into(),
                ));
            }
        };
        let Some(builder) = context.builder.as_mut() else {
            return Err(ImportError::VisitorRejected(
                "missing import builder".into(),
            ));
        };
        builder.operand(manifest_id, role, value)
    })
}

#[unsafe(no_mangle)]
extern "C" fn Bun__Hare__visitorParserError(
    context: *mut c_void,
    error_type: u32,
    syntax_error_type: u32,
    line: i32,
    column: u32,
    message_data: *const c_void,
    message_len: usize,
    message_is_latin1: u32,
) -> u32 {
    if context.is_null() || (message_len != 0 && message_data.is_null()) {
        return 0;
    }
    // SAFETY: the bridge owns `context` for this synchronous call. The message
    // span is callback-scoped and copied completely before returning.
    let context = unsafe { &mut *context.cast::<BridgeContext>() };
    let message = if message_is_latin1 == 1 {
        let bytes = if message_len == 0 {
            Box::default()
        } else {
            // SAFETY: C++ guarantees a live Latin-1 span of `message_len` bytes
            // for the duration of this callback.
            unsafe { core::slice::from_raw_parts(message_data.cast::<u8>(), message_len) }.into()
        };
        SourceText::Latin1(bytes)
    } else if message_is_latin1 == 0 {
        let code_units = if message_len == 0 {
            Box::default()
        } else {
            // SAFETY: C++ guarantees an aligned UTF-16 span of `message_len`
            // code units for the duration of this callback.
            unsafe { core::slice::from_raw_parts(message_data.cast::<u16>(), message_len) }.into()
        };
        SourceText::Utf16(code_units)
    } else {
        return context.reject("invalid parser-message encoding");
    };
    context.parser_diagnostic = Some(ParserDiagnostic {
        error_type,
        syntax_error_type,
        line,
        column,
        message,
    });
    1
}

pub fn import_structural_for_hare(
    format: Format,
    source: &[u8],
    source_provider_url: &mut BunString,
) -> Result<OwnedVisitorUnit, HareImportError> {
    if hare_llvm::Bun__Hare__nativeProbe(0) != 0x4841_5245_5f57_3101 {
        return Err(HareImportError::NativeProbeFailed);
    }
    let input_kind = match format {
        Format::Esm => InputKind::ModuleProgram,
        Format::Cjs => InputKind::ClassicProgram,
        _ => return Err(HareImportError::UnsupportedFormat),
    };
    let public_name = std::string::String::from_utf8_lossy(&source_provider_url.to_owned_slice())
        .into_owned()
        .into_boxed_str();
    let source_record = SourceRecord {
        id: SourceId(0),
        public_name,
        text: SourceText::Latin1(source.into()),
        start_line: 1,
        start_column: 0,
    };
    let mut context = BridgeContext {
        builder: Some(ImportBuilder::new(input_kind, source_record)),
        visitor_error: None,
        parser_diagnostic: None,
    };
    let context_pointer = core::ptr::from_mut(&mut context).cast::<c_void>();
    // SAFETY: all input pointers remain live for the call. C++ calls back only
    // synchronously with `context_pointer` and retains neither source nor context.
    let mut result = CxxImportResult {
        status: u32::MAX,
        detail: 0,
    };
    unsafe {
        match format {
            Format::Esm => Bun__Hare__importModuleFromSource(
                source_provider_url,
                source.as_ptr(),
                source.len(),
                context_pointer,
                &raw mut result,
            ),
            Format::Cjs => Bun__Hare__importProgramFromSource(
                source_provider_url,
                source.as_ptr(),
                source.len(),
                context_pointer,
                &raw mut result,
            ),
            _ => unreachable!(),
        }
    };
    if let Some(error) = context.visitor_error {
        return Err(HareImportError::Visitor(error));
    }
    match result.status {
        0 => {
            let Some(mut builder) = context.builder.take() else {
                return Err(HareImportError::InternalBridge(result.detail));
            };
            builder.mark_structurally_complete();
            builder.finish().map_err(HareImportError::Visitor)
        }
        1 => Err(context
            .parser_diagnostic
            .map(HareImportError::Parser)
            .unwrap_or(HareImportError::MissingParserDiagnostic)),
        2 => Err(HareImportError::NullRoot),
        3 => Err(HareImportError::InternalBridge(result.detail)),
        4 => Err(HareImportError::InternalCxxException),
        5 => Err(HareImportError::InvalidSpecializationPlan),
        6 => Err(HareImportError::NativeProbeFailed),
        7 => Err(HareImportError::PendingJscException),
        _ => Err(HareImportError::InternalBridge(result.status)),
    }
}

/// Link-time entry for the lower bundler crate. The returned unit owns every
/// source and structural record; it contains no JSC pointer or bytecode blob.
#[unsafe(no_mangle)]
fn __bun_jsc_import_hare(
    format: Format,
    source: &[u8],
    source_provider_url: &mut BunString,
) -> Result<OwnedVisitorUnit, HareImportError> {
    crate::virtual_machine::IS_BUNDLER_THREAD_FOR_BYTECODE_CACHE.set(true);
    crate::initialize(false);
    import_structural_for_hare(format, source, source_provider_url)
}
