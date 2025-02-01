use lightning::run;

fn main() {
    env_logger::init();

    let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
    let _puffin_server = puffin_http::Server::new(&server_addr).unwrap();

    profiling::puffin::set_scopes_on(true);

    run();
}
