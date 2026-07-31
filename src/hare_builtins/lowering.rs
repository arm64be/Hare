use std::collections::BTreeSet;
use std::fmt;

use hare_ir::EffectSet;

use crate::{FeatureEntry, FeatureKind, InventoryStrategy};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CapabilityCheck {
    ValidateArguments,
    PublishFrameAndRoots,
    PublishAmbientState,
    CheckException,
    RestoreAmbientState,
    RevalidateAfterReentry,
    AbruptCompletion,
    AllocationSafepoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinCapability<'a> {
    pub id: &'a str,
    pub operation: &'a str,
    pub source: &'a str,
    pub effects: EffectSet,
    pub checks: BTreeSet<CapabilityCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeatureLowering<'a> {
    GenericNative { source: &'a str, symbol: &'a str },
    RuntimeCapability(BuiltinCapability<'a>),
    NativeImplementation { source: &'a str },
    BuildTimeOnly { source: &'a str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureLoweringError {
    Strategy,
    OpaqueCapability,
    MissingVisibleCheck,
}

impl fmt::Display for FeatureLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FeatureLoweringError {}

pub fn lower_inventory_entry(
    entry: &FeatureEntry,
) -> Result<FeatureLowering<'_>, FeatureLoweringError> {
    match entry.strategy {
        InventoryStrategy::GenericNative => Ok(FeatureLowering::GenericNative {
            source: entry.source,
            symbol: entry.symbol,
        }),
        InventoryStrategy::NativeImplementation => Ok(FeatureLowering::NativeImplementation {
            source: entry.source,
        }),
        InventoryStrategy::BuildTimeOnly => Ok(FeatureLowering::BuildTimeOnly {
            source: entry.source,
        }),
        InventoryStrategy::RuntimeCapability => {
            if !matches!(
                entry.kind,
                FeatureKind::NativeModule | FeatureKind::NativeBridge
            ) {
                return Err(FeatureLoweringError::Strategy);
            }
            if entry.symbol.contains("execute_builtin") || entry.symbol.contains("execute_module") {
                return Err(FeatureLoweringError::OpaqueCapability);
            }
            let effects = EffectSet::HEAP_READ
                .union(EffectSet::HEAP_WRITE)
                .union(EffectSet::MAY_ALLOCATE)
                .union(EffectSet::MAY_THROW)
                .union(EffectSet::MAY_CALL_USER)
                .union(EffectSet::SAFEPOINT)
                .union(EffectSet::REALM_ACCESS)
                .union(EffectSet::EXCEPTION_STATE_READ);
            let checks = BTreeSet::from([
                CapabilityCheck::ValidateArguments,
                CapabilityCheck::PublishFrameAndRoots,
                CapabilityCheck::PublishAmbientState,
                CapabilityCheck::CheckException,
                CapabilityCheck::RestoreAmbientState,
                CapabilityCheck::RevalidateAfterReentry,
                CapabilityCheck::AbruptCompletion,
                CapabilityCheck::AllocationSafepoint,
            ]);
            Ok(FeatureLowering::RuntimeCapability(BuiltinCapability {
                id: entry.id,
                operation: entry.symbol,
                source: entry.source,
                effects,
                checks,
            }))
        }
    }
}

pub fn validate_feature_lowerings(entries: &[FeatureEntry]) -> Result<(), FeatureLoweringError> {
    for entry in entries {
        if let FeatureLowering::RuntimeCapability(capability) = lower_inventory_entry(entry)? {
            if capability.operation.is_empty()
                || !capability.effects.contains(EffectSet::MAY_CALL_USER)
                || !capability.effects.contains(EffectSet::SAFEPOINT)
                || !capability
                    .checks
                    .contains(&CapabilityCheck::RevalidateAfterReentry)
            {
                return Err(FeatureLoweringError::MissingVisibleCheck);
            }
        }
    }
    Ok(())
}
