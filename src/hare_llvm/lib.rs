//! LLVM text emission and the versioned Hare runtime-helper ABI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use hare_ir::{EffectSet, RuntimeCapabilityId};

pub const HARE_HELPER_MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetOperatingSystem {
    Linux,
    MacOs,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetArchitecture {
    X86_64,
    Aarch64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetLayout {
    pub operating_system: TargetOperatingSystem,
    pub architecture: TargetArchitecture,
    pub triple: &'static str,
    pub data_layout: &'static str,
    pub pointer_bits: u16,
    pub little_endian: bool,
}

impl TargetLayout {
    pub fn for_target(
        operating_system: TargetOperatingSystem,
        architecture: TargetArchitecture,
    ) -> Self {
        match (operating_system, architecture) {
            (TargetOperatingSystem::Linux, TargetArchitecture::X86_64) => Self {
                operating_system,
                architecture,
                triple: "x86_64-unknown-linux-gnu",
                data_layout: "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
                pointer_bits: 64,
                little_endian: true,
            },
            (TargetOperatingSystem::Linux, TargetArchitecture::Aarch64) => Self {
                operating_system,
                architecture,
                triple: "aarch64-unknown-linux-gnu",
                data_layout: "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32",
                pointer_bits: 64,
                little_endian: true,
            },
            (TargetOperatingSystem::MacOs, TargetArchitecture::X86_64) => Self {
                operating_system,
                architecture,
                triple: "x86_64-apple-macosx10.13.0",
                data_layout: "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
                pointer_bits: 64,
                little_endian: true,
            },
            (TargetOperatingSystem::MacOs, TargetArchitecture::Aarch64) => Self {
                operating_system,
                architecture,
                triple: "arm64-apple-macosx11.0.0",
                data_layout: "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32",
                pointer_bits: 64,
                little_endian: true,
            },
            (TargetOperatingSystem::Windows, TargetArchitecture::X86_64) => Self {
                operating_system,
                architecture,
                triple: "x86_64-pc-windows-msvc",
                data_layout: "e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
                pointer_bits: 64,
                little_endian: true,
            },
            (TargetOperatingSystem::Windows, TargetArchitecture::Aarch64) => Self {
                operating_system,
                architecture,
                triple: "aarch64-pc-windows-msvc",
                data_layout: "e-m:w-p270:32:32-p271:32:32-p272:64:64-p:64:64-i32:32-i64:64-i128:128-n32:64-S128-Fn32",
                pointer_bits: 64,
                little_endian: true,
            },
        }
    }

    pub fn host() -> Result<Self, LlvmError> {
        let os = if cfg!(target_os = "linux") {
            TargetOperatingSystem::Linux
        } else if cfg!(target_os = "macos") {
            TargetOperatingSystem::MacOs
        } else if cfg!(target_os = "windows") {
            TargetOperatingSystem::Windows
        } else {
            return Err(LlvmError::UnsupportedHost);
        };
        let arch = if cfg!(target_arch = "x86_64") {
            TargetArchitecture::X86_64
        } else if cfg!(target_arch = "aarch64") {
            TargetArchitecture::Aarch64
        } else {
            return Err(LlvmError::UnsupportedHost);
        };
        Ok(Self::for_target(os, arch))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallingConvention {
    C,
    Fast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiType {
    Void,
    I1,
    I8,
    I16,
    I32,
    I64,
    F64,
    Pointer,
    TaggedValue,
    RealmHandle,
    RuntimeServiceHandle,
    TaggedCompletion,
}

impl AbiType {
    const fn llvm(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::I1 => "i1",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F64 => "double",
            Self::Pointer
            | Self::TaggedValue
            | Self::RealmHandle
            | Self::RuntimeServiceHandle
            | Self::TaggedCompletion => "ptr",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiOwnership {
    Borrowed,
    Consumed,
    NewlyOwned,
    GcTraced,
    Pinned,
    Copied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelperParameter {
    pub name: Box<str>,
    pub abi_type: AbiType,
    pub ownership: AbiOwnership,
    pub retained_after_return: bool,
    pub retention_owner: Option<Box<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelperEntry {
    pub capability: RuntimeCapabilityId,
    pub semantic_operation: Box<str>,
    pub symbol: Box<str>,
    pub calling_convention: CallingConvention,
    pub parameters: Vec<HelperParameter>,
    pub result: AbiType,
    pub normal_result: bool,
    pub abrupt_results: BTreeSet<Box<str>>,
    pub effects: EffectSet,
    pub safepoint: bool,
    pub reads_ambient_state: BTreeSet<Box<str>>,
    pub writes_ambient_state: BTreeSet<Box<str>>,
    pub optimizer_attributes: BTreeSet<Box<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelperManifest {
    pub version: u32,
    pub target: TargetLayout,
    pub entries: BTreeMap<RuntimeCapabilityId, HelperEntry>,
}

impl HelperManifest {
    pub fn new(target: TargetLayout) -> Self {
        Self {
            version: HARE_HELPER_MANIFEST_VERSION,
            target,
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, entry: HelperEntry) -> Result<(), LlvmError> {
        validate_symbol(&entry.symbol)?;
        if entry.safepoint != entry.effects.contains(EffectSet::SAFEPOINT) {
            return Err(LlvmError::SafepointEffectMismatch(entry.symbol));
        }
        if entry.effects.contains(EffectSet::MAY_THROW) && entry.abrupt_results.is_empty() {
            return Err(LlvmError::MissingAbruptResult(entry.symbol));
        }
        for parameter in &entry.parameters {
            if parameter.retained_after_return != parameter.retention_owner.is_some() {
                return Err(LlvmError::RetentionOwnerMismatch {
                    symbol: entry.symbol.clone(),
                    parameter: parameter.name.clone(),
                });
            }
        }
        if self.entries.insert(entry.capability, entry).is_some() {
            return Err(LlvmError::DuplicateCapability);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFunction {
    pub symbol: Box<str>,
    pub parameters: Vec<AbiType>,
    pub result: AbiType,
    pub body: NativeBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeBody {
    ReturnI64(u64),
    AddI64,
    XorI64(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeModule {
    pub name: Box<str>,
    pub target: TargetLayout,
    pub functions: Vec<NativeFunction>,
}

impl NativeModule {
    pub fn emit_llvm_ir(&self) -> Result<String, LlvmError> {
        validate_module_name(&self.name)?;
        let mut seen = BTreeSet::new();
        let mut output = String::new();
        output.push_str("; Hare native module\n");
        output.push_str(&format!("source_filename = \"{}\"\n", self.name));
        output.push_str(&format!(
            "target datalayout = \"{}\"\n",
            self.target.data_layout
        ));
        output.push_str(&format!("target triple = \"{}\"\n\n", self.target.triple));

        for function in &self.functions {
            validate_symbol(&function.symbol)?;
            if !seen.insert(function.symbol.as_ref()) {
                return Err(LlvmError::DuplicateSymbol(function.symbol.clone()));
            }
            emit_function(&mut output, function)?;
        }
        Ok(output)
    }
}

fn emit_function(output: &mut String, function: &NativeFunction) -> Result<(), LlvmError> {
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, abi)| format!("{} %arg{index}", abi.llvm()))
        .collect::<Vec<_>>()
        .join(", ");
    output.push_str(&format!(
        "define {} @{}({parameters}) nounwind {{\nentry:\n",
        function.result.llvm(),
        function.symbol
    ));
    match function.body {
        NativeBody::ReturnI64(value) => {
            if function.result != AbiType::I64 || !function.parameters.is_empty() {
                return Err(LlvmError::BodySignature(function.symbol.clone()));
            }
            output.push_str(&format!("  ret i64 {value}\n"));
        }
        NativeBody::AddI64 => {
            if function.result != AbiType::I64
                || function.parameters.as_slice() != [AbiType::I64, AbiType::I64]
            {
                return Err(LlvmError::BodySignature(function.symbol.clone()));
            }
            output.push_str("  %sum = add i64 %arg0, %arg1\n  ret i64 %sum\n");
        }
        NativeBody::XorI64(value) => {
            if function.result != AbiType::I64 || function.parameters.as_slice() != [AbiType::I64] {
                return Err(LlvmError::BodySignature(function.symbol.clone()));
            }
            output.push_str(&format!(
                "  %value = xor i64 %arg0, {value}\n  ret i64 %value\n"
            ));
        }
    }
    output.push_str("}\n\n");
    Ok(())
}

fn validate_module_name(name: &str) -> Result<(), LlvmError> {
    if name.is_empty()
        || name
            .bytes()
            .any(|byte| byte == b'"' || byte == b'\n' || byte == b'\r')
    {
        return Err(LlvmError::InvalidModuleName);
    }
    Ok(())
}

fn validate_symbol(symbol: &str) -> Result<(), LlvmError> {
    let mut bytes = symbol.bytes();
    let Some(first) = bytes.next() else {
        return Err(LlvmError::InvalidSymbol(symbol.into()));
    };
    let first_valid = first.is_ascii_alphabetic() || matches!(first, b'_' | b'$' | b'.');
    let rest_valid =
        bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.' | b'-'));
    if !first_valid || !rest_valid {
        return Err(LlvmError::InvalidSymbol(symbol.into()));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LlvmError {
    UnsupportedHost,
    InvalidModuleName,
    InvalidSymbol(Box<str>),
    DuplicateSymbol(Box<str>),
    DuplicateCapability,
    BodySignature(Box<str>),
    SafepointEffectMismatch(Box<str>),
    MissingAbruptResult(Box<str>),
    RetentionOwnerMismatch {
        symbol: Box<str>,
        parameter: Box<str>,
    },
}

impl fmt::Display for LlvmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LlvmError {}

/// Linked native entry used by the W1 structural gate. It is a real native
/// symbol in Bun's Rust archive; the JSC import path calls it after the owned
/// visitor result has been validated.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn Bun__Hare__nativeProbe(value: u64) -> u64 {
    value ^ 0x4841_5245_5f57_3101
}
