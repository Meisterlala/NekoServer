
use log::info;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Pool, Sqlite,
};
use std::time::Duration;
use std::{
    env,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::OnceCell,
    time,
};
use warp::{http::Response, hyper::StatusCode, reply, Filter};

mod count_image;
use count_image::CountImage;

mod image_cache;
use image_cache::ImageCache;

mod const_image;

// Pool of connections to the database
static POOL: OnceCell<Pool<Sqlite>> = OnceCell::const_new();

lazy_static::lazy_static! {
    // Cache of images
    static ref IMAGE_CACHE: ImageCache = ImageCache::new();
}

pub async fn init(port: u16, db_path: &str) {
    // Create the database pool
    POOL.get_or_try_init(|| async {
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(db_path)
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal)
                    .synchronous(SqliteSynchronous::Normal),
            )
            .await
    })
    .await
    .expect("Failed to create database pool");


    // Update the image from the database
    let update_task = tokio::spawn(async {
        let mut interval = time::interval(image_cache::UPDATE_INTERVAL);
        loop {
            interval.tick().await;
            let counts = sqlx::query!("SELECT count FROM ImageInfo")
                .fetch_all(POOL.get().unwrap())
                .await
                .expect("Failed to Image Count Sum from DB");

            let sum: u128 = counts.iter().map(|count| count.count as u128).sum();
            IMAGE_CACHE.update_total_image(sum).await;
        }
    });

    // Create Logger
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "neko_server=info");
    }
    if env::var_os("RUST_LOG_STYLE").is_none() {
        env::set_var("RUST_LOG_STYLE", "never");
    }
    env_logger::builder().format_timestamp(None).init();

    // Create the Table
    sqlx::query_file!("sql/ImageInfo_crate.sql")
        .execute(POOL.get().unwrap())
        .await
        .unwrap();

    // Add Default Data
    sqlx::query_file!("sql/ImageInfo_add_defaults.sql")
        .execute(POOL.get().unwrap())
        .await
        .unwrap();

    // Get list of Sources in the DB
    let sources = sqlx::query!("SELECT name FROM ImageInfo")
        .fetch_all(POOL.get().unwrap())
        .await
        .unwrap();

    // Create a filter for each entry
    let add_routes = sources
        .iter()
        .map(|source| {
            // Convert name to snake_case
            let name = source.name.clone();
            let name_snake = name.replace(' ', "_");
            let name_snake = name_snake.to_lowercase();
            info!("Adding route for {}", &name_snake);
            // Add the path /add/<name_snake>/:count
            warp::path("add")
                .and(warp::path(name_snake.clone()))
                .and(warp::post())
                .and(warp::path::param())
                .and_then(move |count| add(name.clone(), count))
                .or(warp::path("add")
                    .and(warp::path(name_snake))
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

    // Combine all Filters
    let routes = get_image
        .or(add_routes)
        .or(get_count)
        .with(warp::log("neko_server"));

    // Run the server
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
    POOL.get().unwrap().close().await;
    update_task.abort();
    server.abort();
}

async fn add(name: String, count: u8) -> Result<impl warp::Reply, warp::Rejection> {
    if (sqlx::query!(
        r#"
        UPDATE ImageInfo
        SET Count = Count + ?
        WHERE Name = ?
        "#,
        count,
        name
    )
    .execute(POOL.get().unwrap())
    .await)
        .is_ok()
    {
        Ok(reply::with_status("OK", StatusCode::OK))
    } else {
        Ok(reply::with_status("", StatusCode::NOT_MODIFIED))
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
