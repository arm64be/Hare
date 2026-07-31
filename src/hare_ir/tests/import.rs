use hare_ir::{
    ConstantSourceRepresentation, DumpIdentity, FIRST_CONSTANT_REGISTER_INDEX, FunctionId,
    FunctionRelation, FunctionSpecialization, ImportBuilder, InputKind, OperandRole, OperandValue,
    SourceId, SourceRecord, SourceText, VisitorConstant, VisitorConstantValue, render_visitor_dump,
};

#[test]
fn structural_import_is_owned_and_validated() {
    let source = SourceRecord {
        id: SourceId(0),
        public_name: "/tmp/random-workspace/entry.js".into(),
        text: SourceText::Latin1(Box::from(&b"return 1"[..])),
        start_line: 1,
        start_column: 0,
    };
    let mut builder = ImportBuilder::new(InputKind::ClassicProgram, source);
    builder
        .begin_function(
            FunctionId(0),
            None,
            FunctionRelation::Root,
            FunctionSpecialization::Program,
            SourceId(0),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            -1,
            -1,
            5,
            6,
            2,
        )
        .unwrap();
    builder
        .constant(
            0,
            VisitorConstant {
                value: VisitorConstantValue::Int32(1),
                source_representation: ConstantSourceRepresentation::Integer,
            },
        )
        .unwrap();
    builder
        .identifier(0, SourceText::Latin1(Box::from(&b"answer"[..])))
        .unwrap();
    builder.instruction(0, 1, 2, 1, 1).unwrap();
    builder
        .operand(
            "test.constant".into(),
            OperandRole::ValueUse,
            OperandValue::Signed(FIRST_CONSTANT_REGISTER_INDEX),
        )
        .unwrap();
    builder.mark_structurally_complete();
    let unit = builder.finish().unwrap();
    assert_eq!(unit.functions[0].instructions[0].opcode_id, 1);
    assert_eq!(
        unit.functions[0].constants[0].value,
        VisitorConstantValue::Int32(1)
    );
    assert_eq!(unit.sources[0].text.code_unit_len(), 8);

    let identity = DumpIdentity {
        target: "x86_64-unknown-linux-gnu",
        profile: "debug",
    };
    let first = render_visitor_dump(&unit, identity).unwrap();
    let second = render_visitor_dump(&unit, identity).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("name=\"<absolute>/entry.js\""));
    assert!(first.contains("constant k0 source=Integer value=int32:1"));
    assert!(first.contains("identifier id0 value=latin1:616e73776572"));
    assert!(!first.contains("random-workspace"));
}
