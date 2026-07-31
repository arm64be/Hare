use crate::{
    BasicBlock, BasicBlockId, CompilationUnit, Function, FunctionId, Operation, SourceId, ValueId,
};

/// Read-only interface used by analyses that do not need Hare's concrete
/// storage representation.
pub trait UnitView {
    type Function: FunctionView;

    fn entry_points(&self) -> &[FunctionId];
    fn functions(&self) -> &[Self::Function];
}

/// Read-only function interface for generic analysis and validation passes.
pub trait FunctionView {
    type Block: BlockView;

    fn id(&self) -> FunctionId;
    fn source(&self) -> SourceId;
    fn blocks(&self) -> &[Self::Block];
}

/// Read-only basic-block interface.
pub trait BlockView {
    type Operation: OperationView;

    fn id(&self) -> BasicBlockId;
    fn parameters(&self) -> &[ValueId];
    fn operations(&self) -> &[Self::Operation];
}

/// Read-only operation interface.
pub trait OperationView {
    fn results(&self) -> &[ValueId];
}

impl UnitView for CompilationUnit {
    type Function = Function;

    fn entry_points(&self) -> &[FunctionId] {
        &self.entry_points
    }

    fn functions(&self) -> &[Self::Function] {
        &self.functions
    }
}

impl FunctionView for Function {
    type Block = BasicBlock;

    fn id(&self) -> FunctionId {
        self.id
    }

    fn source(&self) -> SourceId {
        self.source
    }

    fn blocks(&self) -> &[Self::Block] {
        &self.blocks
    }
}

impl BlockView for BasicBlock {
    type Operation = Operation;

    fn id(&self) -> BasicBlockId {
        self.id
    }

    fn parameters(&self) -> &[ValueId] {
        &self.parameters
    }

    fn operations(&self) -> &[Self::Operation] {
        &self.operations
    }
}

impl OperationView for Operation {
    fn results(&self) -> &[ValueId] {
        &self.results
    }
}
