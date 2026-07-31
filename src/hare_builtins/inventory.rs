use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const INVENTORY_TSV: &str = include_str!("../../generated/hare/builtins-inventory.tsv");
const HEADER: [&str; 7] = [
    "id", "kind", "source", "symbol", "line", "sha256", "strategy",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FeatureKind {
    InternalModule,
    NativeModule,
    BuiltinFunction,
    RuntimeSource,
    NativeBridge,
    BuildTimeEval,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryStrategy {
    GenericNative,
    RuntimeCapability,
    NativeImplementation,
    BuildTimeOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureEntry {
    pub id: &'static str,
    pub kind: FeatureKind,
    pub source: &'static str,
    pub symbol: &'static str,
    pub line: u32,
    pub sha256: &'static str,
    pub strategy: InventoryStrategy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeatureInventoryError {
    Header,
    FieldCount {
        line: usize,
        count: usize,
    },
    UnknownKind {
        line: usize,
        kind: &'static str,
    },
    UnknownStrategy {
        line: usize,
        strategy: &'static str,
    },
    InvalidLine {
        line: usize,
    },
    InvalidSha256 {
        line: usize,
    },
    DuplicateId(&'static str),
    DuplicateEntry {
        source: &'static str,
        symbol: &'static str,
        line: u32,
    },
    Count {
        kind: FeatureKind,
        expected: usize,
        actual: usize,
    },
    StrategyMismatch(&'static str),
}

impl fmt::Display for FeatureInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FeatureInventoryError {}

pub fn feature_inventory() -> Result<Vec<FeatureEntry>, FeatureInventoryError> {
    let mut lines = INVENTORY_TSV.lines();
    if lines
        .next()
        .is_none_or(|header| header.split('\t').ne(HEADER))
    {
        return Err(FeatureInventoryError::Header);
    }
    let mut entries = Vec::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != HEADER.len() {
            return Err(FeatureInventoryError::FieldCount {
                line: line_number,
                count: fields.len(),
            });
        }
        let kind = match fields[1] {
            "internal_module" => FeatureKind::InternalModule,
            "native_module" => FeatureKind::NativeModule,
            "builtin_function" => FeatureKind::BuiltinFunction,
            "runtime_source" => FeatureKind::RuntimeSource,
            "native_bridge" => FeatureKind::NativeBridge,
            "build_time_eval" => FeatureKind::BuildTimeEval,
            kind => {
                return Err(FeatureInventoryError::UnknownKind {
                    line: line_number,
                    kind,
                });
            }
        };
        let strategy = match fields[6] {
            "generic_native" => InventoryStrategy::GenericNative,
            "runtime_capability" => InventoryStrategy::RuntimeCapability,
            "native_implementation" => InventoryStrategy::NativeImplementation,
            "build_time_only" => InventoryStrategy::BuildTimeOnly,
            strategy => {
                return Err(FeatureInventoryError::UnknownStrategy {
                    line: line_number,
                    strategy,
                });
            }
        };
        let parsed_line = fields[4]
            .parse()
            .map_err(|_| FeatureInventoryError::InvalidLine { line: line_number })?;
        if fields[5].len() != 64 || !fields[5].bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(FeatureInventoryError::InvalidSha256 { line: line_number });
        }
        entries.push(FeatureEntry {
            id: fields[0],
            kind,
            source: fields[2],
            symbol: fields[3],
            line: parsed_line,
            sha256: fields[5],
            strategy,
        });
    }
    validate_feature_inventory(&entries)?;
    Ok(entries)
}

pub fn validate_feature_inventory(entries: &[FeatureEntry]) -> Result<(), FeatureInventoryError> {
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for entry in entries {
        if !ids.insert(entry.id) {
            return Err(FeatureInventoryError::DuplicateId(entry.id));
        }
        if !keys.insert((entry.source, entry.symbol, entry.line)) {
            return Err(FeatureInventoryError::DuplicateEntry {
                source: entry.source,
                symbol: entry.symbol,
                line: entry.line,
            });
        }
        *counts.entry(entry.kind).or_insert(0) += 1;
        let expected = match entry.kind {
            FeatureKind::InternalModule | FeatureKind::BuiltinFunction => {
                InventoryStrategy::GenericNative
            }
            FeatureKind::NativeModule | FeatureKind::NativeBridge => {
                InventoryStrategy::RuntimeCapability
            }
            FeatureKind::RuntimeSource => InventoryStrategy::NativeImplementation,
            FeatureKind::BuildTimeEval => InventoryStrategy::BuildTimeOnly,
        };
        if entry.strategy != expected {
            return Err(FeatureInventoryError::StrategyMismatch(entry.id));
        }
    }
    for (kind, expected) in [
        (FeatureKind::InternalModule, 193),
        (FeatureKind::NativeModule, 13),
        (FeatureKind::BuiltinFunction, 90),
        (FeatureKind::RuntimeSource, 616),
        (FeatureKind::NativeBridge, 84),
        (FeatureKind::BuildTimeEval, 3),
    ] {
        let actual = counts.get(&kind).copied().unwrap_or(0);
        if actual != expected {
            return Err(FeatureInventoryError::Count {
                kind,
                expected,
                actual,
            });
        }
    }
    Ok(())
}
