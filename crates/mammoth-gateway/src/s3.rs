//! Path-style S3 subset for local development: buckets, objects, listings and ranges.
use super::{walk, Gateway};
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use base64::Engine;
use futures_util::StreamExt;
use mammoth_core::{Backend, Error};
use md5::{Digest, Md5};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};

pub fn router(backend: Arc<dyn Backend>) -> Router {
    Router::new().fallback(any(handle)).with_state(Gateway { backend })
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn response(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/xml")], body).into_response()
}
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    response(
        status,
        format!(
            "<?xml version=\"1.0\"?><Error><Code>{code}</Code><Message>{}</Message></Error>",
            xml(message)
        ),
    )
}
fn backend_error(e: Error) -> Response {
    let (status, code) = match e {
        Error::NotFound(_) => (StatusCode::NOT_FOUND, "NoSuchKey"),
        Error::AlreadyExists(_) => (StatusCode::CONFLICT, "BucketAlreadyOwnedByYou"),
        Error::WrongKind { .. } => (StatusCode::CONFLICT, "BucketNotEmpty"),
        Error::ChecksumMismatch { .. } => (StatusCode::BAD_REQUEST, "BadDigest"),
        Error::InvalidInput(_) => (StatusCode::BAD_REQUEST, "InvalidArgument"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "InternalError"),
    };
    error(status, code, &e.to_string())
}
fn timestamp(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn valid_bucket(s: &str) -> bool {
    (3..=63).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        && s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && s.ends_with(|c: char| c.is_ascii_alphanumeric())
        && !s.contains("..")
}
/// Parse one HTTP bytes range into half-open offsets.
pub fn parse_range(value: &str, len: u64) -> Option<std::ops::Range<u64>> {
    let value = value.strip_prefix("bytes=")?;
    let (start, end) = value.split_once('-')?;
    if len == 0 {
        return None;
    }
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        if suffix == 0 {
            None
        } else {
            Some(len.saturating_sub(suffix)..len)
        }
    } else {
        let start = start.parse::<u64>().ok()?;
        let end =
            if end.is_empty() { len } else { end.parse::<u64>().ok()?.saturating_add(1).min(len) };
        if start >= end {
            None
        } else {
            Some(start..end)
        }
    }
}
async fn handle(
    State(state): State<Gateway>,
    Query(q): Query<BTreeMap<String, String>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let decoded = match percent_encoding::percent_decode_str(uri.path()).decode_utf8() {
        Ok(p) => p,
        Err(_) => return error(StatusCode::BAD_REQUEST, "InvalidURI", "Path must be UTF-8"),
    };
    if decoded.contains(['\0', '\\'])
        || decoded.split('/').any(|s| s == ".." || s == ".")
        || decoded.contains("//")
    {
        return error(StatusCode::BAD_REQUEST, "InvalidURI", "Non-canonical object key");
    }
    if headers
        .get("x-amz-content-sha256")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("STREAMING-"))
    {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "NotImplemented",
            "AWS chunked uploads are not supported; use an unsigned request",
        );
    }
    if q.keys().any(|k| {
        ["uploads", "uploadId", "partNumber", "versionId", "versions", "acl", "tagging", "delete"]
            .contains(&k.as_str())
    }) {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "NotImplemented",
            "Multipart, versions, ACLs, tagging and bulk delete are not supported",
        );
    }
    match operation(&state, &q, &method, &decoded, &headers, body).await {
        Ok(r) => r,
        Err(e) => backend_error(e),
    }
}
async fn operation(
    state: &Gateway,
    q: &BTreeMap<String, String>,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
    body: Body,
) -> mammoth_core::Result<Response> {
    let be = state.backend.as_ref();
    let raw = path.trim_start_matches('/');
    if raw.is_empty() {
        if method != Method::GET {
            return Ok(error(
                StatusCode::METHOD_NOT_ALLOWED,
                "MethodNotAllowed",
                "Use GET to list buckets",
            ));
        }
        let buckets = be
            .list(Path::new("/"))
            .await?
            .iter()
            .filter(|s| s.is_dir)
            .map(|s| {
                format!(
                    "<Bucket><Name>{}</Name><CreationDate>{}</CreationDate></Bucket>",
                    xml(&s.path.file_name().unwrap_or_default().to_string_lossy()),
                    timestamp(s.modified)
                )
            })
            .collect::<String>();
        return Ok(response(StatusCode::OK,format!("<?xml version=\"1.0\"?><ListAllMyBucketsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><Owner><ID>local</ID><DisplayName>local</DisplayName></Owner><Buckets>{buckets}</Buckets></ListAllMyBucketsResult>")));
    }
    let (bucket, key) = raw.split_once('/').unwrap_or((raw, ""));
    if !valid_bucket(bucket) {
        return Ok(error(
            StatusCode::BAD_REQUEST,
            "InvalidBucketName",
            "Bucket names must be 3–63 lowercase letters, digits, dots or hyphens",
        ));
    }
    let bucket_path = format!("/{bucket}");
    if path.ends_with('/') && !key.is_empty() {
        return Ok(error(
            StatusCode::BAD_REQUEST,
            "InvalidArgument",
            "Directory-marker objects are unsupported",
        ));
    }
    if method == Method::PUT
        && (headers.contains_key(header::IF_MATCH) || headers.contains_key(header::IF_NONE_MATCH))
    {
        return Ok(error(
            StatusCode::NOT_IMPLEMENTED,
            "NotImplemented",
            "Conditional writes are not supported",
        ));
    }
    if key.is_empty() {
        if method != Method::PUT && !be.stat(Path::new(&bucket_path)).await.is_ok_and(|s| s.is_dir)
        {
            return Ok(error(StatusCode::NOT_FOUND, "NoSuchBucket", "Bucket does not exist"));
        }
        match method.as_str() {
            "PUT" => {
                be.mkdir(Path::new(&bucket_path), false).await?;
                return Ok((StatusCode::OK, [(header::LOCATION, bucket_path)]).into_response());
            }
            "DELETE" => {
                be.remove(Path::new(&bucket_path), false).await?;
                return Ok(StatusCode::NO_CONTENT.into_response());
            }
            "HEAD" => {
                be.stat(Path::new(&bucket_path)).await?;
                return Ok(StatusCode::OK.into_response());
            }
            "GET" => {
                if q.contains_key("location") {
                    return Ok(response(
                        StatusCode::OK,
                        "<LocationConstraint xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"/>"
                            .into(),
                    ));
                }
                let prefix = q.get("prefix").map(String::as_str).unwrap_or("");
                let delimiter = q.get("delimiter").map(String::as_str).unwrap_or("");
                let marker = q
                    .get("continuation-token")
                    .or_else(|| q.get("start-after"))
                    .or_else(|| q.get("marker"))
                    .map(String::as_str)
                    .unwrap_or("");
                let max: usize = q
                    .get("max-keys")
                    .map(|v| v.parse::<usize>())
                    .transpose()
                    .map_err(|_| Error::InvalidInput("invalid max-keys".into()))?
                    .unwrap_or(1000)
                    .min(1000);
                let files = walk(be, Path::new(&bucket_path)).await?;
                let mut items = BTreeMap::new();
                let mut common = BTreeSet::new();
                for s in &files {
                    if s.is_dir {
                        continue;
                    }
                    let name = s.path.to_string_lossy();
                    let name = &name[bucket_path.len() + 1..];
                    if !name.starts_with(prefix) {
                        continue;
                    }
                    if !delimiter.is_empty() {
                        if let Some(index) = name[prefix.len()..].find(delimiter) {
                            let p = &name[..prefix.len() + index + delimiter.len()];
                            if p > marker {
                                common.insert(p.to_string());
                            }
                            continue;
                        }
                    }
                    if name > marker {
                        items.insert(name.to_string(), Some(s));
                    }
                }
                for p in common {
                    items.insert(p, None);
                }
                let truncated = items.len() > max && max > 0;
                let selected: Vec<_> = items.iter().take(max).collect();
                let encode = |s: &str| {
                    if q.get("encoding-type").is_some_and(|v| v == "url") {
                        percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC)
                            .to_string()
                    } else {
                        xml(s)
                    }
                };
                let mut content = String::new();
                for (name, s) in &selected {
                    match s { Some(s)=>content.push_str(&format!("<Contents><Key>{}</Key><LastModified>{}</LastModified><ETag>&quot;{}&quot;</ETag><Size>{}</Size><StorageClass>STANDARD</StorageClass></Contents>",encode(name),timestamp(s.modified),xml(&be.etag(&s.path).await?),s.len)),None=>content.push_str(&format!("<CommonPrefixes><Prefix>{}</Prefix></CommonPrefixes>",encode(name))) }
                }
                let next = if truncated {
                    let last = xml(selected.last().expect("nonzero max").0);
                    format!("<NextContinuationToken>{last}</NextContinuationToken><NextMarker>{last}</NextMarker>")
                } else {
                    String::new()
                };
                let encoding = if q.contains_key("encoding-type") {
                    "<EncodingType>url</EncodingType>"
                } else {
                    ""
                };
                return Ok(response(StatusCode::OK,format!("<?xml version=\"1.0\"?><ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><Name>{bucket}</Name><Prefix>{}</Prefix><Delimiter>{}</Delimiter><KeyCount>{}</KeyCount><MaxKeys>{max}</MaxKeys><IsTruncated>{truncated}</IsTruncated>{encoding}{next}{content}</ListBucketResult>",encode(prefix),encode(delimiter),selected.len())));
            }
            _ => {
                return Ok(error(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "MethodNotAllowed",
                    "Unsupported bucket method",
                ))
            }
        }
    }
    let bucket_stat = be.stat(Path::new(&bucket_path)).await?;
    if !bucket_stat.is_dir {
        return Err(Error::NotFound(bucket_path.into()));
    }
    match method.as_str() {
        "PUT" => {
            if let Some(copy) = headers.get("x-amz-copy-source").and_then(|v| v.to_str().ok()) {
                let source = percent_encoding::percent_decode_str(copy)
                    .decode_utf8()
                    .map_err(|_| Error::InvalidInput("invalid copy source".into()))?;
                be.write(Path::new(path), be.read(Path::new(source.as_ref()), 0..u64::MAX).await?)
                    .await?;
                let etag = be.etag(Path::new(path)).await?;
                return Ok(response(
                    StatusCode::OK,
                    format!(
                        "<CopyObjectResult><ETag>&quot;{}&quot;</ETag></CopyObjectResult>",
                        xml(&etag)
                    ),
                ));
            }
            be.write(Path::new(path), checked_body(body, headers, path)).await?;
            let etag = be.etag(Path::new(path)).await?;
            Ok((StatusCode::OK, [(header::ETAG, format!("\"{}\"", etag))]).into_response())
        }
        "DELETE" => {
            match be.remove(Path::new(path), false).await {
                Ok(()) | Err(Error::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        "GET" | "HEAD" => {
            let snapshot = be
                .open_read(Path::new(path), if method == Method::HEAD { 0..0 } else { 0..u64::MAX })
                .await?;
            let s = snapshot.status;
            if s.is_dir {
                return Err(Error::NotFound(path.into()));
            }
            let etag = format!("\"{}\"", snapshot.etag);
            if headers
                .get(header::IF_MATCH)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v != "*" && v != etag)
            {
                return Ok(StatusCode::PRECONDITION_FAILED.into_response());
            }
            if headers
                .get(header::IF_NONE_MATCH)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == "*" || v == etag)
            {
                return Ok(StatusCode::NOT_MODIFIED.into_response());
            }
            let range = if let Some(raw) = headers.get(header::RANGE) {
                match raw.to_str().ok().and_then(|v| parse_range(v, s.len)) {
                    Some(r) => r,
                    None => {
                        return Ok((
                            StatusCode::RANGE_NOT_SATISFIABLE,
                            [(header::CONTENT_RANGE, format!("bytes */{}", s.len))],
                        )
                            .into_response())
                    }
                }
            } else {
                0..s.len
            };
            let partial = headers.contains_key(header::RANGE);
            let count = range.end - range.start;
            let content_range =
                format!("bytes {}-{}/{}", range.start, range.end.saturating_sub(1), s.len);
            let stream = if method == Method::HEAD {
                Body::empty()
            } else {
                Body::from_stream(futures_util::stream::unfold(
                    (snapshot.data, 0u64, range),
                    |(mut stream, mut offset, range)| async move {
                        while let Some(chunk) = stream.next().await {
                            let bytes = match chunk {
                                Ok(bytes) => bytes,
                                Err(e) => return Some((Err(e), (stream, offset, range))),
                            };
                            let end = offset + bytes.len() as u64;
                            if offset < range.end && end > range.start {
                                let a = range.start.saturating_sub(offset) as usize;
                                let b = (range.end.min(end) - offset) as usize;
                                return Some((Ok(bytes.slice(a..b)), (stream, end, range)));
                            }
                            offset = end;
                            if offset >= range.end {
                                return None;
                            }
                        }
                        None
                    },
                ))
            };
            let mut response = (
                if partial { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK },
                [
                    (header::CONTENT_LENGTH, count.to_string()),
                    (header::CONTENT_TYPE, "application/octet-stream".into()),
                    (header::ACCEPT_RANGES, "bytes".into()),
                    (header::ETAG, etag),
                    (
                        header::LAST_MODIFIED,
                        httpdate::fmt_http_date(
                            std::time::UNIX_EPOCH
                                + std::time::Duration::from_secs(s.modified.max(0) as u64),
                        ),
                    ),
                ],
                stream,
            )
                .into_response();
            if partial {
                response
                    .headers_mut()
                    .insert(header::CONTENT_RANGE, content_range.parse().expect("generated range"));
            }
            Ok(response)
        }
        _ => Ok(error(
            StatusCode::METHOD_NOT_ALLOWED,
            "MethodNotAllowed",
            "Unsupported object method",
        )),
    }
}

// A checksum failure is emitted as the final stream item, before Backend::write
// commits the staged bytes. Failed uploads preserve any previous object.
fn checked_body(body: Body, headers: &HeaderMap, path: &str) -> mammoth_core::backend::ByteStream {
    let expected: Vec<_> =
        ["content-md5", "x-amz-checksum-crc32c", "x-amz-checksum-crc32", "x-amz-content-sha256"]
            .iter()
            .map(|key| headers.get(*key).and_then(|v| v.to_str().ok()).map(str::to_owned))
            .collect();
    let path = path.to_owned();
    Box::pin(futures_util::stream::unfold(
        (
            body.into_data_stream(),
            Md5::new(),
            sha2::Sha256::new(),
            0u32,
            crc32fast::Hasher::new(),
            expected,
            path,
            false,
        ),
        |(mut stream, mut md5, mut sha, mut crc32c, mut crc32, expected, path, done)| async move {
            if done {
                return None;
            }
            if let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        md5.update(&bytes);
                        sha.update(&bytes);
                        crc32c = crc32c::crc32c_append(crc32c, &bytes);
                        crc32.update(&bytes);
                        return Some((
                            Ok(bytes),
                            (stream, md5, sha, crc32c, crc32, expected, path, false),
                        ));
                    }
                    Err(e) => {
                        return Some((
                            Err(Error::Io(std::io::Error::other(e))),
                            (stream, md5, sha, crc32c, crc32, expected, path, true),
                        ))
                    }
                }
            }
            let actual = [
                base64::engine::general_purpose::STANDARD.encode(md5.clone().finalize()),
                base64::engine::general_purpose::STANDARD.encode(crc32c.to_be_bytes()),
                base64::engine::general_purpose::STANDARD
                    .encode(crc32.clone().finalize().to_be_bytes()),
                format!("{:x}", sha.clone().finalize()),
            ];
            for (want, got) in expected.iter().zip(actual) {
                if let Some(want) = want {
                    if want != "UNSIGNED-PAYLOAD" && *want != got {
                        let error = Error::ChecksumMismatch {
                            path: path.clone().into(),
                            expected: want.clone(),
                            actual: got,
                        };
                        return Some((
                            Err(error),
                            (stream, md5, sha, crc32c, crc32, expected, path, true),
                        ));
                    }
                }
            }
            None
        },
    ))
}
