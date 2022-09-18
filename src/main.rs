use std::env;

use clap::Parser;
use neko_server::init;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Some(val) = args.redis {
        env::set_var("REDIS_URL", val);
    }
    init(args.port).await
}

#[derive(Parser, Debug)]
#[clap(author, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[clap(short, long, value_parser, default_value = "80")]
    port: u16,

    /// Redis URL
    #[clap(short, long, value_parser)]
    redis: Option<String>,
}
