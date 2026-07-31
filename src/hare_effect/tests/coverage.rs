use std::collections::BTreeSet;

use hare_effect::{
    ALL_EFFECT_DOMAINS, CALLBACK_CONTRACTS, CallbackContainment, DOMAIN_CONTRACTS, EffectDomain,
    InterruptedContainment, ModuleStrategy, SelectedEffectInventory, lower_effect_domain,
    lower_reachable_module, validate_effect_lowering,
};

#[test]
fn selected_pin_and_every_runtime_domain_have_tier1_lowering() {
    validate_effect_lowering().unwrap();
    let inventory = SelectedEffectInventory::load().unwrap();
    assert_eq!(inventory.exports.len(), 179);
    assert_eq!(inventory.artifact_files.len(), 2_715);
    assert_eq!(inventory.source_files.len(), 362);

    let covered = DOMAIN_CONTRACTS
        .iter()
        .map(|contract| contract.domain)
        .collect::<BTreeSet<_>>();
    assert_eq!(covered.len(), ALL_EFFECT_DOMAINS.len());
    assert!(
        DOMAIN_CONTRACTS
            .iter()
            .all(|contract| contract.generic_native_fallback)
    );
    assert!(
        DOMAIN_CONTRACTS
            .iter()
            .find(|contract| contract.domain == EffectDomain::Micro)
            .unwrap()
            .distinct_machine
    );
    for domain in ALL_EFFECT_DOMAINS {
        let instructions = lower_effect_domain(*domain).unwrap();
        assert!(
            instructions
                .iter()
                .all(|instruction| instruction.domain == *domain)
        );
        assert_eq!(instructions.first().unwrap().ordinal, 0);
    }
}

#[test]
fn every_selected_executable_artifact_has_generic_native_semantics() {
    let inventory = SelectedEffectInventory::load().unwrap();
    for path in &inventory.artifact_files {
        let lowering = lower_reachable_module(&inventory, path, false).unwrap();
        if (path.ends_with(".ts") && !path.ends_with(".d.ts")) || path.ends_with(".js") {
            assert_eq!(lowering.strategy, ModuleStrategy::GenericNative, "{path}");
        }
    }
    assert!(lower_reachable_module(&inventory, "src/not-in-the-pin.ts", false).is_err());
    assert!(lower_reachable_module(&inventory, "src/Effect.ts", true).is_err());
}

#[test]
fn callback_throws_keep_selected_asymmetric_containment() {
    let containment = |name| {
        CALLBACK_CONTRACTS
            .iter()
            .find(|contract| contract.boundary == name)
            .unwrap()
            .ordinary_throw
    };
    assert_eq!(
        containment("supervisor.onEffect"),
        CallbackContainment::Escapes
    );
    assert_eq!(
        containment("tracerAndPrimitiveDispatch"),
        CallbackContainment::RunLoopDie
    );
    assert_eq!(
        containment("asyncRegister"),
        CallbackContainment::InitiateAsyncDefect
    );
    assert_eq!(containment("asyncResumeTell"), CallbackContainment::Escapes);
    assert_eq!(
        CALLBACK_CONTRACTS
            .iter()
            .find(|contract| contract.boundary == "customEffectable.commit")
            .unwrap()
            .interrupted_throw,
        InterruptedContainment::SequentialDefectAndInterrupt
    );
}
