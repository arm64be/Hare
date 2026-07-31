use std::collections::BTreeSet;
use std::fmt;

use hare_ir::EffectSet;

use crate::{CALLBACK_CONTRACTS, CallbackContainment, SelectedEffectInventory};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EffectDomain {
    CoreRunLoop,
    CustomEffectable,
    CurrentFiberAmbient,
    CauseAndExit,
    FibersAndFiberRefs,
    ScopesAndFinalizers,
    Resources,
    ContextAndServices,
    Layers,
    SchedulerClockInterruption,
    AsyncHostBoundary,
    Coordination,
    Concurrency,
    Requests,
    Stm,
    ChannelStreamSink,
    Schedule,
    ConfigAndSchema,
    ObservabilityAndTests,
    Micro,
    PackageIdentity,
}

pub const ALL_EFFECT_DOMAINS: &[EffectDomain] = &[
    EffectDomain::CoreRunLoop,
    EffectDomain::CustomEffectable,
    EffectDomain::CurrentFiberAmbient,
    EffectDomain::CauseAndExit,
    EffectDomain::FibersAndFiberRefs,
    EffectDomain::ScopesAndFinalizers,
    EffectDomain::Resources,
    EffectDomain::ContextAndServices,
    EffectDomain::Layers,
    EffectDomain::SchedulerClockInterruption,
    EffectDomain::AsyncHostBoundary,
    EffectDomain::Coordination,
    EffectDomain::Concurrency,
    EffectDomain::Requests,
    EffectDomain::Stm,
    EffectDomain::ChannelStreamSink,
    EffectDomain::Schedule,
    EffectDomain::ConfigAndSchema,
    EffectDomain::ObservabilityAndTests,
    EffectDomain::Micro,
    EffectDomain::PackageIdentity,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectOp {
    ValidateBrandAndVersion,
    DispatchPrimitive,
    PushContinuation,
    PopContinuation,
    YieldFiber,
    ResumeFiber,
    CallCustomCommit,
    RevalidateReturnedEffect,
    SaveAmbient,
    PublishCurrentFiber,
    RestoreAmbient,
    PropagateCauseTree,
    ForkFiberRefs,
    JoinFiberRefs,
    DrainInbox,
    NotifyObserversReverse,
    RegisterFinalizer,
    CloseScope,
    RunFinalizers,
    AcquireResource,
    RefreshResource,
    ReleaseResource,
    LookupService,
    ProvideContext,
    BuildLayer,
    MemoizeLayer,
    CheckInterruption,
    PatchRuntimeFlags,
    ScheduleTask,
    ReadClock,
    RegisterAsync,
    ResumeAsyncOneShot,
    CancelAsync,
    CompleteDeferred,
    QueueTransition,
    PubSubTransition,
    AcquirePermit,
    ForkStructuredChild,
    ReleaseStructuredChild,
    PlanBlockedRequests,
    CompleteRequest,
    StmReadJournal,
    StmValidateJournal,
    StmCommitJournal,
    StmRegisterRetry,
    ChannelStep,
    StreamPullEmit,
    SinkConsumeLeftovers,
    ScheduleStep,
    ConfigProviderRead,
    SchemaTransform,
    InvokeObserverService,
    TestClockAdvance,
    MicroDispatch,
    MicroFinalize,
    GlobalValueLookup,
    CheckModuleVersion,
}

impl EffectOp {
    pub const fn effects(self) -> EffectSet {
        match self {
            Self::ValidateBrandAndVersion
            | Self::PopContinuation
            | Self::PropagateCauseTree
            | Self::StmReadJournal => EffectSet::HEAP_READ,
            Self::PushContinuation
            | Self::SaveAmbient
            | Self::PublishCurrentFiber
            | Self::RestoreAmbient
            | Self::ForkFiberRefs
            | Self::JoinFiberRefs
            | Self::PatchRuntimeFlags
            | Self::QueueTransition
            | Self::PubSubTransition
            | Self::StmValidateJournal
            | Self::StmCommitJournal
            | Self::GlobalValueLookup
            | Self::CheckModuleVersion => EffectSet::HEAP_READ.union(EffectSet::HEAP_WRITE),
            Self::DispatchPrimitive
            | Self::ResumeFiber
            | Self::RevalidateReturnedEffect
            | Self::CheckInterruption => EffectSet::MAY_THROW,
            Self::CallCustomCommit
            | Self::LookupService
            | Self::ProvideContext
            | Self::BuildLayer
            | Self::ConfigProviderRead
            | Self::SchemaTransform
            | Self::InvokeObserverService => EffectSet::MAY_CALL_USER
                .union(EffectSet::MAY_THROW)
                .union(EffectSet::MAY_ALLOCATE)
                .union(EffectSet::SAFEPOINT),
            Self::YieldFiber
            | Self::RegisterAsync
            | Self::ResumeAsyncOneShot
            | Self::CancelAsync
            | Self::ScheduleTask
            | Self::ReadClock
            | Self::RefreshResource
            | Self::PlanBlockedRequests
            | Self::StmRegisterRetry
            | Self::ChannelStep
            | Self::StreamPullEmit
            | Self::ScheduleStep
            | Self::TestClockAdvance
            | Self::MicroDispatch => EffectSet::MAY_SUSPEND
                .union(EffectSet::SAFEPOINT)
                .union(EffectSet::INTERRUPTION_CHECK),
            _ => EffectSet::HEAP_READ
                .union(EffectSet::HEAP_WRITE)
                .union(EffectSet::MAY_ALLOCATE)
                .union(EffectSet::SAFEPOINT),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionTopology {
    EffectCause,
    EffectExit,
    RetryOrExit,
    ChannelStateOrExit,
    MicroExit,
    OrdinaryValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainContract {
    pub domain: EffectDomain,
    pub operations: &'static [EffectOp],
    pub memory_rows: &'static [&'static str],
    pub completion: CompletionTopology,
    pub generic_native_fallback: bool,
    pub distinct_machine: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectAwareInstruction {
    pub domain: EffectDomain,
    pub ordinal: u16,
    pub operation: EffectOp,
    pub effects: EffectSet,
}

macro_rules! domain {
    ($domain:ident, [$($operation:ident),+ $(,)?], [$($row:literal),+ $(,)?], $completion:ident) => {
        DomainContract {
            domain: EffectDomain::$domain,
            operations: &[$(EffectOp::$operation),+],
            memory_rows: &[$($row),+],
            completion: CompletionTopology::$completion,
            generic_native_fallback: true,
            distinct_machine: false,
        }
    };
}

pub const DOMAIN_CONTRACTS: &[DomainContract] = &[
    domain!(
        CoreRunLoop,
        [
            ValidateBrandAndVersion,
            DispatchPrimitive,
            PushContinuation,
            PopContinuation,
            YieldFiber,
            ResumeFiber
        ],
        ["H004-E06", "H004-E17"],
        EffectExit
    ),
    domain!(
        CustomEffectable,
        [CallCustomCommit, RevalidateReturnedEffect],
        ["H004-C04", "H004-C05"],
        EffectCause
    ),
    domain!(
        CurrentFiberAmbient,
        [SaveAmbient, PublishCurrentFiber, RestoreAmbient],
        ["H004-E17", "H004-E18"],
        OrdinaryValue
    ),
    domain!(
        CauseAndExit,
        [PropagateCauseTree],
        ["H004-E01", "H004-C01"],
        EffectCause
    ),
    domain!(
        FibersAndFiberRefs,
        [
            ForkFiberRefs,
            JoinFiberRefs,
            DrainInbox,
            NotifyObserversReverse
        ],
        ["H004-E06", "H004-E07", "H004-E18"],
        EffectExit
    ),
    domain!(
        ScopesAndFinalizers,
        [RegisterFinalizer, CloseScope, RunFinalizers],
        ["H004-E01", "H004-E08"],
        EffectCause
    ),
    domain!(
        Resources,
        [AcquireResource, RefreshResource, ReleaseResource],
        ["H004-E01", "H004-E13"],
        EffectExit
    ),
    domain!(
        ContextAndServices,
        [LookupService, ProvideContext],
        ["H004-E07", "H004-E14"],
        EffectCause
    ),
    domain!(
        Layers,
        [BuildLayer, MemoizeLayer],
        ["H004-E08", "H004-E14"],
        EffectExit
    ),
    domain!(
        SchedulerClockInterruption,
        [
            CheckInterruption,
            PatchRuntimeFlags,
            ScheduleTask,
            ReadClock
        ],
        ["H004-E13", "H004-E17"],
        EffectCause
    ),
    domain!(
        AsyncHostBoundary,
        [RegisterAsync, ResumeAsyncOneShot, CancelAsync],
        ["H004-A03", "H004-E18", "H004-F04"],
        EffectExit
    ),
    domain!(
        Coordination,
        [CompleteDeferred, QueueTransition, PubSubTransition],
        ["H004-E09", "H004-E18"],
        EffectExit
    ),
    domain!(
        Concurrency,
        [AcquirePermit, ForkStructuredChild, ReleaseStructuredChild],
        ["H004-E02", "H004-E03", "H004-E04"],
        EffectCause
    ),
    domain!(
        Requests,
        [PlanBlockedRequests, CompleteRequest],
        ["H004-E10", "H004-E18"],
        EffectExit
    ),
    domain!(
        Stm,
        [
            StmReadJournal,
            StmValidateJournal,
            StmCommitJournal,
            StmRegisterRetry
        ],
        ["H004-E11", "H004-E18"],
        RetryOrExit
    ),
    domain!(
        ChannelStreamSink,
        [ChannelStep, StreamPullEmit, SinkConsumeLeftovers],
        ["H004-E12", "H004-G01"],
        ChannelStateOrExit
    ),
    domain!(
        Schedule,
        [ScheduleStep],
        ["H004-E13", "H004-E18"],
        EffectExit
    ),
    domain!(
        ConfigAndSchema,
        [ConfigProviderRead, SchemaTransform],
        ["H004-E14", "H004-C05"],
        EffectCause
    ),
    domain!(
        ObservabilityAndTests,
        [InvokeObserverService, TestClockAdvance],
        ["H004-E15", "H004-E18"],
        EffectExit
    ),
    DomainContract {
        distinct_machine: true,
        ..domain!(
            Micro,
            [MicroDispatch, MicroFinalize],
            ["H004-E16", "H004-E18"],
            MicroExit
        )
    },
    domain!(
        PackageIdentity,
        [GlobalValueLookup, CheckModuleVersion],
        ["H004-E14", "H004-E17"],
        OrdinaryValue
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleStrategy {
    GenericNative,
    DataAsset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleLowering<'a> {
    pub path: &'a str,
    pub strategy: ModuleStrategy,
    pub aware_domains: BTreeSet<EffectDomain>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectLoweringError {
    Manifest(crate::ManifestError),
    NotInSelectedPin(Box<str>),
    RuntimeSourceCompilation(Box<str>),
    DomainCoverage(BTreeSet<EffectDomain>),
    DuplicateDomain(EffectDomain),
    EmptyDomain(EffectDomain),
    MissingGenericNative(EffectDomain),
    UnknownDomain(EffectDomain),
    MissingMemoryRow(&'static str),
    CallbackContract,
}

impl fmt::Display for EffectLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EffectLoweringError {}

impl From<crate::ManifestError> for EffectLoweringError {
    fn from(error: crate::ManifestError) -> Self {
        Self::Manifest(error)
    }
}

pub fn lower_reachable_module<'a>(
    inventory: &SelectedEffectInventory,
    path: &'a str,
    performs_runtime_source_compilation: bool,
) -> Result<ModuleLowering<'a>, EffectLoweringError> {
    if performs_runtime_source_compilation {
        return Err(EffectLoweringError::RuntimeSourceCompilation(path.into()));
    }
    if !inventory.contains_artifact(path) {
        return Err(EffectLoweringError::NotInSelectedPin(path.into()));
    }
    let executable = (path.ends_with(".ts") && !path.ends_with(".d.ts"))
        || path.ends_with(".js")
        || path.ends_with(".mjs")
        || path.ends_with(".cjs");
    Ok(ModuleLowering {
        path,
        strategy: if executable {
            ModuleStrategy::GenericNative
        } else {
            ModuleStrategy::DataAsset
        },
        aware_domains: if executable {
            classify_domains(path)
        } else {
            BTreeSet::new()
        },
    })
}

pub fn lower_effect_domain(
    domain: EffectDomain,
) -> Result<Vec<EffectAwareInstruction>, EffectLoweringError> {
    let contract = DOMAIN_CONTRACTS
        .iter()
        .find(|contract| contract.domain == domain)
        .ok_or(EffectLoweringError::UnknownDomain(domain))?;
    Ok(contract
        .operations
        .iter()
        .enumerate()
        .map(|(ordinal, operation)| EffectAwareInstruction {
            domain,
            ordinal: ordinal as u16,
            operation: *operation,
            effects: operation.effects(),
        })
        .collect())
}

pub fn validate_effect_lowering() -> Result<(), EffectLoweringError> {
    let inventory = SelectedEffectInventory::load()?;
    let memory_rows = hare_memory::lifetime_inventory()
        .map_err(|_| EffectLoweringError::MissingMemoryRow("invalid H019 inventory"))?
        .into_iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let mut contracts = BTreeSet::new();
    for contract in DOMAIN_CONTRACTS {
        if !contracts.insert(contract.domain) {
            return Err(EffectLoweringError::DuplicateDomain(contract.domain));
        }
        if contract.operations.is_empty() || contract.memory_rows.is_empty() {
            return Err(EffectLoweringError::EmptyDomain(contract.domain));
        }
        if !contract.generic_native_fallback {
            return Err(EffectLoweringError::MissingGenericNative(contract.domain));
        }
        for row in contract.memory_rows {
            if !memory_rows.contains(row) {
                return Err(EffectLoweringError::MissingMemoryRow(row));
            }
        }
    }
    let required = ALL_EFFECT_DOMAINS.iter().copied().collect::<BTreeSet<_>>();
    let missing = required
        .difference(&contracts)
        .copied()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        return Err(EffectLoweringError::DomainCoverage(missing));
    }
    for domain in ALL_EFFECT_DOMAINS {
        if lower_effect_domain(*domain)?.is_empty() {
            return Err(EffectLoweringError::EmptyDomain(*domain));
        }
    }

    let discovered = inventory
        .source_files
        .iter()
        .flat_map(|path| classify_domains(path))
        .collect::<BTreeSet<_>>();
    let missing = required
        .difference(&discovered)
        .copied()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        return Err(EffectLoweringError::DomainCoverage(missing));
    }

    let containments = CALLBACK_CONTRACTS
        .iter()
        .map(|contract| contract.ordinary_throw)
        .collect::<Vec<_>>();
    if CALLBACK_CONTRACTS.len() != 13
        || !containments.contains(&CallbackContainment::Escapes)
        || !containments.contains(&CallbackContainment::RunLoopDie)
        || !containments.contains(&CallbackContainment::InitiateAsyncDefect)
    {
        return Err(EffectLoweringError::CallbackContract);
    }
    Ok(())
}

pub fn classify_domains(path: &str) -> BTreeSet<EffectDomain> {
    let path = path.to_ascii_lowercase();
    let mut domains = BTreeSet::new();
    let mut add = |needles: &[&str], domain| {
        if needles.iter().any(|needle| path.contains(needle)) {
            domains.insert(domain);
        }
    };
    add(
        &["core", "effect.ts", "fiberruntime"],
        EffectDomain::CoreRunLoop,
    );
    add(&["effectable"], EffectDomain::CustomEffectable);
    add(
        &["fiberruntime", "fiber.ts"],
        EffectDomain::CurrentFiberAmbient,
    );
    add(&["cause", "exit", "texit"], EffectDomain::CauseAndExit);
    add(&["fiber"], EffectDomain::FibersAndFiberRefs);
    add(&["scope", "finalizer"], EffectDomain::ScopesAndFinalizers);
    add(
        &["resource", "reloadable", "scopedref"],
        EffectDomain::Resources,
    );
    add(
        &["context", "defaultservice"],
        EffectDomain::ContextAndServices,
    );
    add(&["layer"], EffectDomain::Layers);
    add(
        &["scheduler", "clock", "runtimeflag"],
        EffectDomain::SchedulerClockInterruption,
    );
    add(
        &["core", "runtime", "abort", "async"],
        EffectDomain::AsyncHostBoundary,
    );
    add(
        &["deferred", "queue", "pubsub", "handoff", "mailbox"],
        EffectDomain::Coordination,
    );
    add(
        &[
            "concurrency",
            "semaphore",
            "pool",
            "ratelimiter",
            "rcmap",
            "rcref",
            "synchronizedref",
        ],
        EffectDomain::Concurrency,
    );
    add(&["request", "datasource"], EffectDomain::Requests);
    add(
        &[
            "/stm",
            "/tref",
            "/tmap",
            "/tqueue",
            "/tpubsub",
            "/tset",
            "/tarray",
            "/tdeferred",
            "/tsemaphore",
            "journal",
        ],
        EffectDomain::Stm,
    );
    add(
        &["channel", "stream", "sink"],
        EffectDomain::ChannelStreamSink,
    );
    add(&["schedule", "cron"], EffectDomain::Schedule);
    add(
        &["config", "schema", "parseresult"],
        EffectDomain::ConfigAndSchema,
    );
    add(
        &[
            "logger",
            "tracer",
            "metric",
            "supervisor",
            "test",
            "logspan",
            "loglevel",
        ],
        EffectDomain::ObservabilityAndTests,
    );
    add(&["micro"], EffectDomain::Micro);
    add(
        &["globalvalue", "moduleversion"],
        EffectDomain::PackageIdentity,
    );
    domains
}
