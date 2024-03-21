slint::include_modules!();

mod logger;

use core::{cell::RefCell, time::Duration};
use logger::{sub_init, LogConf};
use mimalloc::MiMalloc;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use slint::{format as slint_f, ModelRc, Timer};
use std::cell::OnceCell;
use tracing::{debug, error, info, trace, warn};
use uifs::{mk_err, we, Rst};

const BAUD_RATE: u32 = 115_200;
const TIMEOUT: Duration = Duration::from_millis(300);

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

thread_local! {
    static SPS: OnceCell<Vec<SerialPortInfo>> = OnceCell::new();
    static SP: RefCell<Option<Box<dyn SerialPort>>> = RefCell::new(None);
}

#[tokio::main]
async fn main() -> Rst<()> {
    let log_conf = LogConf { dir: "./log", file: true, stdout: true };

    let _writer_guard = sub_init(&log_conf).await?;
    trace!(log_conf = ?log_conf, "Tracing Initialization finished.");

    #[cfg(debug_assertions)]
    debug!("Running on debug mode.");

    let ui = match AppWindow::new() {
        Ok(t) => t,
        Err(e) => {
            let e = mk_err(e, "Failed to initialize app_window!");
            error!("{e}");
            we!("{e}");
        }
    };

    match available_ports() {
        Ok(sps) => {
            debug!(sps = ?sps, "Got available serial ports.");
            let dp_sps: Vec<_> = sps
                .iter()
                .map(|sp| {
                    if let SerialPortType::UsbPort(_) = sp.port_type {
                        slint_f!("{} UsbPort", sp.port_name)
                    } else {
                        slint_f!("{} {:?}", sp.port_name, sp.port_type)
                    }
                })
                .collect();

            ui.set_sps(ModelRc::from(dp_sps.as_slice()));
            SPS.with(|oc| oc.set(sps).unwrap());
            trace!("Serial ports initialization finished.");
        }
        Err(e) => {
            let e = mk_err(e, "Failed to get serial ports!");
            error!("{e}");
            we!("{e}");
        }
    };

    ui.on_sp_open(|sp_idx| {
        let mut rtn = false;
        SPS.with(|oc| {
            SP.with(|rc| {
                let sps = oc.get().unwrap();
                let next_sp = &sps[sp_idx as usize];
                {
                    if let Some(cur_sp) = rc.borrow().as_ref() {
                        if cur_sp.name().unwrap() == next_sp.port_name {
                            info!(sp_idx = sp_idx, next_sp = ?next_sp, "Selected serial port has already been opened.");
                            rtn = true;
                            return;
                        };
                    };
                }

                match serialport::new(next_sp.port_name.as_str(), BAUD_RATE)
                    .timeout(TIMEOUT)
                    .open()
                {
                    Ok(s) => {
                        info!(sp_idx = sp_idx, next_sp = ?next_sp, "Successfully opened selected sp.");
                        rc.replace(Some(s));
                        rtn = true;
                    }
                    Err(e) => {
                        warn!(sp_idx = sp_idx, next_sp = ?next_sp, "{}", mk_err(e, "Failed to open selected sp!"));
                    }
                };
            });
        });
        rtn
    });

    trace!("Start running GUI.");

    let t = Timer::default();
    t.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {});

    if let Err(e) = ui.show() {
        let e = mk_err(e, "Failed to show app_window!");
        error!("{e}");
        we!("{e}");
    };

    if let Err(e) = slint::run_event_loop() {
        let e = mk_err(e, "Failed to run slint event loop!");
        error!("{e}");
        we!("{e}");
    };

    trace!("Application finished.");

    Ok(())
}
