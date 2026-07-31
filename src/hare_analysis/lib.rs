//! Monotone whole-program analysis primitives for Hare.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use hare_ir::{BasicBlockId, CompilationUnit, FunctionId};

/// The evidence category carried by an inferred value.
///
/// A hint is never binding. A guard is binding only on the edge dominated by
/// its explicit runtime predicate. A proof is compile-time evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fact<T> {
    Proof(T),
    Guard(T),
    Hint(T),
    Unknown,
}

impl<T> Fact<T> {
    pub const fn assurance(&self) -> Assurance {
        match self {
            Self::Proof(_) => Assurance::Proof,
            Self::Guard(_) => Assurance::Guard,
            Self::Hint(_) => Assurance::Hint,
            Self::Unknown => Assurance::Unknown,
        }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Proof(value) | Self::Guard(value) | Self::Hint(value) => Some(value),
            Self::Unknown => None,
        }
    }

    pub const fn is_binding(&self) -> bool {
        matches!(self, Self::Proof(_) | Self::Guard(_))
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Fact<U> {
        match self {
            Self::Proof(value) => Fact::Proof(map(value)),
            Self::Guard(value) => Fact::Guard(map(value)),
            Self::Hint(value) => Fact::Hint(map(value)),
            Self::Unknown => Fact::Unknown,
        }
    }

    /// Erases non-binding profile/type hints before a semantic decision.
    pub fn binding_only(self) -> Self {
        match self {
            Self::Hint(_) => Self::Unknown,
            other => other,
        }
    }
}

impl<T: Eq + Clone> Fact<T> {
    /// Control-flow merge. Only equal values survive; assurance weakens to the
    /// least-assured incoming path.
    pub fn join(&self, other: &Self) -> Self {
        let (Some(left), Some(right)) = (self.value(), other.value()) else {
            return Self::Unknown;
        };
        if left != right {
            return Self::Unknown;
        }
        match self.assurance().min(other.assurance()) {
            Assurance::Proof => Self::Proof(left.clone()),
            Assurance::Guard => Self::Guard(left.clone()),
            Assurance::Hint => Self::Hint(left.clone()),
            Assurance::Unknown => Self::Unknown,
        }
    }

    /// Conjunctive refinement. A stronger equal premise can refine a weaker
    /// one; contradictory values become unknown rather than unsound proof.
    pub fn meet(&self, other: &Self) -> Self {
        match (self.value(), other.value()) {
            (None, _) => other.clone(),
            (_, None) => self.clone(),
            (Some(left), Some(right)) if left != right => Self::Unknown,
            (Some(value), Some(_)) => match self.assurance().max(other.assurance()) {
                Assurance::Proof => Self::Proof(value.clone()),
                Assurance::Guard => Self::Guard(value.clone()),
                Assurance::Hint => Self::Hint(value.clone()),
                Assurance::Unknown => Self::Unknown,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Assurance {
    Unknown,
    Hint,
    Guard,
    Proof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowState<K, V> {
    facts: BTreeMap<K, Fact<V>>,
}

impl<K, V> Default for FlowState<K, V> {
    fn default() -> Self {
        Self {
            facts: BTreeMap::new(),
        }
    }
}

impl<K: Ord, V> FlowState<K, V> {
    pub fn get(&self, key: &K) -> Option<&Fact<V>> {
        self.facts.get(key)
    }

    pub fn insert(&mut self, key: K, fact: Fact<V>) -> Option<Fact<V>> {
        self.facts.insert(key, fact)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &Fact<V>)> {
        self.facts.iter()
    }
}

impl<K: Ord + Clone, V: Eq + Clone> FlowState<K, V> {
    pub fn join(&self, other: &Self) -> Self {
        let mut merged = BTreeMap::new();
        for key in self.facts.keys().chain(other.facts.keys()) {
            let left = self.facts.get(key).unwrap_or(&Fact::Unknown);
            let right = other.facts.get(key).unwrap_or(&Fact::Unknown);
            let fact = left.join(right);
            if fact != Fact::Unknown {
                merged.insert(key.clone(), fact);
            }
        }
        Self { facts: merged }
    }
}

/// Deterministic control-flow graph interface used by the worklist solver.
pub trait DataflowGraph {
    fn entry(&self) -> BasicBlockId;
    fn blocks(&self) -> &[BasicBlockId];
    fn successors(&self, block: BasicBlockId) -> &[BasicBlockId];
}

/// Transfer function for one forward analysis.
pub trait Transfer<K, V> {
    fn apply(&mut self, block: BasicBlockId, input: &FlowState<K, V>) -> FlowState<K, V>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataflowResult<K, V> {
    pub entry_states: BTreeMap<BasicBlockId, FlowState<K, V>>,
    pub exit_states: BTreeMap<BasicBlockId, FlowState<K, V>>,
    pub iterations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisError {
    MissingEntry(BasicBlockId),
    UnknownSuccessor {
        from: BasicBlockId,
        to: BasicBlockId,
    },
    IterationLimit {
        limit: usize,
    },
    UnknownFunction(FunctionId),
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AnalysisError {}

pub fn solve_forward<G, T, K, V>(
    graph: &G,
    transfer: &mut T,
    initial: FlowState<K, V>,
    iteration_limit: usize,
) -> Result<DataflowResult<K, V>, AnalysisError>
where
    G: DataflowGraph,
    T: Transfer<K, V>,
    K: Ord + Clone,
    V: Eq + Clone,
{
    let block_set = graph.blocks().iter().copied().collect::<BTreeSet<_>>();
    if !block_set.contains(&graph.entry()) {
        return Err(AnalysisError::MissingEntry(graph.entry()));
    }
    for from in graph.blocks() {
        for to in graph.successors(*from) {
            if !block_set.contains(to) {
                return Err(AnalysisError::UnknownSuccessor {
                    from: *from,
                    to: *to,
                });
            }
        }
    }

    let mut entry_states = BTreeMap::new();
    entry_states.insert(graph.entry(), initial);
    let mut exit_states = BTreeMap::new();
    let mut queue = VecDeque::from([graph.entry()]);
    let mut queued = BTreeSet::from([graph.entry()]);
    let mut iterations = 0;

    while let Some(block) = queue.pop_front() {
        queued.remove(&block);
        iterations += 1;
        if iterations > iteration_limit {
            return Err(AnalysisError::IterationLimit {
                limit: iteration_limit,
            });
        }
        let input = entry_states.get(&block).cloned().unwrap_or_default();
        let output = transfer.apply(block, &input);
        if exit_states.get(&block) == Some(&output) {
            continue;
        }
        exit_states.insert(block, output.clone());

        for successor in graph.successors(block) {
            let merged = match entry_states.get(successor) {
                Some(current) => current.join(&output),
                None => output.clone(),
            };
            let changed = entry_states.get(successor) != Some(&merged);
            if changed {
                entry_states.insert(*successor, merged);
                if queued.insert(*successor) {
                    queue.push_back(*successor);
                }
            }
        }
    }

    Ok(DataflowResult {
        entry_states,
        exit_states,
        iterations,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSummary<V> {
    pub return_fact: Fact<V>,
    pub may_throw: bool,
    pub may_suspend: bool,
    pub may_call_user: bool,
}

impl<V> Default for FunctionSummary<V> {
    fn default() -> Self {
        Self {
            return_fact: Fact::Unknown,
            may_throw: true,
            may_suspend: true,
            may_call_user: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WholeProgram<V> {
    callees: BTreeMap<FunctionId, BTreeSet<FunctionId>>,
    callers: BTreeMap<FunctionId, BTreeSet<FunctionId>>,
    summaries: BTreeMap<FunctionId, FunctionSummary<V>>,
}

impl<V: Clone + Eq> WholeProgram<V> {
    pub fn new(unit: &CompilationUnit) -> Self {
        let mut callees = BTreeMap::new();
        let mut callers = BTreeMap::new();
        let mut summaries = BTreeMap::new();
        for function in &unit.functions {
            callees.insert(function.id, BTreeSet::new());
            callers.insert(function.id, BTreeSet::new());
            summaries.insert(function.id, FunctionSummary::default());
        }
        Self {
            callees,
            callers,
            summaries,
        }
    }

    pub fn add_call(
        &mut self,
        caller: FunctionId,
        callee: FunctionId,
    ) -> Result<(), AnalysisError> {
        if !self.callees.contains_key(&caller) {
            return Err(AnalysisError::UnknownFunction(caller));
        }
        if !self.callees.contains_key(&callee) {
            return Err(AnalysisError::UnknownFunction(callee));
        }
        self.callees.get_mut(&caller).unwrap().insert(callee);
        self.callers.get_mut(&callee).unwrap().insert(caller);
        Ok(())
    }

    pub fn summary(&self, function: FunctionId) -> Option<&FunctionSummary<V>> {
        self.summaries.get(&function)
    }

    /// Recomputes summaries until callers stop changing. The callback may use
    /// current callee summaries; recursive components converge through the
    /// deterministic worklist.
    pub fn solve(
        &mut self,
        iteration_limit: usize,
        mut derive: impl FnMut(
            FunctionId,
            &BTreeSet<FunctionId>,
            &BTreeMap<FunctionId, FunctionSummary<V>>,
        ) -> FunctionSummary<V>,
    ) -> Result<usize, AnalysisError> {
        let mut queue = self.callees.keys().copied().collect::<VecDeque<_>>();
        let mut queued = self.callees.keys().copied().collect::<BTreeSet<_>>();
        let mut iterations = 0;
        while let Some(function) = queue.pop_front() {
            queued.remove(&function);
            iterations += 1;
            if iterations > iteration_limit {
                return Err(AnalysisError::IterationLimit {
                    limit: iteration_limit,
                });
            }
            let next = derive(function, &self.callees[&function], &self.summaries);
            if self.summaries.get(&function) == Some(&next) {
                continue;
            }
            self.summaries.insert(function, next);
            for caller in &self.callers[&function] {
                if queued.insert(*caller) {
                    queue.push_back(*caller);
                }
            }
        }
        Ok(iterations)
    }
}
