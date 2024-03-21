use std::path::Path;
use tracing_appender::non_blocking as nb;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::{self, time::ChronoLocal};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use uifs::{mk_err, we, Dbg, Deser, Opt, Rst, Ser};

#[derive(Clone, Dbg, PartialEq, Ser, Deser)]
pub struct LogConf {
    pub dir: &'static str,
    pub file: bool,
    pub stdout: bool,
}

const LOG_PREFIX: &str =
    concat!(env!("CARGO_PKG_NAME"), '-', env!("CARGO_PKG_VERSION"), '-');

pub async fn sub_init(conf: &LogConf) -> Rst<(Opt<WorkerGuard>, Opt<WorkerGuard>)> {
    if let Err(e) = tokio::fs::create_dir_all(Path::new(&conf.dir)).await {
        we!("{} conf={conf:?}", mk_err(e, "Failed to initialize log directory!"));
    };

    let dir = match tokio::fs::canonicalize(&conf.dir).await {
        Ok(f) => f,
        Err(e) => {
            we!("{} conf={conf:?}", mk_err(e, "Failed to canonicalize log directory!"))
        }
    };

    let mut layers = Vec::new();
    let _file_guard = if conf.file {
        let (file_writer, _file_guard) =
            nb(tracing_appender::rolling::daily(dir, LOG_PREFIX));
        layers.push(
            fmt::Layer::new()
                .with_ansi(false)
                .with_level(true)
                .with_line_number(true)
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(true)
                .with_timer(ChronoLocal::default())
                .with_writer(file_writer),
        );
        Some(_file_guard)
    } else {
        None
    };
    let _stdout_guard = if conf.stdout {
        let (stdout_writer, _stdout_guard) = nb(std::io::stdout());
        layers.push(
            fmt::Layer::new()
                .with_ansi(true)
                .with_level(true)
                .with_line_number(true)
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(true)
                .with_timer(ChronoLocal::default())
                .with_writer(stdout_writer),
        );
        Some(_stdout_guard)
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(layers)
        .with(EnvFilter::from_default_env())
        .init();
    Ok((_file_guard, _stdout_guard))
}
