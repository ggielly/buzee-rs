#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database: {0}")]
    Db(#[from] diesel::result::Error),
    #[error("connection pool: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("tantivy: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform: {0}")]
    Platform(String),
    #[error("lock poisoned")]
    LockPoisoned,
    #[error("job already running")]
    JobAlreadyRunning,
    #[error("job cancelled")]
    Cancelled,
    #[error("channel closed")]
    ChannelClosed,
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn new(message: &str) -> Self {
        Self::Other(message.to_string())
    }
}

impl Clone for AppError {
    fn clone(&self) -> Self {
        match self {
            Self::Db(e) => Self::Other(e.to_string()),
            Self::Pool(e) => Self::Other(e.to_string()),
            Self::Tantivy(e) => Self::Other(e.to_string()),
            Self::Io(e) => Self::Io(std::io::Error::new(e.kind(), e.to_string())),
            Self::Platform(s) => Self::Platform(s.clone()),
            Self::LockPoisoned => Self::LockPoisoned,
            Self::JobAlreadyRunning => Self::JobAlreadyRunning,
            Self::Cancelled => Self::Cancelled,
            Self::ChannelClosed => Self::ChannelClosed,
            Self::Other(s) => Self::Other(s.clone()),
        }
    }
}