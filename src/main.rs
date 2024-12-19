use lightning::run;
use log::debug;

fn main() {
    env_logger::init();

    debug!("test env_logger");

    run();
}
