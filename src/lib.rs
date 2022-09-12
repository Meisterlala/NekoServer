use image::{ImageBuffer, Rgba};
use log::info;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Pool, Sqlite,
};
use std::io::Write;
use std::time::Duration;
use std::{
    env,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{Mutex, OnceCell},
    time,
};
use warp::{http::Response, hyper::StatusCode, reply, Filter};

const COUNT_IMAGE_INTERVAL: Duration = Duration::from_secs(60 * 15);

// Pool of connections to the database
static POOL: OnceCell<Pool<Sqlite>> = OnceCell::const_new();

// Image in memory
static COUNT_IMAGE: OnceCell<Mutex<CountImage>> = OnceCell::const_new();

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

    // Load Image to memory
    COUNT_IMAGE.set(Mutex::new(CountImage::new())).unwrap();

    // Create Logger
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "neko_server=info");
    }
    pretty_env_logger::init_timed();

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

    // Background Image generation every hour
    let img_task = tokio::spawn(async {
        let mut interval = time::interval(COUNT_IMAGE_INTERVAL);
        loop {
            interval.tick().await;
            COUNT_IMAGE.get().unwrap().lock().await.update_image().await;
        }
    });

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

    // Create a filter to get the image
    let get_image = warp::path("count_image")
        .and(warp::get())
        .and_then(get_image);

    // Combine all Filters
    let routes = get_image.or(add_routes).with(warp::log("neko_server"));

    // Run the server
    let server = tokio::spawn(async move { warp::serve(routes).run(([0, 0, 0, 0], port)).await });

    // Wait for termination signal
    let term = Arc::new(AtomicBool::new(false));
    for sig in signal_hook::consts::TERM_SIGNALS {
        signal_hook::flag::register(*sig, Arc::clone(&term))
            .expect("Failed to register signal handler");
    }
    while !term.load(Ordering::Relaxed) {}

    // Shutdown the server
    info!("Shutting down");

    // Cleanup
    POOL.get().unwrap().close().await;
    img_task.abort();
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

async fn get_image() -> Result<impl warp::Reply, warp::Rejection> {
    let image = COUNT_IMAGE.get().unwrap().lock().await;
    Ok(Response::builder()
        .header("Content-Type", "image/png")
        .body(image.get_image()))
}

#[derive(Debug)]
/// This struct holds the image in memory
pub struct CountImage {
    data: ImageBuffer<Rgba<u8>, Vec<u8>>,
    body: String,
}

impl CountImage {
    /// Returns a Clone of the image
    pub fn get_image(&self) -> warp::hyper::Body {
        self.body.clone().into()
    }

    pub fn new() -> Self {
        let inital_data = match image::open("image.png") {
            Ok(data) => data.to_rgba8(),
            Err(_) => image::RgbaImage::new(128, 128),
        };
        CountImage {
            data: inital_data,
            body: "".into(),
        }
    }

    /// Render the image
    pub fn create_image(count: i128) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        info!("Creating new Image with Count: {}", count);

        // Load template image to draw over
        let mut base = image::load_from_memory(include_bytes!("../template/count.png"))
            .expect("Could not load template image")
            .to_rgba8();

        // Load Number Images
        let numbers: [image::RgbaImage; 10] = [
            image::load_from_memory(include_bytes!("../template/numbers/0.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/1.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/2.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/3.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/4.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/5.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/6.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/7.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/8.png"))
                .expect("Could not load number image")
                .to_rgba8(),
            image::load_from_memory(include_bytes!("../template/numbers/9.png"))
                .expect("Could not load number image")
                .to_rgba8(),
        ];

        // Create a new image with the same dimensions as the template
        let mut overlay = image::RgbaImage::new(base.width(), base.height());

        // Wite the Count onto the image
        let number_start = (560, ((base.height() - numbers[0].height()) / 2) as i64);
        let mut number_pos = number_start;
        for digit in count.to_string().chars() {
            let digit = digit.to_digit(10).unwrap() as usize;
            let number = &numbers[digit];
            image::imageops::overlay(&mut overlay, number, number_pos.0, number_pos.1);
            number_pos.0 += number.width() as i64;
        }

        // Overlay the image
        image::imageops::overlay(&mut base, &overlay, 0, 0);

        base
    }

    /// Save to Image to the disk (not really needed but nice for debugging)
    fn save_image(&self) {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open("image.png")
            .expect("Cant open image to write to it");
        let mut bufferd = std::io::BufWriter::new(file);
        self.data
            .write_to(&mut bufferd, image::ImageOutputFormat::Png)
            .expect("Cant write to image");
        bufferd.flush().expect("Cant flush image");
    }

    /// Convert the image to a String
    fn update_body(&mut self) {
        // Setup Bufferwriter with a Cursor so we have Seek
        let mut buffer = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));
        // Write Image to Buffer
        self.data
            .write_to(&mut buffer, image::ImageOutputFormat::Png)
            .unwrap();
        buffer.flush().unwrap();
        // Extract the written data
        let data = buffer.into_inner().unwrap().into_inner();
        // SAFETY: We dont care, that this is not Valid UTF-8. Its binary data
        let res = unsafe { String::from_utf8_unchecked(data) };
        self.body = res;
    }

    /// Update the image from the database
    pub async fn update_image(&mut self) {
        let counts = sqlx::query!("SELECT count FROM ImageInfo")
            .fetch_all(POOL.get().unwrap())
            .await
            .expect("Failed to Image Count Sum from DB");

        let sum: i128 = counts.iter().map(|count| count.count as i128).sum();
        self.data = Self::create_image(sum);
        self.update_body();
        self.save_image();
    }
}

impl Default for CountImage {
    fn default() -> Self {
        Self::new()
    }
}
