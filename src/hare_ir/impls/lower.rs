use crate::{
    BasicBlock, BasicBlockId, CompilationUnit, Function, HARE_IR_SCHEMA_VERSION, ImportError,
    OwnedVisitorUnit, Terminator, ValidationError,
};

/// Converts a structurally complete visitor unit to the initial core IR.
/// Opcode-family operations are filled by H012-H017 before this gate is used
/// for an application build.
pub fn lower_structural_unit(visitor: OwnedVisitorUnit) -> Result<CompilationUnit, ImportError> {
    visitor.validate().map_err(ImportError::Validation)?;
    if !visitor.structurally_complete {
        return Err(ImportError::Validation(
            ValidationError::StructuralCoverageIncomplete,
        ));
    }
    let functions = visitor
        .functions
        .iter()
        .map(|function| Function {
            id: function.id,
            source: function.source,
            specialization: function.specialization,
            parameters: Vec::new(),
            values: Vec::new(),
            blocks: vec![BasicBlock {
                id: BasicBlockId(0),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Unreachable,
            }],
            direct_eval_context: None,
            function_constructor_context: None,
        })
        .collect();
    let unit = CompilationUnit {
        schema_version: HARE_IR_SCHEMA_VERSION,
        sources: visitor.sources,
        functions,
        entry_points: vec![crate::FunctionId(0)],
        capabilities: Vec::new(),
    };
    unit.validate().map_err(ImportError::Validation)?;
    Ok(unit)
}
