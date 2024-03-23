use std::path::Path;
use tracing_appender::non_blocking as non_blk;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::{self, time::ChronoLocal};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use uifs::{mk_err_str, we, Dbg, Deser, Opt, Rst, Ser};

#[derive(Clone, Dbg, PartialEq, Ser, Deser)]
pub struct Config<'s> {
	log_dir: &'s str,
	log_to_file: bool,
	log_to_cnsl: bool,
}

impl<'s> Config<'s> {
	const LOG_PREFIX: &'static str =
		concat!(env!("CARGO_PKG_NAME"), '-', env!("CARGO_PKG_VERSION"), '-');

	pub const fn new(log_dir: &'s str, log_to_file: bool, log_to_cnsl: bool) -> Self {
		Self { log_dir, log_to_file, log_to_cnsl }
	}

	pub async fn init(&self) -> Rst<(Opt<WorkerGuard>, Opt<WorkerGuard>)> {
		if let Err(e) = tokio::fs::create_dir_all(Path::new(self.log_dir)).await {
			we!("{} conf={self:?}", mk_err_str(e, "Failed to initialize log directory!"));
		};
		let dir = match tokio::fs::canonicalize(self.log_dir).await {
			Ok(f) => f,
			Err(e) => {
				we!("{} conf={self:?}", mk_err_str(e, "Failed to canonicalize log directory!"))
			}
		};
		let mut layers = Vec::new();
		let _file_guard = if self.log_to_file {
			let (file_writer, _file_guard) =
				non_blk(tracing_appender::rolling::daily(dir, Self::LOG_PREFIX));
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
		let _cnsl_guard = if self.log_to_cnsl {
			let (cnsl_writer, _cnsl_guard) = non_blk(std::io::stdout());
			layers.push(
				fmt::Layer::new()
					.with_ansi(true)
					.with_level(true)
					.with_line_number(true)
					.with_target(true)
					.with_thread_ids(false)
					.with_thread_names(true)
					.with_timer(ChronoLocal::default())
					.with_writer(cnsl_writer),
			);
			Some(_cnsl_guard)
		} else {
			None
		};

		tracing_subscriber::registry().with(layers).with(EnvFilter::from_default_env()).init();
		Ok((_file_guard, _cnsl_guard))
	}
}
