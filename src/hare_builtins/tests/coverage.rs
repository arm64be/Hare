use std::collections::BTreeMap;

use hare_builtins::{
    ApiLowering, CapabilityCheck, CompileErrorReason, FeatureKind, FeatureLowering,
    FeatureSemantics, Provenance, RESTRICTED_OPERATIONS, feature_inventory, lower_api,
    lower_inventory_entry, validate_feature_lowerings,
};

#[test]
fn every_generated_feature_has_owned_tier1_lowering() {
    let entries = feature_inventory().unwrap();
    assert_eq!(entries.len(), 999);
    validate_feature_lowerings(&entries).unwrap();

    let mut counts = BTreeMap::new();
    for entry in &entries {
        *counts.entry(entry.kind).or_insert(0) += 1;
        match lower_inventory_entry(entry).unwrap() {
            FeatureLowering::GenericNative { .. } => assert!(matches!(
                entry.kind,
                FeatureKind::InternalModule | FeatureKind::BuiltinFunction
            )),
            FeatureLowering::RuntimeCapability(capability) => {
                assert!(!capability.operation.is_empty());
                assert!(
                    capability
                        .checks
                        .contains(&CapabilityCheck::PublishFrameAndRoots)
                );
                assert!(
                    capability
                        .checks
                        .contains(&CapabilityCheck::RevalidateAfterReentry)
                );
            }
            FeatureLowering::NativeImplementation { .. } => {
                assert_eq!(entry.kind, FeatureKind::RuntimeSource)
            }
            FeatureLowering::BuildTimeOnly { .. } => {
                assert_eq!(entry.kind, FeatureKind::BuildTimeEval)
            }
        }
    }
    assert_eq!(counts[&FeatureKind::InternalModule], 193);
    assert_eq!(counts[&FeatureKind::NativeBridge], 84);
    assert_eq!(counts[&FeatureKind::RuntimeSource], 616);
}

#[test]
fn every_source_compiler_is_rejected_even_for_literal_input() {
    for operation in RESTRICTED_OPERATIONS {
        let lowering = lower_api(operation.semantics, Provenance::StaticLiteral);
        assert!(
            matches!(lowering, ApiLowering::CompileError(_)),
            "{}",
            operation.name
        );
    }
    assert_eq!(
        lower_api(
            FeatureSemantics::RuntimeSourceCompiler,
            Provenance::StaticLiteral
        ),
        ApiLowering::CompileError(CompileErrorReason::RuntimeSourceCompilation)
    );
    assert_eq!(
        lower_api(
            FeatureSemantics::WorkerSourceEval,
            Provenance::StaticLiteral
        ),
        ApiLowering::CompileError(CompileErrorReason::WorkerSourceExecution)
    );
}

#[test]
fn graph_loaders_and_workers_require_actual_graph_membership() {
    assert_eq!(
        lower_api(
            FeatureSemantics::ClosedGraphLoader,
            Provenance::StaticGraphMember
        ),
        ApiLowering::ClosedGraphNative
    );
    assert_eq!(
        lower_api(
            FeatureSemantics::StaticWorker,
            Provenance::StaticGraphMember
        ),
        ApiLowering::StaticWorkerNative
    );
    assert_eq!(
        lower_api(FeatureSemantics::StaticWorker, Provenance::RuntimeValue),
        ApiLowering::CompileError(CompileErrorReason::GraphTargetUnknown)
    );
    assert_eq!(
        lower_api(
            FeatureSemantics::EvalOrFunctionOwnedByH017,
            Provenance::StaticLiteral
        ),
        ApiLowering::H017Frontend
    );
}
