use std::path::Path;
use suif::{type_of, we, Dbg, Deser, Opt, Rst, Ser};
use tracing_appender::non_blocking as nb;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::{self, time::ChronoLocal};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[derive(Clone, Dbg, Ser, Deser)]
pub struct LogConf {
    pub dir: &'static str,
    pub file: bool,
    pub stdout: bool,
}

const LOG_PREFIX: &str =
    concat!(env!("CARGO_PKG_NAME"), '-', env!("CARGO_PKG_VERSION"), '-');

pub async fn suber_init(conf: &LogConf) -> Rst<(Opt<WorkerGuard>, Opt<WorkerGuard>)> {
    if let Err(e) = tokio::fs::create_dir_all(Path::new(&conf.dir)).await {
        we!(
            "{}: {e:?}; Failed to initialize log directory! conf={:?}",
            type_of(&e),
            conf
        );
    };

    let dir = match tokio::fs::canonicalize(&conf.dir).await {
        Ok(f) => f,
        Err(e) => we!(
            "{}: {e:?}; Failed to canonicalize log directory! conf={:?}",
            type_of(&e),
            conf
        ),
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
        .with(EnvFilter::from_env("SUIF_LOG"))
        .init();
    Ok((_file_guard, _stdout_guard))
}
