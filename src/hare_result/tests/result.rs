use std::cell::Cell;

use hare_result::{
    CallGraphResultFacts, Cause, DataMaterializer, DataSchemaId, ErrorId, FiberId, LogicalFrameId,
    MicroCause, NativeFailure, NativeResult, ResultIrOp, ResultLayout, SourceId, StaticTraceTables,
    TraceRecipe, TransactionExit, materialize_result_failure, select_result_layout,
    validate_result_contract,
};

const FRAMES: &[LogicalFrameId] = &[LogicalFrameId(0), LogicalFrameId(1)];

struct CountingMaterializer {
    calls: Cell<usize>,
}

impl DataMaterializer<&'static [(&'static str, i32)]> for CountingMaterializer {
    type Materialized = Vec<(&'static str, i32)>;

    fn materialize(
        &self,
        _schema: DataSchemaId,
        captured: &&'static [(&'static str, i32)],
    ) -> Self::Materialized {
        self.calls.set(self.calls.get() + 1);
        captured.to_vec()
    }
}

fn tables() -> StaticTraceTables<'static> {
    StaticTraceTables {
        errors: &["request failed"],
        sources: &["src/app.ts:7:3"],
        logical_frames: &["fetchUser", "Effect.retry"],
        data_schema_count: 1,
    }
}

fn recipe() -> TraceRecipe {
    TraceRecipe {
        error: ErrorId(0),
        source: SourceId(0),
        logical_frames: FRAMES,
        data_schema: DataSchemaId(0),
    }
}

#[test]
fn success_performs_no_failure_or_backtrace_work() {
    validate_result_contract().unwrap();
    let result: NativeResult<i32, &'static str, &'static str, &'static [(&'static str, i32)]> =
        NativeResult::success(42);
    let materializer = CountingMaterializer {
        calls: Cell::new(0),
    };
    assert!(materialize_result_failure(&result, &tables(), &materializer).is_none());
    assert_eq!(materializer.calls.get(), 0);

    let lowering = hare_result::ResultLowering::tier1();
    assert!(
        lowering
            .entry
            .iter()
            .chain(&lowering.success)
            .all(|operation| !operation.is_failure_only())
    );
    assert!(lowering.failure.contains(&ResultIrOp::BuildBacktrace));
}

#[test]
fn failure_materializes_static_ids_and_dictionary_after_the_edge() {
    let result: NativeResult<(), &str, &str, &'static [(&'static str, i32)]> =
        NativeResult::failure(NativeFailure {
            cause: Cause::Fail("not found"),
            trace: recipe(),
            captured_data: &[("user_id", 7)],
        });
    let materializer = CountingMaterializer {
        calls: Cell::new(0),
    };
    assert_eq!(materializer.calls.get(), 0);
    let materialized = materialize_result_failure(&result, &tables(), &materializer)
        .unwrap()
        .unwrap();
    assert_eq!(materializer.calls.get(), 1);
    assert_eq!(materialized.error, "request failed");
    assert_eq!(materialized.source, "src/app.ts:7:3");
    assert_eq!(materialized.logical_frames, ["fetchUser", "Effect.retry"]);
    assert_eq!(materialized.data, [("user_id", 7)]);
}

#[test]
fn effect_cause_topology_and_failure_kinds_are_lossless() {
    let fiber = FiberId {
        start_time_millis: 10,
        sequence: 2,
    };
    let cause = Cause::parallel(
        Cause::Fail("typed"),
        Cause::sequential(Cause::Die("defect"), Cause::Interrupt(fiber)),
    );
    assert_eq!(cause.leaf_count(), 3);
    assert!(matches!(cause, Cause::Parallel(_, _)));
    assert_ne!(
        TransactionExit::<(), &str, &str>::Retry,
        TransactionExit::Interrupt(fiber)
    );
    let micro = MicroCause::<&str, &str>::Interrupt { trace: recipe() };
    assert!(matches!(micro, MicroCause::Interrupt { .. }));
}

#[test]
fn compact_layout_is_selected_from_call_graph_facts() {
    assert_eq!(
        select_result_layout(CallGraphResultFacts {
            success_reachable: true,
            failure_reachable: false,
            maximum_payload_bits: 64,
            crosses_native_abi: false,
        }),
        Ok(ResultLayout::DirectSuccess)
    );
    assert_eq!(
        select_result_layout(CallGraphResultFacts {
            success_reachable: true,
            failure_reachable: true,
            maximum_payload_bits: 63,
            crosses_native_abi: false,
        }),
        Ok(ResultLayout::LowBitTaggedWord)
    );
    assert_eq!(
        select_result_layout(CallGraphResultFacts {
            success_reachable: true,
            failure_reachable: true,
            maximum_payload_bits: 8,
            crosses_native_abi: true,
        }),
        Ok(ResultLayout::TaggedCompletionAbi)
    );
}

#[test]
fn malformed_static_ids_fail_without_materializing_data() {
    let result: NativeResult<(), &str, &str, &'static [(&'static str, i32)]> =
        NativeResult::failure(NativeFailure {
            cause: Cause::Die("bad"),
            trace: TraceRecipe {
                error: ErrorId(99),
                ..recipe()
            },
            captured_data: &[],
        });
    let materializer = CountingMaterializer {
        calls: Cell::new(0),
    };
    assert!(
        materialize_result_failure(&result, &tables(), &materializer)
            .unwrap()
            .is_err()
    );
    assert_eq!(materializer.calls.get(), 0);
}
