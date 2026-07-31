#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackContainment {
    Escapes,
    FinallyMasking,
    RunLoopDie,
    InitiateAsyncDefect,
    SourceSiteSpecific,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptedContainment {
    SameAsOrdinary,
    SequentialDefectAndInterrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackPhase {
    BeforeEvaluateTry,
    EvaluateFinally,
    BeforeRunLoopTry,
    InRunLoopTry,
    ExitReporting,
    AsyncInitiation,
    OutsideRunLoop,
    SourceSiteSpecific,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallbackContract {
    pub boundary: &'static str,
    pub phase: CallbackPhase,
    pub ordinary_throw: CallbackContainment,
    pub interrupted_throw: InterruptedContainment,
    pub publishes_current_fiber: bool,
    pub one_shot: bool,
}

macro_rules! callback {
    ($boundary:literal, $phase:ident, $ordinary:ident, $interrupted:ident, $published:literal, $one_shot:literal) => {
        CallbackContract {
            boundary: $boundary,
            phase: CallbackPhase::$phase,
            ordinary_throw: CallbackContainment::$ordinary,
            interrupted_throw: InterruptedContainment::$interrupted,
            publishes_current_fiber: $published,
            one_shot: $one_shot,
        }
    };
}

pub const CALLBACK_CONTRACTS: &[CallbackContract] = &[
    callback!(
        "supervisor.onResume",
        BeforeEvaluateTry,
        Escapes,
        SameAsOrdinary,
        true,
        false
    ),
    callback!(
        "supervisor.onSuspend",
        EvaluateFinally,
        FinallyMasking,
        SameAsOrdinary,
        true,
        false
    ),
    callback!(
        "supervisor.onEffect",
        BeforeRunLoopTry,
        Escapes,
        SameAsOrdinary,
        true,
        false
    ),
    callback!(
        "scheduler.shouldYield",
        BeforeRunLoopTry,
        Escapes,
        SameAsOrdinary,
        true,
        false
    ),
    callback!(
        "runningFiberMessage",
        BeforeRunLoopTry,
        Escapes,
        SameAsOrdinary,
        true,
        true
    ),
    callback!(
        "suspendedMessage.onFiber",
        OutsideRunLoop,
        Escapes,
        SameAsOrdinary,
        true,
        true
    ),
    callback!(
        "tracerAndPrimitiveDispatch",
        InRunLoopTry,
        RunLoopDie,
        SequentialDefectAndInterrupt,
        true,
        false
    ),
    callback!(
        "customEffectable.commit",
        InRunLoopTry,
        RunLoopDie,
        SequentialDefectAndInterrupt,
        true,
        false
    ),
    callback!(
        "exitObserver",
        ExitReporting,
        Escapes,
        SameAsOrdinary,
        true,
        true
    ),
    callback!(
        "asyncRegister",
        AsyncInitiation,
        InitiateAsyncDefect,
        SameAsOrdinary,
        true,
        true
    ),
    callback!(
        "asyncResumeTell",
        OutsideRunLoop,
        Escapes,
        SameAsOrdinary,
        false,
        true
    ),
    callback!(
        "scheduler.scheduleTask",
        OutsideRunLoop,
        Escapes,
        SameAsOrdinary,
        false,
        false
    ),
    callback!(
        "serviceLoggerMetricTestCallback",
        SourceSiteSpecific,
        SourceSiteSpecific,
        SameAsOrdinary,
        true,
        false
    ),
];
