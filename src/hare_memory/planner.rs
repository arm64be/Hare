use std::fmt;

use hare_analysis::Assurance;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedClass {
    ProvenImmediate,
    Managed,
    Weak,
    Native,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Tagged,
    RawAddress,
    InteriorAddress,
    WeakObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipOperation {
    Alias,
    SharedBorrow,
    MutableBorrow,
    Move,
    Copy,
    StructuredClone,
    Transfer,
    RetainedRegistration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Boundaries(u16);

impl Boundaries {
    pub const SAFEPOINT: Self = Self(1 << 0);
    pub const CALLBACK_REENTRY: Self = Self(1 << 1);
    pub const SUSPENSION: Self = Self(1 << 2);
    pub const EFFECT: Self = Self(1 << 3);
    pub const FFI: Self = Self(1 << 4);
    pub const WORKER: Self = Self(1 << 5);
    pub const CROSS_THREAD: Self = Self(1 << 6);
    pub const STORAGE_MOVEMENT: Self = Self(1 << 7);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExitSet(u16);

impl ExitSet {
    pub const RETURN: Self = Self(1 << 0);
    pub const THROW: Self = Self(1 << 1);
    pub const FINALLY: Self = Self(1 << 2);
    pub const ASYNC_REJECT: Self = Self(1 << 3);
    pub const EFFECT_FAILURE: Self = Self(1 << 4);
    pub const EFFECT_DEFECT: Self = Self(1 << 5);
    pub const INTERRUPTION: Self = Self(1 << 6);
    pub const CANCELLATION: Self = Self(1 << 7);
    pub const CALLBACK_REENTRY: Self = Self(1 << 8);
    pub const WORKER_TERMINATION: Self = Self(1 << 9);
    pub const FFI_CANCELLATION: Self = Self(1 << 10);
    pub const SAFEPOINT: Self = Self(1 << 11);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn missing_from(self, provided: Self) -> Self {
        Self(self.0 & !provided.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerCarrier {
    Immediate,
    Activation,
    ManagedHeap,
    Continuation,
    LexicalRegion,
    Registration,
    TransferToken,
    CloneAllocation,
    SharedAtomicStorage,
    IndependentReferenceCount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootCarrier {
    None,
    StackMap,
    ContinuationField,
    StrongHandle,
    Registration,
    Queue,
    TransferToken,
    SharedOwners,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinCarrier {
    None,
    StackSlot,
    StableContinuationField,
    StableExternal,
    SharedBackingStore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryPlan {
    pub owner: OwnerCarrier,
    pub root: RootCarrier,
    pub pin: PinCarrier,
    pub reload_after_boundary: bool,
    pub releases_on: ExitSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRequest {
    pub managed: ManagedClass,
    pub access: AccessMode,
    pub operation: OwnershipOperation,
    pub assurance: Assurance,
    pub boundaries: Boundaries,
    pub exits: ExitSet,
    pub escapes: bool,
    pub stable_external_owner: bool,
    pub stable_continuation_field: bool,
    pub exclusive_or_synchronized: bool,
    pub explicit_unique_move: bool,
    pub lexical_region: bool,
    pub shared_atomic_storage: bool,
    pub independent_lifetimes: bool,
    pub atomic_one_shot: bool,
    pub guard_failure_is_generic_tier1: bool,
}

impl MemoryRequest {
    pub fn managed_alias(boundaries: Boundaries) -> Self {
        Self {
            managed: ManagedClass::Managed,
            access: AccessMode::Tagged,
            operation: OwnershipOperation::Alias,
            assurance: Assurance::Unknown,
            boundaries,
            exits: required_exits(boundaries),
            escapes: false,
            stable_external_owner: false,
            stable_continuation_field: false,
            exclusive_or_synchronized: false,
            explicit_unique_move: false,
            lexical_region: false,
            shared_atomic_storage: false,
            independent_lifetimes: false,
            atomic_one_shot: false,
            guard_failure_is_generic_tier1: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    MissingExit(ExitSet),
    GuardWithoutGenericTier1,
    HintCannotAuthorizeOwnership,
    WeakIsNotRoot,
    WeakAccessWithoutObservation,
    ManagedImmediateMisclassified,
    RawBorrowEscapes,
    RawAddressCrossesMovement,
    MutableBorrowCrossesBoundary,
    MoveWithoutUniqueEvidence,
    RegistrationWithoutOneShot,
    RegionCrossesBoundary,
    SharedMutableWithoutSynchronization,
    IndependentReferenceCountUnjustified,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MemoryError {}

pub const fn required_exits(boundaries: Boundaries) -> ExitSet {
    let mut exits = ExitSet::RETURN
        .union(ExitSet::THROW)
        .union(ExitSet::FINALLY);
    if boundaries.contains(Boundaries::SAFEPOINT) {
        exits = exits.union(ExitSet::SAFEPOINT);
    }
    if boundaries.contains(Boundaries::CALLBACK_REENTRY) {
        exits = exits.union(ExitSet::CALLBACK_REENTRY);
    }
    if boundaries.contains(Boundaries::SUSPENSION) {
        exits = exits
            .union(ExitSet::ASYNC_REJECT)
            .union(ExitSet::CANCELLATION);
    }
    if boundaries.contains(Boundaries::EFFECT) {
        exits = exits
            .union(ExitSet::EFFECT_FAILURE)
            .union(ExitSet::EFFECT_DEFECT)
            .union(ExitSet::INTERRUPTION)
            .union(ExitSet::CANCELLATION);
    }
    if boundaries.contains(Boundaries::FFI) {
        exits = exits.union(ExitSet::FFI_CANCELLATION);
    }
    if boundaries.intersects(Boundaries::WORKER.union(Boundaries::CROSS_THREAD)) {
        exits = exits.union(ExitSet::WORKER_TERMINATION);
    }
    exits
}

pub fn select_memory_plan(request: MemoryRequest) -> Result<MemoryPlan, MemoryError> {
    let required = required_exits(request.boundaries);
    let missing = required.missing_from(request.exits);
    if !missing.is_empty() {
        return Err(MemoryError::MissingExit(missing));
    }
    if request.assurance == Assurance::Guard && !request.guard_failure_is_generic_tier1 {
        return Err(MemoryError::GuardWithoutGenericTier1);
    }
    let ownership_sensitive = matches!(
        request.operation,
        OwnershipOperation::MutableBorrow | OwnershipOperation::Move | OwnershipOperation::Transfer
    );
    if ownership_sensitive && request.assurance == Assurance::Hint {
        return Err(MemoryError::HintCannotAuthorizeOwnership);
    }
    if request.managed == ManagedClass::Weak {
        if request.access != AccessMode::WeakObservation {
            return Err(MemoryError::WeakAccessWithoutObservation);
        }
        return Ok(MemoryPlan {
            owner: OwnerCarrier::ManagedHeap,
            root: RootCarrier::None,
            pin: PinCarrier::None,
            reload_after_boundary: true,
            releases_on: request.exits,
        });
    }
    if request.access == AccessMode::WeakObservation {
        return Err(MemoryError::WeakIsNotRoot);
    }
    if request.managed == ManagedClass::ProvenImmediate && request.access != AccessMode::Tagged {
        return Err(MemoryError::ManagedImmediateMisclassified);
    }
    if matches!(
        request.operation,
        OwnershipOperation::SharedBorrow | OwnershipOperation::MutableBorrow
    ) && request.escapes
    {
        return Err(MemoryError::RawBorrowEscapes);
    }
    if request.operation == OwnershipOperation::MutableBorrow
        && request.boundaries.intersects(
            Boundaries::CALLBACK_REENTRY
                .union(Boundaries::SUSPENSION)
                .union(Boundaries::CROSS_THREAD),
        )
        && !request.exclusive_or_synchronized
    {
        return Err(MemoryError::MutableBorrowCrossesBoundary);
    }
    if matches!(
        request.access,
        AccessMode::RawAddress | AccessMode::InteriorAddress
    ) && request.boundaries.intersects(
        Boundaries::SAFEPOINT
            .union(Boundaries::CALLBACK_REENTRY)
            .union(Boundaries::SUSPENSION)
            .union(Boundaries::STORAGE_MOVEMENT),
    ) && !request.stable_external_owner
        && !request.stable_continuation_field
    {
        return Err(MemoryError::RawAddressCrossesMovement);
    }
    if matches!(
        request.operation,
        OwnershipOperation::Move | OwnershipOperation::Transfer
    ) && (!request.explicit_unique_move
        || !matches!(request.assurance, Assurance::Proof | Assurance::Guard))
    {
        return Err(MemoryError::MoveWithoutUniqueEvidence);
    }
    if request.operation == OwnershipOperation::RetainedRegistration && !request.atomic_one_shot {
        return Err(MemoryError::RegistrationWithoutOneShot);
    }
    if request.lexical_region
        && (request.escapes
            || request.boundaries.intersects(
                Boundaries::CALLBACK_REENTRY
                    .union(Boundaries::SUSPENSION)
                    .union(Boundaries::FFI)
                    .union(Boundaries::WORKER)
                    .union(Boundaries::CROSS_THREAD),
            ))
    {
        return Err(MemoryError::RegionCrossesBoundary);
    }
    if request.shared_atomic_storage
        && (!request.boundaries.contains(Boundaries::CROSS_THREAD)
            || !request.exclusive_or_synchronized)
    {
        return Err(MemoryError::SharedMutableWithoutSynchronization);
    }
    if request.independent_lifetimes && !request.boundaries.contains(Boundaries::CROSS_THREAD) {
        return Err(MemoryError::IndependentReferenceCountUnjustified);
    }

    let owner = match request.operation {
        OwnershipOperation::StructuredClone | OwnershipOperation::Copy => {
            OwnerCarrier::CloneAllocation
        }
        OwnershipOperation::Transfer => OwnerCarrier::TransferToken,
        OwnershipOperation::RetainedRegistration => OwnerCarrier::Registration,
        _ if request.shared_atomic_storage => OwnerCarrier::SharedAtomicStorage,
        _ if request.lexical_region => OwnerCarrier::LexicalRegion,
        _ if request.independent_lifetimes => OwnerCarrier::IndependentReferenceCount,
        _ if request.managed == ManagedClass::ProvenImmediate => OwnerCarrier::Immediate,
        _ if request.boundaries.contains(Boundaries::SUSPENSION) => OwnerCarrier::Continuation,
        _ if request.escapes
            || request.boundaries.intersects(
                Boundaries::CALLBACK_REENTRY
                    .union(Boundaries::WORKER)
                    .union(Boundaries::CROSS_THREAD),
            ) =>
        {
            OwnerCarrier::ManagedHeap
        }
        _ => OwnerCarrier::Activation,
    };
    let root = match owner {
        OwnerCarrier::Immediate => RootCarrier::None,
        OwnerCarrier::Continuation => RootCarrier::ContinuationField,
        OwnerCarrier::Registration => RootCarrier::Registration,
        OwnerCarrier::TransferToken => RootCarrier::TransferToken,
        OwnerCarrier::SharedAtomicStorage | OwnerCarrier::IndependentReferenceCount => {
            RootCarrier::SharedOwners
        }
        OwnerCarrier::CloneAllocation | OwnerCarrier::ManagedHeap
            if request.boundaries.contains(Boundaries::WORKER) =>
        {
            RootCarrier::Queue
        }
        _ if request.managed == ManagedClass::Managed
            && request.boundaries.is_empty()
            && !request.escapes =>
        {
            RootCarrier::StackMap
        }
        _ if request.managed == ManagedClass::Managed => RootCarrier::StrongHandle,
        _ => RootCarrier::None,
    };
    let pin = if request.shared_atomic_storage {
        PinCarrier::SharedBackingStore
    } else if request.stable_external_owner {
        PinCarrier::StableExternal
    } else if request.stable_continuation_field {
        PinCarrier::StableContinuationField
    } else if matches!(
        request.access,
        AccessMode::RawAddress | AccessMode::InteriorAddress
    ) {
        PinCarrier::StackSlot
    } else {
        PinCarrier::None
    };
    Ok(MemoryPlan {
        owner,
        root,
        pin,
        reload_after_boundary: request.managed == ManagedClass::Managed
            && pin == PinCarrier::None
            && !request.boundaries.is_empty(),
        releases_on: request.exits,
    })
}
