use std::collections::BTreeMap;

use hare_analysis::{Fact, FunctionSummary, WholeProgram};
use hare_ir::{
    BasicBlock, BasicBlockId, CompilationUnit, Function, FunctionId, FunctionSpecialization,
    HARE_IR_SCHEMA_VERSION, SourceId, SourceRecord, SourceText, Terminator,
};

#[test]
fn merge_never_promotes_a_hint() {
    assert_eq!(Fact::Proof(1).join(&Fact::Hint(1)), Fact::Hint(1));
    assert_eq!(Fact::Guard(1).join(&Fact::Unknown), Fact::Unknown);
    assert_eq!(Fact::Proof(1).join(&Fact::Proof(2)), Fact::Unknown);
    assert!(!Fact::Hint(1).is_binding());
}

#[test]
fn whole_program_worklist_revisits_callers() {
    let source = SourceRecord {
        id: SourceId(0),
        public_name: "entry.js".into(),
        text: SourceText::Latin1(Box::default()),
        start_line: 1,
        start_column: 0,
    };
    let make_function = |id| Function {
        id: FunctionId(id),
        source: SourceId(0),
        specialization: FunctionSpecialization::Call,
        parameters: Vec::new(),
        values: Vec::new(),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return(None),
        }],
        direct_eval_context: None,
        function_constructor_context: None,
    };
    let unit = CompilationUnit {
        schema_version: HARE_IR_SCHEMA_VERSION,
        sources: vec![source],
        functions: vec![make_function(0), make_function(1)],
        entry_points: vec![FunctionId(0)],
        capabilities: Vec::new(),
    };
    let mut program = WholeProgram::<u32>::new(&unit);
    program.add_call(FunctionId(0), FunctionId(1)).unwrap();
    let mut visits = BTreeMap::new();
    program
        .solve(16, |function, callees, summaries| {
            *visits.entry(function).or_insert(0) += 1;
            if function == FunctionId(1) {
                FunctionSummary {
                    return_fact: Fact::Proof(7),
                    may_throw: false,
                    may_suspend: false,
                    may_call_user: false,
                }
            } else {
                let callee = summaries.get(callees.first().unwrap()).unwrap();
                callee.clone()
            }
        })
        .unwrap();
    assert_eq!(
        program.summary(FunctionId(0)).unwrap().return_fact,
        Fact::Proof(7)
    );
    assert!(visits[&FunctionId(0)] >= 1);
}
