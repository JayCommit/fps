use sqlx::error::BoxDynError;

pub fn column_decode(column: &str, source: impl Into<BoxDynError>) -> sqlx::Error {
    sqlx::Error::ColumnDecode {
        index: column.into(),
        source: source.into(),
    }
}

pub fn parse_id<T>(raw: &str, column: &'static str) -> Result<T, sqlx::Error>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    raw.parse().map_err(|e| column_decode(column, Box::new(e)))
}
