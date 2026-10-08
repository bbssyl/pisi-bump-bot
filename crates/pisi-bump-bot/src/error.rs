use std::io;

#[derive(Debug, thiserror::Error, Clone)]
pub enum FetchError {
    #[error("{url}: io hatası: {reason}")]
    Io { url: String, reason: String },
    #[error("{url}: zaman aşımı")]
    Timeout { url: String },
    #[error("{url}: bağlantı kurulamadı")]
    ConnectionFailed { url: String },
    #[error("{url}: sunucu adı çözümlenemedi")]
    HostNotFound { url: String },
    #[error("{url}: TLS hatası: {reason}")]
    Tls { url: String, reason: String },
    #[error("{url}: HTTP {status} yanıtı okunamadı")]
    ErrorBodyUnreadable { url: String, status: u16 },
    #[error("{url}: istek başarısız: {reason}")]
    Other { url: String, reason: String },
}

impl FetchError {
    pub fn from_transport(url: &str, error: &ureq::Error) -> Self {
        let url = url.to_string();
        match error {
            ureq::Error::Io(source) => Self::Io {
                url,
                reason: describe_io_error(source),
            },
            ureq::Error::Timeout(_) => Self::Timeout { url },
            ureq::Error::ConnectionFailed => Self::ConnectionFailed { url },
            ureq::Error::HostNotFound => Self::HostNotFound { url },
            ureq::Error::Tls(reason) => Self::Tls {
                url,
                reason: reason.to_string(),
            },
            other => Self::Other {
                url,
                reason: other.to_string(),
            },
        }
    }

    pub fn from_io(url: &str, source: &io::Error) -> Self {
        Self::Io {
            url: url.to_string(),
            reason: describe_io_error(source),
        }
    }
}

fn describe_io_error(source: &io::Error) -> String {
    format!("{}: {source}", io_error_kind_name(source.kind()))
}

fn io_error_kind_name(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::ConnectionReset => "ConnectionReset",
        io::ErrorKind::ConnectionRefused => "ConnectionRefused",
        io::ErrorKind::ConnectionAborted => "ConnectionAborted",
        io::ErrorKind::UnexpectedEof => "UnexpectedEof",
        io::ErrorKind::TimedOut => "TimedOut",
        _ => "IoError",
    }
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum UpstreamError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Fetch(#[from] FetchError),
}

impl UpstreamError {
    pub fn message(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[error("GitHub API oran sınırı aşıldı")]
pub struct RateLimitExceeded;

#[derive(Debug, thiserror::Error, Clone)]
pub enum GithubError {
    #[error(transparent)]
    Upstream(#[from] UpstreamError),
    #[error(transparent)]
    RateLimited(#[from] RateLimitExceeded),
}

#[derive(Debug, thiserror::Error, Clone)]
#[error("{message}")]
pub struct ArchiveDownloadError {
    pub message: String,
    pub transient: bool,
}

impl ArchiveDownloadError {
    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transient: true,
        }
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transient: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PspecUpdateError {
    #[error("pspec.xml okunamadı: {0}")]
    Parse(#[from] roxmltree::Error),
    #[error("History/Update satırı beklenen biçimde değil")]
    UnexpectedIndent,
    #[error("Archive öğesinde sha1sum özniteliği yok")]
    MissingSha1,
    #[error("eşleşen Archive öğesi bulunamadı")]
    NoMatchingArchive,
    #[error("History içinde Update bulunamadı")]
    NoHistoryUpdate,
    #[error("History/Update kapanmıyor")]
    UnclosedUpdate,
    #[error("yeni History kaydı doğrulanamadı")]
    HistoryNotValidated,
    #[error("Archive güncellemesi doğrulanamadı")]
    ArchiveNotValidated,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("durum dosyası okunamadı: {0}")]
    Io(#[from] io::Error),
    #[error("durum dosyası okunamadı: {0}")]
    Json(#[from] serde_json::Error),
    #[error("durum dosyası okunamadı: {0}")]
    Message(String),
}

#[derive(Debug, thiserror::Error, Clone)]
#[error("{message}")]
pub struct PrepareFailure {
    pub message: String,
    pub transient: bool,
}

impl PrepareFailure {
    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transient: true,
        }
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transient: false,
        }
    }
}

impl From<ArchiveDownloadError> for PrepareFailure {
    fn from(error: ArchiveDownloadError) -> Self {
        Self {
            message: error.message,
            transient: error.transient,
        }
    }
}

impl From<PspecUpdateError> for PrepareFailure {
    fn from(error: PspecUpdateError) -> Self {
        Self::permanent(error.to_string())
    }
}
