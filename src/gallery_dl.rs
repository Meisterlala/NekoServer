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
    source: String,
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

pub async fn query(
    auth: Option<String>,
    request: GalleryQuery,
    mut redis: ConnectionManager,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !is_authorized(auth.as_deref()) {
        return Ok(json_error(StatusCode::UNAUTHORIZED, "unauthorized"));
    }

    let cache_ttl_seconds = cache_ttl_seconds();
    let limit = request.limit.unwrap_or(50);
    let source = match normalize_source(request.source) {
        Ok(source) => source,
        Err(message) => return Ok(json_error(StatusCode::BAD_REQUEST, message)),
    };
    let query = match normalize_query(request.tags, request.rating) {
        Ok(query) => query,
        Err(message) => return Ok(json_error(StatusCode::BAD_REQUEST, message)),
    };
    if limit == 0 || limit > MAX_LIMIT {
        return Ok(json_error(StatusCode::BAD_REQUEST, "invalid limit"));
    }

    let url = match build_url(&source, &query.terms, limit) {
        Some(url) => url,
        None => return Ok(json_error(StatusCode::BAD_REQUEST, "unsupported source")),
    };

    let cache_key = cache_key(&source, &query.terms, limit);
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

    let output = match run_gallery_dl(&url, limit).await {
        Ok(value) => value,
        Err(message) => {
            let _: Result<(), _> = redis.del(lock_key).await;
            return Ok(json_error(StatusCode::BAD_GATEWAY, message));
        }
    };

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

fn is_authorized(auth: Option<&str>) -> bool {
    let Ok(token) = env::var("GALLERY_DL_TOKEN") else {
        return true;
    };

    let expected = format!("Bearer {}", token);
    auth == Some(expected.as_str())
}

struct NormalizedQuery {
    terms: Vec<String>,
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
