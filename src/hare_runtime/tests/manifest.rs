use hare_frontend::inventories;
use hare_ir::EffectSet;
use hare_llvm::TargetLayout;
use hare_runtime::{RuntimeManifest, Tier1Strategy, VisibleCheck, helper_symbols};

#[test]
fn every_frontend_opcode_has_visible_tier1_runtime_coverage() {
    let manifest = RuntimeManifest::build(TargetLayout::host().unwrap()).unwrap();
    let frontend_count = inventories()
        .into_iter()
        .map(|inventory| inventory.len())
        .sum::<usize>();
    assert_eq!(manifest.operations.len(), frontend_count);
    assert_eq!(
        helper_symbols(&manifest).len(),
        manifest.helpers.entries.len()
    );

    for operation in &manifest.operations {
        assert_eq!(manifest.operation(operation.opcode_id), Some(operation));
        if operation.effects.contains(EffectSet::MAY_CALL_USER) {
            assert!(
                operation
                    .checks
                    .contains(&VisibleCheck::PublishFrameAndRoots)
            );
            assert!(
                operation
                    .checks
                    .contains(&VisibleCheck::RevalidateAfterReentry)
            );
        }
        if operation.effects.contains(EffectSet::MAY_ALLOCATE) {
            assert!(
                operation
                    .checks
                    .contains(&VisibleCheck::AllocationSafepoint)
            );
        }
        if let Tier1Strategy::RuntimeHelper(capability) = operation.strategy {
            let helper = &manifest.helpers.entries[&capability];
            assert_eq!(helper.effects, operation.effects);
            assert!(!helper.semantic_operation.contains("execute_opcode"));
        }
    }
}
