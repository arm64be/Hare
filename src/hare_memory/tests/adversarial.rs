use std::sync::{Arc, Barrier};

use hare_analysis::Assurance;
use hare_memory::{
    AccessMode, Boundaries, EvidenceStatus, ExitSet, ManagedClass, MemoryError, MemoryRequest,
    OneShotRegistration, OwnerCarrier, OwnershipOperation, PinCarrier, RegistrationState,
    RootCarrier, TransferState, TransferToken, ViewIdentity, lifetime_inventory, required_exits,
    select_memory_plan, yield_view,
};

#[test]
fn complete_lifetime_inventory_is_machine_checked() {
    let rows = lifetime_inventory().unwrap();
    assert_eq!(rows.len(), 50);
    for required in [
        "H004-S01", "H004-A03", "H004-G01", "H004-E18", "H004-F04", "H004-C05", "H004-W01",
        "H004-T06",
    ] {
        assert!(rows.iter().any(|row| row.id == required), "{required}");
    }
    assert!(rows.iter().any(|row| row.status == EvidenceStatus::Unknown));
    for id in ["H004-S05", "H004-S09"] {
        let row = rows.iter().find(|row| row.id == id).unwrap();
        assert!(row.case.contains("non-suspending"));
        assert!(!row.exits.contains(&"async-reject"));
    }
}

#[test]
fn root_never_substitutes_for_a_pin() {
    let boundaries = Boundaries::SAFEPOINT.union(Boundaries::STORAGE_MOVEMENT);
    let mut request = MemoryRequest::managed_alias(boundaries);
    request.access = AccessMode::InteriorAddress;
    assert_eq!(
        select_memory_plan(request),
        Err(MemoryError::RawAddressCrossesMovement)
    );

    request.stable_external_owner = true;
    let plan = select_memory_plan(request).unwrap();
    assert_eq!(plan.root, RootCarrier::StrongHandle);
    assert_eq!(plan.pin, PinCarrier::StableExternal);
}

#[test]
fn hints_do_not_authorize_move_or_exclusive_ownership() {
    let mut request = MemoryRequest::managed_alias(Boundaries::default());
    request.operation = OwnershipOperation::Move;
    request.assurance = Assurance::Hint;
    request.explicit_unique_move = true;
    assert_eq!(
        select_memory_plan(request),
        Err(MemoryError::HintCannotAuthorizeOwnership)
    );
}

#[test]
fn ordinary_managed_callback_alias_may_escape_by_promotion() {
    let boundaries = Boundaries::CALLBACK_REENTRY.union(Boundaries::SAFEPOINT);
    let mut request = MemoryRequest::managed_alias(boundaries);
    request.escapes = true;
    let plan = select_memory_plan(request).unwrap();
    assert_eq!(plan.root, RootCarrier::StrongHandle);
    assert!(plan.reload_after_boundary);
}

#[test]
fn mutable_raw_borrows_do_not_cross_reentry_without_a_contract() {
    let boundaries = Boundaries::CALLBACK_REENTRY;
    let mut request = MemoryRequest::managed_alias(boundaries);
    request.operation = OwnershipOperation::MutableBorrow;
    request.assurance = Assurance::Proof;
    assert_eq!(
        select_memory_plan(request),
        Err(MemoryError::MutableBorrowCrossesBoundary)
    );
}

#[test]
fn all_boundary_exits_must_be_represented() {
    let boundaries = Boundaries::FFI.union(Boundaries::CALLBACK_REENTRY);
    let mut request = MemoryRequest::managed_alias(boundaries);
    request.exits = ExitSet::RETURN
        .union(ExitSet::THROW)
        .union(ExitSet::FINALLY);
    assert!(matches!(
        select_memory_plan(request),
        Err(MemoryError::MissingExit(_))
    ));
    assert!(required_exits(boundaries).contains(ExitSet::FFI_CANCELLATION));
}

#[test]
fn one_shot_race_has_exactly_one_payload_owner() {
    let registration = Arc::new(OneShotRegistration::new(String::from("payload")));
    let barrier = Arc::new(Barrier::new(3));
    let complete = {
        let registration = Arc::clone(&registration);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            registration.complete()
        })
    };
    let cancel = {
        let registration = Arc::clone(&registration);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            registration.cancel()
        })
    };
    barrier.wait();
    let winners = [complete.join().unwrap(), cancel.join().unwrap()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(winners, ["payload"]);
    assert!(matches!(
        registration.state(),
        RegistrationState::Completed | RegistrationState::Cancelled
    ));
    assert_eq!(registration.close(), None);
}

#[test]
fn transfer_commit_is_not_rolled_back_by_enqueue_failure() {
    let source = TransferToken::validating(vec![1_u8, 2, 3]);
    assert_eq!(source.validation_failed().unwrap(), [1, 2, 3]);

    let mut token = TransferToken::validating(vec![4_u8, 5, 6]);
    token.commit().unwrap();
    assert!(token.sender_detached());
    token.enqueue(false).unwrap();
    assert_eq!(token.state(), TransferState::Dropped);
    assert!(token.sender_detached());
    assert!(token.deliver().is_err());
}

#[test]
fn yielded_views_preserve_exact_identity_and_offsets() {
    let original = ViewIdentity {
        view: 7,
        backing_store: 11,
        byte_offset: 4,
        byte_length: 8,
    };
    assert_eq!(yield_view(original), original);
}

#[test]
fn reference_counting_requires_independent_cross_thread_lifetimes() {
    let mut request = MemoryRequest::managed_alias(Boundaries::default());
    request.managed = ManagedClass::Native;
    request.independent_lifetimes = true;
    assert_eq!(
        select_memory_plan(request),
        Err(MemoryError::IndependentReferenceCountUnjustified)
    );
}

#[test]
fn lexical_regions_cannot_silently_become_continuations() {
    let mut lexical = MemoryRequest::managed_alias(Boundaries::default());
    lexical.lexical_region = true;
    assert_eq!(
        select_memory_plan(lexical).unwrap().owner,
        OwnerCarrier::LexicalRegion
    );

    let mut suspending = MemoryRequest::managed_alias(Boundaries::SUSPENSION);
    suspending.lexical_region = true;
    assert_eq!(
        select_memory_plan(suspending),
        Err(MemoryError::RegionCrossesBoundary)
    );
}

#[test]
fn shared_cross_thread_storage_requires_synchronization() {
    let mut request = MemoryRequest::managed_alias(Boundaries::CROSS_THREAD);
    request.managed = ManagedClass::Native;
    request.shared_atomic_storage = true;
    assert_eq!(
        select_memory_plan(request),
        Err(MemoryError::SharedMutableWithoutSynchronization)
    );

    request.exclusive_or_synchronized = true;
    let plan = select_memory_plan(request).unwrap();
    assert_eq!(plan.owner, OwnerCarrier::SharedAtomicStorage);
    assert_eq!(plan.root, RootCarrier::SharedOwners);
    assert_eq!(plan.pin, PinCarrier::SharedBackingStore);
}
