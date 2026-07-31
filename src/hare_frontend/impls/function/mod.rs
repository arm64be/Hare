use hare_ir::VisitorInstruction;

use crate::{FrontendError, LoweredInstruction, OpcodeFamily, lower_owned_instruction};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/hare/function/inventory.rs"
));

pub fn lower(instruction: &VisitorInstruction) -> Result<LoweredInstruction, FrontendError> {
    lower_owned_instruction(OpcodeFamily::Function, instruction)
}
