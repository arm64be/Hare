use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RegistrationState {
    Pending,
    Completed,
    Cancelled,
    Closed,
}

pub struct OneShotRegistration<T> {
    state: AtomicU8,
    payload: Mutex<Option<T>>,
}

impl<T> OneShotRegistration<T> {
    pub fn new(payload: T) -> Self {
        Self {
            state: AtomicU8::new(RegistrationState::Pending as u8),
            payload: Mutex::new(Some(payload)),
        }
    }

    pub fn state(&self) -> RegistrationState {
        match self.state.load(Ordering::Acquire) {
            value if value == RegistrationState::Pending as u8 => RegistrationState::Pending,
            value if value == RegistrationState::Completed as u8 => RegistrationState::Completed,
            value if value == RegistrationState::Cancelled as u8 => RegistrationState::Cancelled,
            value if value == RegistrationState::Closed as u8 => RegistrationState::Closed,
            _ => unreachable!("registration state is private"),
        }
    }

    pub fn complete(&self) -> Option<T> {
        self.claim(RegistrationState::Completed)
    }

    pub fn cancel(&self) -> Option<T> {
        self.claim(RegistrationState::Cancelled)
    }

    pub fn close(&self) -> Option<T> {
        self.claim(RegistrationState::Closed)
    }

    fn claim(&self, terminal: RegistrationState) -> Option<T> {
        if self
            .state
            .compare_exchange(
                RegistrationState::Pending as u8,
                terminal as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return None;
        }
        self.payload
            .lock()
            .expect("one-shot payload mutex poisoned")
            .take()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferState {
    Validating,
    Committed,
    Enqueued,
    Delivered,
    Dropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferError {
    InvalidTransition {
        from: TransferState,
        operation: &'static str,
    },
}

impl fmt::Display for TransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TransferError {}

pub struct TransferToken<T> {
    state: TransferState,
    backing_store: Option<T>,
    sender_detached: bool,
}

impl<T> TransferToken<T> {
    pub fn validating(backing_store: T) -> Self {
        Self {
            state: TransferState::Validating,
            backing_store: Some(backing_store),
            sender_detached: false,
        }
    }

    pub const fn state(&self) -> TransferState {
        self.state
    }

    pub const fn sender_detached(&self) -> bool {
        self.sender_detached
    }

    pub fn validation_failed(mut self) -> Result<T, TransferError> {
        self.require(TransferState::Validating, "validation_failed")?;
        Ok(self
            .backing_store
            .take()
            .expect("validating token owns bytes"))
    }

    pub fn commit(&mut self) -> Result<(), TransferError> {
        self.require(TransferState::Validating, "commit")?;
        self.sender_detached = true;
        self.state = TransferState::Committed;
        Ok(())
    }

    pub fn enqueue(&mut self, accepted: bool) -> Result<(), TransferError> {
        self.require(TransferState::Committed, "enqueue")?;
        if accepted {
            self.state = TransferState::Enqueued;
        } else {
            self.backing_store.take();
            self.state = TransferState::Dropped;
        }
        Ok(())
    }

    pub fn deliver(&mut self) -> Result<T, TransferError> {
        self.require(TransferState::Enqueued, "deliver")?;
        self.state = TransferState::Delivered;
        Ok(self
            .backing_store
            .take()
            .expect("enqueued token owns bytes"))
    }

    pub fn drop_delivery(&mut self) -> Result<(), TransferError> {
        self.require(TransferState::Enqueued, "drop_delivery")?;
        self.backing_store.take();
        self.state = TransferState::Dropped;
        Ok(())
    }

    fn require(
        &self,
        expected: TransferState,
        operation: &'static str,
    ) -> Result<(), TransferError> {
        if self.state == expected {
            Ok(())
        } else {
            Err(TransferError::InvalidTransition {
                from: self.state,
                operation,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewIdentity {
    pub view: u64,
    pub backing_store: u64,
    pub byte_offset: usize,
    pub byte_length: usize,
}

pub const fn yield_view(identity: ViewIdentity) -> ViewIdentity {
    identity
}
