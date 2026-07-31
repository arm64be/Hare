use hare_frontend::{
    ImportedValueKind, cache, descriptor, inventories, lower_instruction, validate_inventory,
};
use hare_ir::{OperandValue, VisitorInstruction, VisitorOperand};

#[test]
fn every_semantic_opcode_and_operand_has_typed_owned_lowering() {
    validate_inventory().unwrap();
    let mut lowered = 0;
    for expected in inventories().into_iter().flatten() {
        let operands = expected
            .operands
            .iter()
            .map(|operand| VisitorOperand {
                manifest_id: operand.manifest_id.into(),
                role: operand.role,
                value: match operand.value_kind {
                    ImportedValueKind::Signed => OperandValue::Signed(0),
                    ImportedValueKind::Unsigned => OperandValue::Unsigned(0),
                    ImportedValueKind::Boolean => OperandValue::Boolean(false),
                },
            })
            .collect();
        let instruction = VisitorInstruction {
            byte_offset: 0,
            opcode_id: u32::from(expected.opcode_id),
            encoded_size: 1,
            opcode_id_bytes: 1,
            width_bytes: 1,
            operands,
        };
        let actual = lower_instruction(&instruction).unwrap();
        assert_eq!(actual.opcode, expected.opcode);
        assert_eq!(descriptor(instruction.opcode_id), Some(expected));
        lowered += 1;
    }
    assert_eq!(lowered, 183);
    assert_eq!(cache::OPCODES.len(), 11);
}
