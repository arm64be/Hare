use std::fmt;

use crate::{NativeFailure, NativeResult};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ErrorId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalFrameId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DataSchemaId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceRecipe {
    pub error: ErrorId,
    pub source: SourceId,
    pub logical_frames: &'static [LogicalFrameId],
    pub data_schema: DataSchemaId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticTraceTables<'a> {
    pub errors: &'a [&'a str],
    pub sources: &'a [&'a str],
    pub logical_frames: &'a [&'a str],
    pub data_schema_count: u32,
}

pub trait DataMaterializer<C> {
    type Materialized;

    fn materialize(&self, schema: DataSchemaId, captured: &C) -> Self::Materialized;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedFailure<'a, M> {
    pub error: &'a str,
    pub source: &'a str,
    pub logical_frames: Vec<&'a str>,
    pub data: M,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceError {
    ErrorId(ErrorId),
    SourceId(SourceId),
    LogicalFrameId(LogicalFrameId),
    DataSchemaId(DataSchemaId),
}

impl fmt::Display for TraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TraceError {}

pub fn materialize_failure<'a, E, D, C, M>(
    failure: &NativeFailure<E, D, C>,
    tables: &StaticTraceTables<'a>,
    materializer: &M,
) -> Result<MaterializedFailure<'a, M::Materialized>, TraceError>
where
    M: DataMaterializer<C>,
{
    let recipe = failure.trace;
    let error = tables
        .errors
        .get(recipe.error.0 as usize)
        .copied()
        .ok_or(TraceError::ErrorId(recipe.error))?;
    let source = tables
        .sources
        .get(recipe.source.0 as usize)
        .copied()
        .ok_or(TraceError::SourceId(recipe.source))?;
    if recipe.data_schema.0 >= tables.data_schema_count {
        return Err(TraceError::DataSchemaId(recipe.data_schema));
    }
    let logical_frames = recipe
        .logical_frames
        .iter()
        .map(|frame| {
            tables
                .logical_frames
                .get(frame.0 as usize)
                .copied()
                .ok_or(TraceError::LogicalFrameId(*frame))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let data = materializer.materialize(recipe.data_schema, &failure.captured_data);
    Ok(MaterializedFailure {
        error,
        source,
        logical_frames,
        data,
    })
}

pub fn materialize_result_failure<'a, T, E, D, C, M>(
    result: &NativeResult<T, E, D, C>,
    tables: &StaticTraceTables<'a>,
    materializer: &M,
) -> Option<Result<MaterializedFailure<'a, M::Materialized>, TraceError>>
where
    M: DataMaterializer<C>,
{
    result
        .as_failure()
        .map(|failure| materialize_failure(failure, tables, materializer))
}
