slint::include_modules!();

mod logger;

use core::time::Duration;
use logger::{suber_init, LogConf};
use mimalloc::MiMalloc;
use serialport::{available_ports, SerialPortType};
use slint::{format as slint_f, ModelRc, Timer};
use suif::{f, type_of, we, Rst};
use tracing::{debug, error, info, trace, warn};

const BAUD_RATE: u32 = 115_200;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Rst<()> {
    let log_conf = LogConf { dir: "./log", file: true, stdout: true };

    let _writer_guard = suber_init(&log_conf).await?;
    trace!(log_conf = ?log_conf, "Tracing Initialization finished.");

    #[cfg(debug_assertions)]
    debug!("Running on debug mode.");

    let ui = match AppWindow::new() {
        Ok(t) => t,
        Err(e) => {
            let e = f!("{}: {e:?}; Failed to initialize app_window!", type_of(&e),);
            error!("{e}");
            we!("{e}");
        }
    };

    let sps = match available_ports() {
        Ok(sps) => ModelRc::from({
            debug!(sps = ?sps, "Got available serial ports.");
            sps.into_iter()
                .map(|sp| {
                    if let SerialPortType::UsbPort(_) = sp.port_type {
                        slint_f!("{} UsbPort", sp.port_name)
                    } else {
                        slint_f!("{} {:?}", sp.port_name, sp.port_type)
                    }
                })
                .collect::<Vec<_>>()
                .as_slice()
        }),
        Err(e) => {
            let e = f!("{}: {e:?}; Failed to get serial ports!", type_of(&e),);
            error!("{e}");
            we!("{e}");
        }
    };

    ui.set_sps(sps);

    ui.on_sp_open(|sp_name| {
        info!(sp_name = ?sp_name, "Opening selected sp.");
        if let Some(path) = sp_name.split_whitespace().next() {
            match serialport::new(path, BAUD_RATE)
                .timeout(Duration::from_millis(500))
                .open()
            {
                Ok(s) => {
                    info!(sp = ?s, "Successfully opened sp.");
                    true
                }

                Err(e) => {
                    warn!("{}: {e:?}; Failed to open sp.", type_of(&e),);
                    false
                }
            }
        } else {
            warn!("Failed to parse chosen selected port name");
            false
        }
    });

    trace!("Start running GUI");

    let t = Timer::default();
    t.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {});

    if let Err(e) = ui.show() {
        let e = f!("{}: {e:?}; Failed to show app_window!", type_of(&e));
        error!("{e}");
        we!("{e}");
    };

    if let Err(e) = slint::run_event_loop() {
        let e = f!("{}: {e:?}; Failed to run slint event loop!", type_of(&e));
        error!("{e}");
        we!("{e}");
    };

    trace!("Application finished.");

    Ok(())
}
