use clap::Parser;
use neko_server::init;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    init(args.port, &args.db).await
}

#[derive(Parser, Debug)]
#[clap(author, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[clap(short, long, value_parser)]
    port: u16,

    /// Path to the SQLite database
    #[clap(short, long, value_parser, value_name = "FILE")]
    db: String,
}
