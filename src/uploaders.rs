use anyhow::Result;
use reqwest::{blocking::Client, header};

use reqwest::blocking::Body;
use std::env;
use std::fs::{self, File};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Uploader {
    /// For production usage, uploads and redirects to s3.
    /// For test usage with `TestApp::with_proxy()`, the recording proxy is used.
    S3 {
        bucket: Box<s3::Bucket>,
        index_bucket: Option<Box<s3::Bucket>>,
        cdn: Option<String>,
    },

    /// For development usage only: "uploads" crate files to `dist` and serves them
    /// from there as well to enable local publishing and download
    Local,
}

pub enum UploadBucket {
    Default,
    Index,
}

impl Uploader {
    /// Returns the URL of an uploaded crate's version archive.
    ///
    /// The function doesn't check for the existence of the file.
    pub fn crate_location(&self, crate_name: &str, version: &str) -> String {
        let version = version.replace('+', "%2B");

        match *self {
            Uploader::S3 {
                ref bucket,
                ref cdn,
                ..
            } => {
                let path = Uploader::crate_path(crate_name, &version);
                match *cdn {
                    Some(ref host) => format!("https://{host}/{path}"),
                    None => bucket.url(&path).unwrap(),
                }
            }
            Uploader::Local => format!("/{}", Uploader::crate_path(crate_name, &version)),
        }
    }

    /// Returns the URL of an uploaded crate's version readme.
    ///
    /// The function doesn't check for the existence of the file.
    pub fn readme_location(&self, crate_name: &str, version: &str) -> String {
        let version = version.replace('+', "%2B");

        match *self {
            Uploader::S3 {
                ref bucket,
                ref cdn,
                ..
            } => {
                let path = Uploader::readme_path(crate_name, &version);
                match *cdn {
                    Some(ref host) => format!("https://{host}/{path}"),
                    None => bucket.url(&path).unwrap(),
                }
            }
            Uploader::Local => format!("/{}", Uploader::readme_path(crate_name, &version)),
        }
    }

    /// Returns the internal path of an uploaded crate's version archive.
    pub fn crate_path(name: &str, version: &str) -> String {
        format!("crates/{name}/{name}-{version}.crate")
    }

    /// Returns the internal path of an uploaded crate's version readme.
    pub fn readme_path(name: &str, version: &str) -> String {
        format!("readmes/{name}/{name}-{version}.html")
    }

    /// Returns the absolute path to the locally uploaded file.
    fn local_uploads_path(path: &str, upload_bucket: UploadBucket) -> PathBuf {
        let path = match upload_bucket {
            UploadBucket::Index => PathBuf::from("index").join(path),
            UploadBucket::Default => PathBuf::from(path),
        };
        env::current_dir().unwrap().join("local_uploads").join(path)
    }

    /// Uploads a file using the configured uploader (either `S3`, `Local`).
    ///
    /// It returns the path of the uploaded file.
    ///
    /// # Panics
    ///
    /// This function can panic on an `Self::Local` during development.
    /// Production and tests use `Self::S3` which should not panic.
    #[instrument(skip_all, fields(%path))]
    pub fn upload<R: Into<Body>>(
        &self,
        client: &Client,
        path: &str,
        content: R,
        content_type: &str,
        extra_headers: header::HeaderMap,
        upload_bucket: UploadBucket,
    ) -> Result<Option<String>> {
        match *self {
            Uploader::S3 {
                ref bucket,
                ref index_bucket,
                ..
            } => {
                let bucket = match upload_bucket {
                    UploadBucket::Default => Some(bucket),
                    UploadBucket::Index => index_bucket.as_ref(),
                };

                if let Some(bucket) = bucket {
                    bucket.put(client, path, content, content_type, extra_headers)?;
                }

                Ok(Some(String::from(path)))
            }
            Uploader::Local => {
                let filename = Self::local_uploads_path(path, upload_bucket);
                let dir = filename.parent().unwrap();
                fs::create_dir_all(dir)?;
                let mut file = File::create(&filename)?;
                let mut body = content.into();
                let mut buffer = body.buffer()?;
                std::io::copy(&mut buffer, &mut file)?;
                Ok(filename.to_str().map(String::from))
            }
        }
    }
}
