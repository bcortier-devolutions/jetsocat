use seahorse::{App, Command, Context};
use std::env;
use tokio::runtime;

fn main() {
    let args: Vec<String> = if let Ok(args_str) = std::env::var("JETSOCAT_ARGS") {
        env::args()
            .take(1)
            .chain(args_str.split(" ").map(|s| s.to_owned()))
            .collect()
    } else {
        env::args().collect()
    };

    let app = App::new(env!("CARGO_PKG_NAME"))
        .description(env!("CARGO_PKG_DESCRIPTION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .version(env!("CARGO_PKG_VERSION"))
        .usage(format!("{} [command]", env!("CARGO_PKG_NAME")))
        .command(connect_command())
        .command(accept_command());

    app.run(args);
}

fn setup_logger() -> slog::Logger {
    use slog::o;
    use slog::Drain;
    use std::fs::OpenOptions;
    use std::panic;

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("jetsocat.log")
        .unwrap();

    let decorator = slog_term::PlainDecorator::new(file);
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")));

    let logger_cloned = logger.clone();
    panic::set_hook(Box::new(move |panic_info| {
        slog::error!(logger_cloned, "{:?}", panic_info);
    }));

    logger
}

fn connect_command() -> Command {
    Command::new("connect")
        .description("Connect to a jet association and pipe stdin / stdout")
        .alias("c")
        .usage(format!(
            "{} connect ws://URL | wss://URL",
            env!("CARGO_PKG_NAME")
        ))
        .action(connect_action)
}

pub fn connect_action(c: &Context) {
    let addr = c.args.first().unwrap().clone();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let log = setup_logger();

    rt.block_on(jetsocat::connect(addr, log)).unwrap();
}

fn accept_command() -> Command {
    Command::new("accept")
        .description("Accept a jet association and pipe with powershell")
        .alias("a")
        .usage(format!(
            "{} accept ws://URL | wss://URL",
            env!("CARGO_PKG_NAME")
        ))
        .action(accept_action)
}

pub fn accept_action(c: &Context) {
    let addr = c.args.first().unwrap().clone();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let log = setup_logger();

    rt.block_on(jetsocat::accept(addr, log)).unwrap();
}
