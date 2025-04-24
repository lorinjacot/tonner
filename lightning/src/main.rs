use std::path::PathBuf;

use clap::Parser;
use lightning::run;

#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Optional asset to load on launch
    asset: Option<PathBuf>,
}

fn main() {
    env_logger::init();

    let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
    let _puffin_server = puffin_http::Server::new(&server_addr).unwrap();

    profiling::puffin::set_scopes_on(true);

    let args = Args::parse();

    run(args.asset);
}
