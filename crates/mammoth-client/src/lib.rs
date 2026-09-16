//! Remote Backend over the versioned HTTP filesystem API.
//!
//! This connects to a local gateway; it is not the planned direct-worker gRPC client.
#![forbid(unsafe_code)]
use base64::Engine;
use futures_util::StreamExt;
use mammoth_core::{
    backend::ByteStream, Backend, BlockPlacement, ClusterReport, Error, FileStatus, Result,
};
use reqwest::{Client, Method, Url};
use serde::de::DeserializeOwned;
use std::{ops::Range, path::Path};

#[derive(Clone)]
pub struct ClusterBackend {
    client: Client,
    base: Url,
}
impl ClusterBackend {
    pub fn connect(endpoint: &str) -> Result<Self> {
        let endpoint = if endpoint.contains("://") {
            endpoint.to_string()
        } else {
            format!("http://{endpoint}")
        };
        let base = Url::parse(&endpoint).map_err(|e| Error::Config(e.to_string()))?;
        if !["http", "https"].contains(&base.scheme()) {
            return Err(Error::Config("gateway endpoint must use HTTP or HTTPS".into()));
        }
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(3600))
            .build()
            .map_err(transport)?;
        Ok(Self { client, base })
    }
    fn url(&self, op: &str, args: &[(&str, String)]) -> Url {
        let mut url = self.base.clone();
        url.set_path(&format!("/api/v1/{op}"));
        url.query_pairs_mut().extend_pairs(args.iter().map(|(k, v)| (*k, v)));
        url
    }
    async fn request(
        &self,
        method: Method,
        op: &str,
        args: &[(&str, String)],
        body: Option<ByteStream>,
    ) -> Result<reqwest::Response> {
        let mut request = self.client.request(method, self.url(op, args));
        if let Some(body) = body {
            request = request.body(reqwest::Body::wrap_stream(body));
        }
        let response = request.send().await.map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.text().await.map_err(transport)?;
        let parsed = serde_json::from_str::<serde_json::Value>(&body).ok();
        let code = parsed.as_ref().and_then(|v| v["code"].as_str()).unwrap_or("E0500");
        let message = parsed.as_ref().and_then(|v| v["message"].as_str()).unwrap_or(&body);
        Err(Error::Remote { code: code.into(), message: message.into(), status: status.as_u16() })
    }

    async fn get<T: DeserializeOwned>(&self, op: &str, args: &[(&str, String)]) -> Result<T> {
        self.request(Method::GET, op, args, None).await?.json().await.map_err(transport)
    }
    async fn modify(&self, method: Method, op: &str, args: &[(&str, String)]) -> Result<()> {
        self.request(method, op, args, None).await?;
        Ok(())
    }
}
fn transport(e: reqwest::Error) -> Error {
    Error::Io(std::io::Error::other(e))
}
fn path(p: &Path) -> Result<String> {
    p.to_str().map(String::from).ok_or_else(|| Error::InvalidInput("path must be UTF-8".into()))
}
#[async_trait::async_trait]
impl Backend for ClusterBackend {
    async fn list(&self, p: &Path) -> Result<Vec<FileStatus>> {
        self.get("core/list", &[("path", path(p)?)]).await
    }
    async fn stat(&self, p: &Path) -> Result<FileStatus> {
        self.get("core/stat", &[("path", path(p)?)]).await
    }
    async fn block_layout(&self, p: &Path) -> Result<Vec<BlockPlacement>> {
        self.get("core/blocks", &[("path", path(p)?)]).await
    }
    async fn cluster_report(&self) -> Result<ClusterReport> {
        self.get("core/report", &[]).await
    }
    async fn read(&self, p: &Path, r: Range<u64>) -> Result<ByteStream> {
        let response = self
            .request(
                Method::GET,
                "fs/data",
                &[("path", path(p)?), ("start", r.start.to_string()), ("end", r.end.to_string())],
                None,
            )
            .await?;
        Ok(Box::pin(response.bytes_stream().map(|r| r.map_err(transport))))
    }
    async fn write(&self, p: &Path, data: ByteStream) -> Result<()> {
        self.request(Method::PUT, "fs/data", &[("path", path(p)?)], Some(data)).await?;
        Ok(())
    }
    async fn create(&self, p: &Path, data: ByteStream) -> Result<()> {
        self.request(
            Method::PUT,
            "fs/data",
            &[("path", path(p)?), ("create", "true".into())],
            Some(data),
        )
        .await?;
        Ok(())
    }
    async fn remove(&self, p: &Path, recursive: bool) -> Result<()> {
        self.modify(
            Method::DELETE,
            "fs",
            &[("path", path(p)?), ("recursive", recursive.to_string())],
        )
        .await
    }
    async fn mkdir(&self, p: &Path, parents: bool) -> Result<()> {
        self.modify(
            Method::PUT,
            "fs/directory",
            &[("path", path(p)?), ("parents", parents.to_string())],
        )
        .await
    }
    async fn rename(&self, p: &Path, to: &Path) -> Result<()> {
        self.modify(Method::POST, "fs/rename", &[("path", path(p)?), ("to", path(to)?)]).await
    }
    async fn set_attributes(
        &self,
        p: &Path,
        mode: Option<u32>,
        owner: Option<String>,
        group: Option<String>,
    ) -> Result<()> {
        let mut args = vec![("path", path(p)?)];
        if let Some(m) = mode {
            args.push(("mode", m.to_string()));
        }
        if let Some(o) = owner {
            args.push(("owner", o));
        }
        if let Some(g) = group {
            args.push(("group", g));
        }
        self.modify(Method::POST, "fs/attributes", &args).await
    }
    async fn set_replication(&self, p: &Path, n: u8) -> Result<()> {
        self.modify(
            Method::POST,
            "fs/replication",
            &[("path", path(p)?), ("replication", n.to_string())],
        )
        .await
    }
    async fn repair(&self) -> Result<u64> {
        let r: serde_json::Value = self
            .request(Method::POST, "admin/repair", &[], None)
            .await?
            .json()
            .await
            .map_err(transport)?;
        r["repaired"].as_u64().ok_or_else(|| Error::InvalidInput("invalid repair response".into()))
    }
    async fn open_read(
        &self,
        p: &Path,
        r: Range<u64>,
    ) -> Result<mammoth_core::backend::ReadSnapshot> {
        #[derive(serde::Deserialize)]
        struct Info {
            status: FileStatus,
            etag: String,
            range: Range<u64>,
        }
        let response = self
            .request(
                Method::GET,
                "fs/data",
                &[("path", path(p)?), ("start", r.start.to_string()), ("end", r.end.to_string())],
                None,
            )
            .await?;
        let header = response
            .headers()
            .get("x-mammoth-snapshot")
            .ok_or_else(|| Error::InvalidInput("gateway did not return a read snapshot".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(header.as_bytes())
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        let info: Info =
            serde_json::from_slice(&bytes).map_err(|e| Error::InvalidInput(e.to_string()))?;
        Ok(mammoth_core::backend::ReadSnapshot {
            status: info.status,
            etag: info.etag,
            range: info.range,
            data: Box::pin(response.bytes_stream().map(|r| r.map_err(transport))),
        })
    }
    async fn etag(&self, p: &Path) -> Result<String> {
        self.get("core/etag", &[("path", path(p)?)]).await
    }
    async fn gc(&self) -> Result<u64> {
        let r: serde_json::Value = self
            .request(Method::POST, "admin/gc", &[], None)
            .await?
            .json()
            .await
            .map_err(transport)?;
        r["removed"].as_u64().ok_or_else(|| Error::InvalidInput("invalid gc response".into()))
    }
}
