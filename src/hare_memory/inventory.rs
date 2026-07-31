use std::collections::BTreeSet;
use std::fmt;

const LIFETIMES_TSV: &str = include_str!("../../LIFETIMES.tsv");
const HEADER: [&str; 10] = [
    "id",
    "domain",
    "case",
    "owner",
    "root",
    "pin",
    "carrier",
    "exits",
    "status",
    "escalation",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceStatus {
    Proof,
    Guard,
    Hint,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifetimeCase {
    pub id: &'static str,
    pub domain: &'static str,
    pub case: &'static str,
    pub owner: &'static str,
    pub root: &'static str,
    pub pin: &'static str,
    pub carrier: &'static str,
    pub exits: Vec<&'static str>,
    pub status: EvidenceStatus,
    pub escalation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryError {
    Header,
    FieldCount { line: usize, count: usize },
    EmptyField { line: usize, field: usize },
    DuplicateId(&'static str),
    InvalidStatus { line: usize, status: &'static str },
    EmptyExit { line: usize },
    OpaqueContract { line: usize },
    RowCount(usize),
}

impl fmt::Display for InventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InventoryError {}

pub fn lifetime_inventory() -> Result<Vec<LifetimeCase>, InventoryError> {
    let mut lines = LIFETIMES_TSV.lines();
    let Some(header) = lines.next() else {
        return Err(InventoryError::Header);
    };
    if header.split('\t').ne(HEADER) {
        return Err(InventoryError::Header);
    }

    let mut ids = BTreeSet::new();
    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != HEADER.len() {
            return Err(InventoryError::FieldCount {
                line: line_number,
                count: fields.len(),
            });
        }
        if let Some(field) = fields.iter().position(|field| field.trim().is_empty()) {
            return Err(InventoryError::EmptyField {
                line: line_number,
                field,
            });
        }
        if fields
            .iter()
            .any(|field| field.to_ascii_lowercase().contains("runtime magic"))
        {
            return Err(InventoryError::OpaqueContract { line: line_number });
        }
        if !ids.insert(fields[0]) {
            return Err(InventoryError::DuplicateId(fields[0]));
        }
        let status = match fields[8] {
            "P" => EvidenceStatus::Proof,
            "G" => EvidenceStatus::Guard,
            "H" => EvidenceStatus::Hint,
            "U" => EvidenceStatus::Unknown,
            status => {
                return Err(InventoryError::InvalidStatus {
                    line: line_number,
                    status,
                });
            }
        };
        let exits = fields[7].split('|').collect::<Vec<_>>();
        if exits.iter().any(|exit| exit.is_empty()) {
            return Err(InventoryError::EmptyExit { line: line_number });
        }
        rows.push(LifetimeCase {
            id: fields[0],
            domain: fields[1],
            case: fields[2],
            owner: fields[3],
            root: fields[4],
            pin: fields[5],
            carrier: fields[6],
            exits,
            status,
            escalation: fields[9],
        });
    }
    if rows.len() != 50 {
        return Err(InventoryError::RowCount(rows.len()));
    }
    Ok(rows)
}
