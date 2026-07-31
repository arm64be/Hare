use std::collections::BTreeSet;
use std::fmt;

use hare_effect::{ALL_EFFECT_DOMAINS, CompletionTopology, DOMAIN_CONTRACTS, EffectDomain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultIrOp {
    TestTag,
    ForwardSuccessValue,
    ForwardFailureRecipe,
    MaterializeDataDictionary,
    ResolveErrorString,
    ResolveSourceString,
    ResolveLogicalFrames,
    BuildBacktrace,
    ConstructEffectCause,
}

impl ResultIrOp {
    pub const fn is_failure_only(self) -> bool {
        matches!(
            self,
            Self::ForwardFailureRecipe
                | Self::MaterializeDataDictionary
                | Self::ResolveErrorString
                | Self::ResolveSourceString
                | Self::ResolveLogicalFrames
                | Self::BuildBacktrace
                | Self::ConstructEffectCause
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultLowering {
    pub entry: Vec<ResultIrOp>,
    pub success: Vec<ResultIrOp>,
    pub failure: Vec<ResultIrOp>,
}

impl ResultLowering {
    pub fn tier1() -> Self {
        Self {
            entry: vec![ResultIrOp::TestTag],
            success: vec![ResultIrOp::ForwardSuccessValue],
            failure: vec![
                ResultIrOp::ForwardFailureRecipe,
                ResultIrOp::ResolveErrorString,
                ResultIrOp::ResolveSourceString,
                ResultIrOp::ResolveLogicalFrames,
                ResultIrOp::MaterializeDataDictionary,
                ResultIrOp::BuildBacktrace,
                ResultIrOp::ConstructEffectCause,
            ],
        }
    }

    pub fn validate(&self) -> Result<(), ResultContractError> {
        if self
            .entry
            .iter()
            .chain(&self.success)
            .any(|operation| operation.is_failure_only())
        {
            return Err(ResultContractError::FailureWorkOnSuccess);
        }
        for required in [
            ResultIrOp::ForwardFailureRecipe,
            ResultIrOp::MaterializeDataDictionary,
            ResultIrOp::ResolveErrorString,
            ResultIrOp::ResolveSourceString,
            ResultIrOp::ResolveLogicalFrames,
            ResultIrOp::BuildBacktrace,
            ResultIrOp::ConstructEffectCause,
        ] {
            if !self.failure.contains(&required) {
                return Err(ResultContractError::IncompleteFailurePath(required));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallGraphResultFacts {
    pub success_reachable: bool,
    pub failure_reachable: bool,
    pub maximum_payload_bits: u16,
    pub crosses_native_abi: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultLayout {
    DirectSuccess,
    DirectFailure,
    LowBitTaggedWord,
    TagAndPayload,
    TaggedCompletionAbi,
}

pub fn select_result_layout(
    facts: CallGraphResultFacts,
) -> Result<ResultLayout, ResultContractError> {
    match (facts.success_reachable, facts.failure_reachable) {
        (false, false) => Err(ResultContractError::UnreachableResult),
        (true, false) => Ok(ResultLayout::DirectSuccess),
        (false, true) => Ok(ResultLayout::DirectFailure),
        (true, true) if facts.crosses_native_abi => Ok(ResultLayout::TaggedCompletionAbi),
        (true, true) if facts.maximum_payload_bits <= 63 => Ok(ResultLayout::LowBitTaggedWord),
        (true, true) => Ok(ResultLayout::TagAndPayload),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainResultContract {
    pub domain: EffectDomain,
    pub completion: CompletionTopology,
    pub preserves_cause_topology: bool,
    pub lazy_failure_materialization: bool,
}

pub fn domain_result_contracts() -> Vec<DomainResultContract> {
    DOMAIN_CONTRACTS
        .iter()
        .map(|contract| DomainResultContract {
            domain: contract.domain,
            completion: contract.completion,
            preserves_cause_topology: true,
            lazy_failure_materialization: true,
        })
        .collect()
}

pub fn validate_result_contract() -> Result<(), ResultContractError> {
    ResultLowering::tier1().validate()?;
    let domains = domain_result_contracts();
    let unique = domains
        .iter()
        .map(|contract| contract.domain)
        .collect::<BTreeSet<_>>();
    let required = ALL_EFFECT_DOMAINS.iter().copied().collect::<BTreeSet<_>>();
    if unique != required
        || domains.iter().any(|contract| {
            !contract.preserves_cause_topology || !contract.lazy_failure_materialization
        })
    {
        return Err(ResultContractError::DomainCoverage);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultContractError {
    FailureWorkOnSuccess,
    IncompleteFailurePath(ResultIrOp),
    UnreachableResult,
    DomainCoverage,
}

impl fmt::Display for ResultContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ResultContractError {}
