use thiserror::Error;

#[derive(Error, Debug)]
pub enum FhirSchemaError {
    #[error("Conversion error: {message}")]
    Conversion { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Load error: {message}")]
    Load { message: String },

    #[error("Storage error: {message}")]
    Storage { message: String },

    #[error("Download error: {message}")]
    Download { message: String },

    #[error("Parsing error: {message}")]
    Parsing { message: String },

    #[error("Dependency error: {message}")]
    Dependency { message: String },

    #[error("Initialization error: {message}")]
    Initialization { message: String },

    #[error("Search error: {message}")]
    Search { message: String },

    #[error("Concurrency error: {message}")]
    Concurrency { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
}

#[derive(Error, Debug)]
#[error("Conversion error: {message}")]
pub struct ConversionError {
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ConversionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

#[derive(Error, Debug)]
#[error("Validation error: {message}")]
pub struct ValidationError {
    pub message: String,
    pub path: Option<String>,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: None,
            source: None,
        }
    }

    pub fn with_path(message: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: Some(path.into()),
            source: None,
        }
    }
}

#[derive(Error, Debug)]
#[error("Load error: {message}")]
pub struct LoadError {
    pub message: String,
    pub url: Option<String>,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl LoadError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            url: None,
            source: None,
        }
    }

    pub fn with_url(message: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            url: Some(url.into()),
            source: None,
        }
    }
}

pub type Result<T> = std::result::Result<T, FhirSchemaError>;
