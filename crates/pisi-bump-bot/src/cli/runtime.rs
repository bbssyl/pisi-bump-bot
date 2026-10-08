use crate::archive_download::{DownloadResult, download_sha1};
use crate::error::{ArchiveDownloadError, FetchError};
use crate::github_upstream::Fetch;
use crate::http_client::{fetch, stream_sha1};
use crate::package_checker::HashArchive;
use crate::prepare_runner::Downloader;

pub struct Runtime {
    pub fetch: Fetch,
    pub hash_archive: Box<HashArchive<'static>>,
    pub token: Option<String>,
    pub download: Box<Downloader<'static>>,
}

impl Runtime {
    pub fn new(
        fetch: Fetch,
        hash_archive: Box<HashArchive<'static>>,
        token: Option<String>,
        download: Box<Downloader<'static>>,
    ) -> Self {
        Self {
            fetch,
            hash_archive,
            token,
            download,
        }
    }

    pub fn production(token: Option<String>) -> Self {
        Self {
            fetch: Box::new(fetch),
            hash_archive: Box::new(stream_sha1) as Box<dyn Fn(&str) -> Result<String, FetchError>>,
            token,
            download: Box::new(download_sha1)
                as Box<dyn Fn(&str) -> Result<DownloadResult, ArchiveDownloadError>>,
        }
    }
}
