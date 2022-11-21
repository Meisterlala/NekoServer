use log::info;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::time::Duration;
use std::{
    env,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::time;
use warp::{http::Response, hyper::StatusCode, reply, Filter};

mod count_image;
use count_image::CountImage;

mod image_cache;
use image_cache::ImageCache;

mod const_image;
mod season_images;

const IMAGE_SOURCES: [&str; 12] = [
    "nekos.life",
    "nekos.best",
    "pic.re",
    "shibe.online",
    "catboys",
    "waifu.im",
    "waifu.pics",
    "dog_ceo",
    "the_cat_api",
    "twitter_search",
    "twitter_user_timeline",
    "testing",
];

lazy_static::lazy_static! {
    // Cache of images
    static ref IMAGE_CACHE: ImageCache = ImageCache::new();
}

pub async fn init(port: u16) {
    // Create Logger
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "neko_server=info");
    }
    if env::var_os("RUST_LOG_STYLE").is_none() {
        env::set_var("RUST_LOG_STYLE", "never");
    }
    env_logger::builder()
        .format_timestamp(None)
        .target(env_logger::Target::Stdout)
        .init();

    // Database connection
    let redis_url: &str = &env::var("REDIS_URL").expect("REDIS_URL must be set");
    let redis_client = redis::Client::open(redis_url).expect("Incorrect Redis URL");
    let redis = ConnectionManager::new(redis_client)
        .await
        .unwrap_or_else(|_| panic!("Failed to connect to Redis at: {}", redis_url));
    {
        let mut conn = redis.clone();
        let check: Result<(), _> = conn.set("auth_test", "success").await;
        check.expect("Failed to execute redis commands");
    }

    // Update the image from the database
    let mut redis_clone = redis.clone();
    let update_task = tokio::spawn(async move {
        let mut interval = time::interval(image_cache::UPDATE_INTERVAL);
        loop {
            interval.tick().await;

            let mut pipe = redis::pipe();
            pipe.atomic();
            for source in IMAGE_SOURCES.iter() {
                pipe.get(*source);
            }
            let results = pipe
                .query_async::<_, Vec<Option<u64>>>(&mut redis_clone)
                .await;

            if results.is_err() {
                log::error!("Failed to get image count from Redis");
                continue;
            }
            let sum: u64 = results.unwrap().into_iter().flatten().sum();

            IMAGE_CACHE.update_total_image(sum as u128).await;
        }
    });

    // Save historical data
    let mut redis_clone = redis.clone();
    let history_task = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60 * 60)); // 1 hours
        loop {
            interval.tick().await;

            // Get current sum
            let mut pipe = redis::pipe();
            pipe.atomic();
            for source in IMAGE_SOURCES.iter() {
                pipe.get(*source);
            }
            let results = pipe
                .query_async::<_, Vec<Option<u64>>>(&mut redis_clone)
                .await;

            if results.is_err() {
                log::error!(target: "history", "Failed to get image count from Redis");
                continue;
            }
            let sum: u64 = results.unwrap().into_iter().flatten().sum();

            // Get Date and Key name
            let date = chrono::Utc::now().format("%Y%m%d").to_string();
            let key_name = format!("history:{}", date);

            // Save to Redis
            let result: Result<(), redis::RedisError> = redis_clone.set(key_name, sum).await;
            if result.is_err() {
                log::error!(target: "history", "Failed to save history to Redis");
                continue;
            }

            log::info!(target: "history", "Saved history of {} downloads on {} to Redis", sum, date);
        }
    });

    // Print current season
    info!("Current Season: {}", season_images::seasonal_name());

    // Create a filter for each Imagesource
    let add_routes = IMAGE_SOURCES
        .iter()
        .map(|name| {
            info!("Adding route for {}", name);
            // Add the path /add/<name_snake>/:count
            warp::path("add")
                .and(warp::path(name))
                .and(warp::post())
                .and(warp::path::param())
                .and(warp::header::optional::<String>("User-Agent"))
                .and(with_redis(redis.clone()))
                .and_then(move |count, agent, redis| add(name, count, agent, redis))
                .or(warp::path("add")
                    .and(warp::path(name))
                    .and(warp::post())
                    .and_then(|| async {
                        Ok::<_, warp::Rejection>(
                            Response::builder()
                                .status(StatusCode::NOT_ACCEPTABLE)
                                .body("Number to large".to_string())
                                .unwrap(),
                        )
                    }))
                .boxed()
        })
        .reduce(|acc, item| acc.or(item).unify().boxed())
        .expect("No routes were created");

    // Add error message
    let add_routes = add_routes.or(warp::path("add").map(|| {
        Response::builder()
            .status(StatusCode::NOT_ACCEPTABLE)
            .body("Unknown Source".to_string())
            .unwrap()
    }));

    // Create a filter to get the total count image
    let get_image = warp::path("count_total")
        .and(warp::get())
        .and_then(get_total_image);

    // Create a filter to get the image for a specific count
    let get_count = warp::path("count")
        .and(warp::get())
        .and(warp::path::param())
        .and_then(get_count_image);

    // Add a default route to display server verison
    let index = warp::path::end().map(|| {
        reply::html(format!(
            r#"<!DOCTYPE html>
<html>
    <head>
        <title>Neko Server</title>
    </head>
    <body>
        <h1 style="text-align: center;font-size:10vw">Neko Server</h1>
        <p style="text-align: center;font-size:3vw">Version: {}</p>
    </body>
</html>"#,
            env!("CARGO_PKG_VERSION")
        ))
    });

    // Combine all Filters
    let routes = get_image.or(add_routes).or(get_count).or(index);

    // Add logger
    let routes = routes.with(warp::log::custom(|info| {
        // Look for the `x-forwarded-for` header, and if it's not present, fall back to x-real-ip, and if that's not present, fall back to the remote addr.
        let ip: String = info
            .request_headers()
            .get("x-forwarded-for")
            .or_else(|| info.request_headers().get("x-real-ip"))
            .map(|ip| ip.to_str().unwrap_or("Invalid Header"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                info.remote_addr()
                    .map(|sa| sa.ip().to_string())
                    .unwrap_or_else(|| "Unknown".to_owned())
            });

        log::info!(
            "{} {} - Status: {} - IP: {} - Agent: {} - Time: {:?}",
            info.method(),
            info.path(),
            info.status(),
            ip,
            info.user_agent().unwrap_or("no agent"),
            info.elapsed()
        );
    }));

    // Default route
    let error_log = warp::log::custom(|info| {
        // Header to list
        let headers: String = info
            .request_headers()
            .iter()
            .map(|(key, value)| format!("{}: {}\n", key, value.to_str().unwrap_or("[empty]")))
            .collect::<String>();
        log::info!(
            "SUS REQUEST: {} {} - Status: {} - Agent: {} - Time: {:?} - Headers: \n{}",
            info.method(),
            info.path(),
            info.status(),
            info.user_agent().unwrap_or("no agent"),
            info.elapsed(),
            headers.trim_end()
        );
    });
    let routes = routes.or(warp::any()
        .map(|| {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body("Not Found. This incident will be reported.".to_string())
                .unwrap()
        })
        .with(error_log));

    // Run the server
    info!(
        "Starting nekoserver version {} on port {}",
        env!("CARGO_PKG_VERSION"),
        port
    );
    let server = tokio::spawn(async move { warp::serve(routes).run(([0, 0, 0, 0], port)).await });

    // Wait for termination signal
    let term = Arc::new(AtomicBool::new(false));
    for sig in signal_hook::consts::TERM_SIGNALS {
        signal_hook::flag::register(*sig, Arc::clone(&term))
            .expect("Failed to register signal handler");
    }
    while !term.load(Ordering::Relaxed) {
        time::sleep(Duration::from_secs(1)).await;
    }

    // Shutdown the server
    info!("Shutting down");

    // Cleanup
    update_task.abort();
    history_task.abort();
    server.abort();
}

async fn add(
    name: &str,
    count: u8,
    agent: Option<String>,
    mut redis: ConnectionManager,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Check for correct header
    match agent {
        Some(agent) if agent.contains("NekoFans") => (),
        _ => {
            return Ok(reply::with_status(
                "Valid request, but unauthorized",
                StatusCode::UNAUTHORIZED,
            ));
        }
    }

    // Increment the count
    let r: Result<(), redis::RedisError> = redis.incr(name, count.to_string()).await;

    match r {
        Ok(_) => Ok(reply::with_status("OK", StatusCode::OK)),
        Err(_) => Ok(reply::with_status("", StatusCode::NOT_MODIFIED)),
    }
}

async fn get_total_image() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(Response::builder()
        .header("Content-Type", "image/png")
        .header("Cache-Control", "no-cache")
        .body(IMAGE_CACHE.get_total().await))
}

async fn get_count_image(count: u128) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(Response::builder()
        .header("Content-Type", "image/png")
        .body(IMAGE_CACHE.get_count(count).await))
}

fn with_redis(
    connection: ConnectionManager,
) -> impl Filter<Extract = (ConnectionManager,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || connection.clone())
}
