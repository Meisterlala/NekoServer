use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::time::Duration;
use tokio::time;
use warp::http::Response;
use warp::hyper::StatusCode;

const DEFAULT_CACHE_TTL_SECONDS: u64 = 15 * 60;
const LOCK_TTL_SECONDS: u64 = 30;
const MAX_TAGS: usize = 20;
const MAX_TAG_LENGTH: usize = 64;
const MAX_RATING_LENGTH: usize = 32;
const MAX_LIMIT: u16 = 100;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
pub struct GalleryQuery {
    source: Option<String>,
    url: Option<String>,
    tags: Option<Vec<String>>,
    rating: Option<String>,
    limit: Option<u16>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    url: &'a str,
    args: Vec<String>,
}

#[derive(Serialize)]
struct GalleryResponse {
    items: Vec<GalleryItem>,
    errors: Vec<GalleryExtractorError>,
}

#[derive(Serialize)]
struct GalleryItem {
    url: Option<String>,
    source: Option<String>,
    category: Option<String>,
    subcategory: Option<String>,
    id: Option<serde_json::Value>,
    title: Option<String>,
    filename: Option<String>,
    extension: Option<String>,
    file_url: Option<String>,
    preview_url: Option<String>,
    sample_url: Option<String>,
    width: Option<u64>,
    height: Option<u64>,
    rating: Option<String>,
    score: Option<i64>,
    tags: Option<serde_json::Value>,
    created_at: Option<serde_json::Value>,
    metadata: serde_json::Value,
}

#[derive(Serialize)]
struct GalleryExtractorError {
    error: Option<String>,
    message: Option<String>,
    metadata: serde_json::Value,
}

struct GalleryTarget {
    cache_source: String,
    cache_terms: Vec<String>,
    url: String,
}

pub async fn query(
    request: GalleryQuery,
    mut redis: ConnectionManager,
) -> Result<impl warp::Reply, warp::Rejection> {
    let cache_ttl_seconds = cache_ttl_seconds();
    let limit = request.limit.unwrap_or(50);
    if limit == 0 || limit > MAX_LIMIT {
        return Ok(json_error(StatusCode::BAD_REQUEST, "invalid limit"));
    }
    let target = match gallery_target(request, limit) {
        Ok(target) => target,
        Err(message) => return Ok(json_error(StatusCode::BAD_REQUEST, message)),
    };

    let cache_key = cache_key(&target.cache_source, &target.cache_terms, limit);
    let cached: Result<Option<String>, _> = redis.get(&cache_key).await;
    let cached = match cached {
        Ok(value) => value,
        Err(_) => {
            return Ok(json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "cache unavailable",
            ))
        }
    };
    if let Some(value) = cached {
        if let Ok(gallery_dl) = serde_json::from_str::<serde_json::Value>(&value) {
            let ttl = cache_ttl(&mut redis, &cache_key, cache_ttl_seconds).await;
            return Ok(json_response(
                StatusCode::OK,
                &gallery_dl,
                Some(("HIT", ttl)),
            ));
        }
        let _: Result<(), _> = redis.del(&cache_key).await;
    }

    let lock_key = cache_key.replace("gallery:v1:result:", "gallery:v1:lock:");
    let lock: Result<Option<String>, _> = redis::cmd("SET")
        .arg(&lock_key)
        .arg("1")
        .arg("NX")
        .arg("EX")
        .arg(LOCK_TTL_SECONDS)
        .query_async(&mut redis)
        .await;
    match lock {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(json_error(StatusCode::ACCEPTED, "query already running")),
        Err(_) => {
            return Ok(json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "cache unavailable",
            ))
        }
    }

    let output = match run_gallery_dl(&target.url, limit).await {
        Ok(value) => value,
        Err(message) => {
            let _: Result<(), _> = redis.del(lock_key).await;
            return Ok(json_error(StatusCode::BAD_GATEWAY, message));
        }
    };
    let output = normalize_gallery_output(output);

    let serialized = match serde_json::to_string(&output) {
        Ok(serialized) => serialized,
        Err(_) => {
            let _: Result<(), _> = redis.del(lock_key).await;
            return Ok(json_error(
                StatusCode::BAD_GATEWAY,
                "invalid gallery-dl output",
            ));
        }
    };
    if let Err(error) = redis
        .set_ex::<_, _, ()>(&cache_key, serialized, cache_ttl_seconds)
        .await
    {
        log::error!("failed to cache gallery-dl response: {}", error);
        let _: Result<(), _> = redis.del(lock_key).await;
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "cache unavailable",
        ));
    }
    let _: Result<(), _> = redis.del(lock_key).await;

    Ok(json_response(
        StatusCode::OK,
        &output,
        Some(("MISS", cache_ttl_seconds)),
    ))
}

struct NormalizedQuery {
    terms: Vec<String>,
}

fn gallery_target(request: GalleryQuery, limit: u16) -> Result<GalleryTarget, &'static str> {
    if let Some(url) = request.url {
        if request.source.is_some() || request.tags.is_some() || request.rating.is_some() {
            return Err("url cannot be combined with source, tags, or rating");
        }
        let url = normalize_url(url)?;
        return Ok(GalleryTarget {
            cache_source: "url".to_string(),
            cache_terms: vec![url.clone()],
            url,
        });
    }

    let source = match request.source {
        Some(source) => normalize_source(source)?,
        None => return Err("missing source or url"),
    };
    let query = normalize_query(request.tags, request.rating)?;
    let url = match build_url(&source, &query.terms, limit) {
        Some(url) => url,
        None => return Err("unsupported source"),
    };

    Ok(GalleryTarget {
        cache_source: source,
        cache_terms: query.terms,
        url,
    })
}

fn normalize_url(url: String) -> Result<String, &'static str> {
    let url = url.trim().to_string();
    if url.len() > 2048 || !url.starts_with("https://") || url.contains('\0') {
        return Err("invalid url");
    }
    Ok(url)
}

fn normalize_source(source: String) -> Result<String, &'static str> {
    let source = source.trim().to_ascii_lowercase();
    if source.is_empty() {
        return Err("invalid source");
    }
    Ok(source)
}

fn normalize_query(
    tags: Option<Vec<String>>,
    rating: Option<String>,
) -> Result<NormalizedQuery, &'static str> {
    let tags = normalize_tags(tags.unwrap_or_default())?;
    let rating = normalize_rating(rating)?;
    let mut terms = tags.clone();
    if let Some(rating) = &rating {
        terms.push(format!("rating:{}", rating));
    }
    terms.sort();
    terms.dedup();

    if terms.is_empty() {
        return Err("missing query terms");
    }

    Ok(NormalizedQuery { terms })
}

fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, &'static str> {
    if tags.len() > MAX_TAGS {
        return Err("invalid tag count");
    }

    let mut normalized = Vec::with_capacity(tags.len());
    for tag in tags {
        let tag = tag.trim().to_ascii_lowercase();
        if tag.is_empty() || tag.len() > MAX_TAG_LENGTH || !tag.chars().all(is_safe_tag_char) {
            return Err("invalid tag");
        }
        normalized.push(tag);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn normalize_rating(rating: Option<String>) -> Result<Option<String>, &'static str> {
    let Some(rating) = rating else {
        return Ok(None);
    };

    let rating = rating.trim().to_ascii_lowercase();
    let rating = rating
        .strip_prefix("rating:")
        .unwrap_or(&rating)
        .to_string();
    if rating.is_empty()
        || rating.len() > MAX_RATING_LENGTH
        || !rating.chars().all(is_safe_rating_char)
    {
        return Err("invalid rating");
    }

    Ok(Some(rating))
}

fn is_safe_tag_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':')
}

fn is_safe_rating_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}

fn cache_ttl_seconds() -> u64 {
    env::var("GALLERY_DL_CACHE_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|ttl| *ttl > 0)
        .unwrap_or(DEFAULT_CACHE_TTL_SECONDS)
}

async fn cache_ttl(redis: &mut ConnectionManager, cache_key: &str, fallback: u64) -> u64 {
    let ttl: Result<i64, _> = redis.ttl(cache_key).await;
    ttl.ok()
        .and_then(|ttl| u64::try_from(ttl).ok())
        .filter(|ttl| *ttl > 0)
        .unwrap_or(fallback)
}

fn build_url(source: &str, tags: &[String], limit: u16) -> Option<String> {
    let joined_tags = tags.join("+");
    match source {
        "danbooru" => Some(format!(
            "https://danbooru.donmai.us/posts?tags={}&limit={}",
            joined_tags, limit
        )),
        "gelbooru" => Some(format!(
            "https://gelbooru.com/index.php?page=post&s=list&tags={}",
            joined_tags
        )),
        "safebooru" => Some(format!(
            "https://safebooru.org/index.php?page=post&s=list&tags={}",
            joined_tags
        )),
        "konachan" => Some(format!(
            "https://konachan.com/post?tags={}&limit={}",
            joined_tags, limit
        )),
        "yandere" => Some(format!(
            "https://yande.re/post?tags={}&limit={}",
            joined_tags, limit
        )),
        _ => None,
    }
}

fn cache_key(source: &str, tags: &[String], limit: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.update([0]);
    hasher.update(tags.join("\n").as_bytes());
    hasher.update([0]);
    hasher.update(limit.to_string().as_bytes());

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{:02x}", byte));
    }
    format!("gallery:v1:result:{}", hex)
}

fn normalize_gallery_output(value: serde_json::Value) -> GalleryResponse {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut pending_metadata: Option<serde_json::Value> = None;

    let serde_json::Value::Array(events) = value else {
        return GalleryResponse { items, errors };
    };

    for event in events {
        let serde_json::Value::Array(fields) = event else {
            continue;
        };
        let Some(code) = fields.first().and_then(serde_json::Value::as_i64) else {
            continue;
        };

        match code {
            -1 => {
                if let Some(metadata) = fields.get(1).cloned() {
                    errors.push(gallery_error(metadata));
                }
            }
            2 => {
                pending_metadata = fields.get(1).cloned();
            }
            3 => {
                let url = fields
                    .get(1)
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let metadata = fields
                    .get(2)
                    .cloned()
                    .or_else(|| pending_metadata.clone())
                    .unwrap_or(serde_json::Value::Null);
                items.push(gallery_item(url, metadata));
                pending_metadata = None;
            }
            _ => {}
        }
    }

    if items.is_empty() {
        if let Some(metadata) = pending_metadata {
            items.push(gallery_item(None, metadata));
        }
    }

    GalleryResponse { items, errors }
}

fn gallery_item(url: Option<String>, metadata: serde_json::Value) -> GalleryItem {
    GalleryItem {
        url,
        source: metadata_string(&metadata, "category"),
        category: metadata_string(&metadata, "category"),
        subcategory: metadata_string(&metadata, "subcategory"),
        id: metadata.get("id").cloned(),
        title: metadata_string(&metadata, "title"),
        filename: metadata_string(&metadata, "filename"),
        extension: metadata_string(&metadata, "extension")
            .or_else(|| metadata_string(&metadata, "file_ext")),
        file_url: metadata_string(&metadata, "file_url"),
        preview_url: metadata_string(&metadata, "preview_url")
            .or_else(|| metadata_string(&metadata, "preview_file_url")),
        sample_url: metadata_string(&metadata, "sample_url"),
        width: metadata_u64(&metadata, "width").or_else(|| metadata_u64(&metadata, "image_width")),
        height: metadata_u64(&metadata, "height")
            .or_else(|| metadata_u64(&metadata, "image_height")),
        rating: metadata_string(&metadata, "rating"),
        score: metadata_i64(&metadata, "score"),
        tags: metadata.get("tags").cloned(),
        created_at: metadata.get("created_at").cloned(),
        metadata,
    }
}

fn gallery_error(metadata: serde_json::Value) -> GalleryExtractorError {
    GalleryExtractorError {
        error: metadata_string(&metadata, "error"),
        message: metadata_string(&metadata, "message"),
        metadata,
    }
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn metadata_u64(metadata: &serde_json::Value, key: &str) -> Option<u64> {
    metadata
        .get(key)
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
}

fn metadata_i64(metadata: &serde_json::Value, key: &str) -> Option<i64> {
    metadata
        .get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
}

async fn run_gallery_dl(url: &str, limit: u16) -> Result<serde_json::Value, &'static str> {
    let worker_url =
        env::var("GALLERY_DL_WORKER_URL").map_err(|_| "GALLERY_DL_WORKER_URL is not configured")?;
    let client = reqwest::Client::new();
    let response = time::timeout(
        COMMAND_TIMEOUT,
        client
            .post(worker_url)
            .json(&WorkerRequest {
                url,
                args: vec!["--range".to_string(), format!("1-{}", limit)],
            })
            .send(),
    )
    .await
    .map_err(|_| "gallery-dl worker timed out")?
    .map_err(|_| "failed to call gallery-dl worker")?;

    if !response.status().is_success() {
        return Err("gallery-dl worker failed");
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|_| "failed to read gallery-dl worker response")?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err("gallery-dl output too large");
    }

    serde_json::from_slice(&bytes).map_err(|_| "invalid gallery-dl output")
}

fn json_response<T: Serialize>(
    status: StatusCode,
    value: &T,
    server_cache: Option<(&str, u64)>,
) -> Response<String> {
    let body = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    let mut builder = Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store");

    if let Some((cache_state, ttl)) = server_cache {
        builder = builder
            .header("X-Server-Cache", cache_state)
            .header("X-Server-Cache-Ttl-Seconds", ttl.to_string());
    }

    builder.body(body).unwrap()
}

fn json_error(status: StatusCode, message: &str) -> Response<String> {
    json_response(
        status,
        &ErrorResponse {
            error: message.to_string(),
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_download_events() {
        let response = normalize_gallery_output(json!([
            [2, {"category": "danbooru", "id": 123, "image_width": 800, "image_height": 600}],
            [3, "https://example.test/file.jpg", {"category": "danbooru", "id": 123, "file_ext": "jpg"}]
        ]));

        assert_eq!(response.items.len(), 1);
        assert_eq!(response.errors.len(), 0);
        assert_eq!(
            response.items[0].url.as_deref(),
            Some("https://example.test/file.jpg")
        );
        assert_eq!(response.items[0].category.as_deref(), Some("danbooru"));
        assert_eq!(response.items[0].extension.as_deref(), Some("jpg"));
    }

    #[test]
    fn normalizes_error_events() {
        let response = normalize_gallery_output(json!([[
            -1,
            {"error": "AuthRequired", "message": "credentials missing"}
        ]]));

        assert_eq!(response.items.len(), 0);
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].error.as_deref(), Some("AuthRequired"));
        assert_eq!(
            response.errors[0].message.as_deref(),
            Some("credentials missing")
        );
    }
}
