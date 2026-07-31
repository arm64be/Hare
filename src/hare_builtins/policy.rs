#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureSemantics {
    OrdinaryRuntimeApi,
    RuntimeSourceCompiler,
    ClosedGraphLoader,
    ResolutionOnly,
    PathObservingAsset,
    StaticWorker,
    WorkerSourceEval,
    EvalOrFunctionOwnedByH017,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provenance {
    None,
    StaticLiteral,
    StaticGraphMember,
    RuntimeValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileErrorReason {
    RuntimeSourceCompilation,
    GraphTargetUnknown,
    VirtualPathContractUndefined,
    WorkerSourceExecution,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiLowering {
    RuntimeCapability,
    ClosedGraphNative,
    StaticWorkerNative,
    H017Frontend,
    CompileError(CompileErrorReason),
}

pub fn lower_api(semantics: FeatureSemantics, provenance: Provenance) -> ApiLowering {
    match semantics {
        FeatureSemantics::OrdinaryRuntimeApi => ApiLowering::RuntimeCapability,
        FeatureSemantics::RuntimeSourceCompiler => {
            ApiLowering::CompileError(CompileErrorReason::RuntimeSourceCompilation)
        }
        FeatureSemantics::ClosedGraphLoader => {
            if provenance == Provenance::StaticGraphMember {
                ApiLowering::ClosedGraphNative
            } else {
                ApiLowering::CompileError(CompileErrorReason::GraphTargetUnknown)
            }
        }
        FeatureSemantics::ResolutionOnly | FeatureSemantics::PathObservingAsset => {
            ApiLowering::CompileError(CompileErrorReason::VirtualPathContractUndefined)
        }
        FeatureSemantics::StaticWorker => {
            if provenance == Provenance::StaticGraphMember {
                ApiLowering::StaticWorkerNative
            } else {
                ApiLowering::CompileError(CompileErrorReason::GraphTargetUnknown)
            }
        }
        FeatureSemantics::WorkerSourceEval => {
            ApiLowering::CompileError(CompileErrorReason::WorkerSourceExecution)
        }
        FeatureSemantics::EvalOrFunctionOwnedByH017 => ApiLowering::H017Frontend,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestrictedOperation {
    pub name: &'static str,
    pub semantics: FeatureSemantics,
}

pub const RESTRICTED_OPERATIONS: &[RestrictedOperation] = &[
    RestrictedOperation {
        name: "node:vm.Script",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:vm.runInThisContext",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:vm.runInContext",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:vm.runInNewContext",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:vm.compileFunction",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:vm.SourceTextModule",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:vm.Module",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "ShadowRealm.evaluate",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "ShadowRealm.importValue",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "Bun.Transpiler.transform",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "Bun.Transpiler.transformSync",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "Bun.Transpiler.scan",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "Bun.Transpiler.scanSync",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "Bun.build",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "Bun.plugin.onLoadSource",
        semantics: FeatureSemantics::RuntimeSourceCompiler,
    },
    RestrictedOperation {
        name: "node:worker_threads.Worker.eval",
        semantics: FeatureSemantics::WorkerSourceEval,
    },
    RestrictedOperation {
        name: "require.resolve",
        semantics: FeatureSemantics::ResolutionOnly,
    },
    RestrictedOperation {
        name: "import.meta.resolve",
        semantics: FeatureSemantics::ResolutionOnly,
    },
    RestrictedOperation {
        name: "Bun.resolveSync",
        semantics: FeatureSemantics::ResolutionOnly,
    },
    RestrictedOperation {
        name: "path-observing asset",
        semantics: FeatureSemantics::PathObservingAsset,
    },
];
