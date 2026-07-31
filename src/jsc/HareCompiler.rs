use core::ffi::c_void;

use bun_core::String as BunString;
use bun_options_types::Format;
use hare_ir::{
    FunctionId, FunctionRelation, FunctionSpecialization, HareImportError, ImportBuilder,
    ImportError, InputKind, OwnedVisitorUnit, ParserDiagnostic, SourceId, SourceRecord, SourceText,
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
            let Some(builder) = context.builder.take() else {
                return Err(HareImportError::InternalBridge(result.detail));
            };
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
