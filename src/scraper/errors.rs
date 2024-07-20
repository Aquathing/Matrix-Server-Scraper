#[derive(Debug)]
pub enum SearchError {
    DbError,
    RequestError,
    ParseError,
    CantFindServer,
}

impl From<reqwest::Error> for SearchError {
    fn from(value: reqwest::Error) -> Self {
        Self::RequestError
    }
}

impl From<deadpool_postgres::PoolError> for SearchError {
    fn from(value: deadpool_postgres::PoolError) -> Self {
        Self::DbError
    }
}

impl From<tokio_postgres::Error> for SearchError {
    fn from(value: tokio_postgres::Error) -> Self {
        Self::DbError
    }
}

impl From<serde_json::Error> for SearchError {
    fn from(value: serde_json::Error) -> Self {
        Self::ParseError
    }
}
