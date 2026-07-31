use crate::TraceRecipe;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FiberId {
    pub start_time_millis: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Cause<E, D> {
    Empty,
    Fail(E),
    Die(D),
    Interrupt(FiberId),
    Sequential(Box<Self>, Box<Self>),
    Parallel(Box<Self>, Box<Self>),
}

impl<E, D> Cause<E, D> {
    pub fn sequential(left: Self, right: Self) -> Self {
        Self::Sequential(Box::new(left), Box::new(right))
    }

    pub fn parallel(left: Self, right: Self) -> Self {
        Self::Parallel(Box::new(left), Box::new(right))
    }

    pub fn leaf_count(&self) -> usize {
        let mut count = 0;
        let mut pending = vec![self];
        while let Some(cause) = pending.pop() {
            match cause {
                Self::Empty => {}
                Self::Fail(_) | Self::Die(_) | Self::Interrupt(_) => count += 1,
                Self::Sequential(left, right) | Self::Parallel(left, right) => {
                    pending.push(right);
                    pending.push(left);
                }
            }
        }
        count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectExit<T, E, D> {
    Success(T),
    Failure(Cause<E, D>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionExit<T, E, D> {
    Success(T),
    Failure(E),
    Defect(D),
    Interrupt(FiberId),
    Retry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MicroCause<E, D> {
    Fail { error: E, trace: TraceRecipe },
    Die { defect: D, trace: TraceRecipe },
    Interrupt { trace: TraceRecipe },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MicroExit<T, E, D> {
    Success(T),
    Failure(MicroCause<E, D>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFailure<E, D, C = ()> {
    pub cause: Cause<E, D>,
    pub trace: TraceRecipe,
    pub captured_data: C,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeResult<T, E, D, C = ()> {
    Success(T),
    Failure(NativeFailure<E, D, C>),
}

impl<T, E, D, C> NativeResult<T, E, D, C> {
    pub const fn success(value: T) -> Self {
        Self::Success(value)
    }

    pub const fn failure(failure: NativeFailure<E, D, C>) -> Self {
        Self::Failure(failure)
    }

    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    pub fn as_failure(&self) -> Option<&NativeFailure<E, D, C>> {
        match self {
            Self::Success(_) => None,
            Self::Failure(failure) => Some(failure),
        }
    }
}
