/// Stable carrier at the Hare native ABI boundary.
///
/// The bits are runtime-owned; optimizer passes may only inspect them through
/// explicit representation operations and proven tag predicates.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaggedValue(u64);

impl TaggedValue {
    pub const fn from_abi_bits(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn abi_bits(self) -> u64 {
        self.0
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionTag {
    Normal,
    Throw,
    Suspend,
    Cancel,
    Terminate,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaggedCompletion {
    pub tag: CompletionTag,
    pub detail: u32,
    pub value: TaggedValue,
}

impl TaggedCompletion {
    pub const fn normal(value: TaggedValue) -> Self {
        Self {
            tag: CompletionTag::Normal,
            detail: 0,
            value,
        }
    }
}
