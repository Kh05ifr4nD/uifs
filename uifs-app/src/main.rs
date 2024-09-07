slint::include_modules!();

mod logger;
mod protocol;
mod receiver;

use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use slint::{ModelRc, Weak};

use tracing::{debug, error, info, trace, warn};
use uifs_app::{mk_err_str, slint_f, we, Opt, Rst, SP_BAUD_RATE, SP_TIMEOUT, TX_MSG_MAX_LEN};

use crate::protocol::{key, sm3, sm4_dec_cbc, sm4_dec_ecb, sm4_enc_cbc, sm4_enc_ecb};

use core::cell::{OnceCell, RefCell};
thread_local! {
  static ALL_SPS: OnceCell<Vec<SerialPortInfo>> = OnceCell::new();
  static CUR_LSN_HNDLR: RefCell<Opt<tokio::task::JoinHandle<()>>> = RefCell::new(None);
  static CUR_SP: RefCell<Opt<Box<dyn SerialPort>>> = RefCell::new(None);
  static WEAK_APP: OnceCell<Weak<AppWindow>> = OnceCell::new();
}

use core::sync::atomic::AtomicI32;
static CUR_SP_IDX: AtomicI32 = AtomicI32::new(-1);

#[tokio::main(worker_threads = 1)]
async fn main() -> Rst<()> {
  // !!! the result should never be ignored or named `_` !!!
  let _guards = {
    use std::env::var;
    let log_dir = var("UIFS_LOG_DIR").unwrap_or("./log".to_string());
    let log_to_file = var("UIFS_DIS_LOG_FILE").is_err();
    let log_to_cnsl = var("UIFS_ENBL_LOG_CNSL").is_ok();
    logger::Config::new(&log_dir, log_to_file, log_to_cnsl).init().await?
  };

  trace!("Tracing Initialization finished.");
  #[cfg(debug_assertions)]
  debug!("Running on debug mode.");

  let app = match AppWindow::new() {
    Ok(t) => t,
    Err(e) => {
      let e = mk_err_str(e, "Failed to initialize app_window!");
      error!("{e}");
      we!("{e}");
    }
  };

  WEAK_APP.with(|weak| {
    if let Err(_) = weak.set(app.as_weak()) {
      error!("Failed to get weak pointer!");
      panic!("Failed to get weak pointer!");
    };
  });

  match serialport::available_ports() {
    Ok(all_sps) => {
      debug!(all_sps = ?all_sps, "Got available serial ports.");
      let slint_sps: Vec<_> = all_sps
        .iter()
        .map(|sp| {
          if let SerialPortType::UsbPort(_) = sp.port_type {
            slint_f!("{} UsbPort", sp.port_name)
          } else {
            slint_f!("{} {:?}", sp.port_name, sp.port_type)
          }
        })
        .collect();

      app.global::<Options>().set_sps(ModelRc::from(slint_sps.as_slice()));
      ALL_SPS.with(|oc| oc.set(all_sps).unwrap());
      trace!("Serial ports initialization finished.");
    }
    Err(e) => {
      let e = mk_err_str(e, "Failed to get serial ports!");
      error!("{e}");
      we!("{e}");
    }
  };

  app.global::<Options>().on_sp_open(|sel_sp_idx| {
    use core::sync::atomic::Ordering::Relaxed;
    if sel_sp_idx == CUR_SP_IDX.load(Relaxed) {
      return true;
    };

    let mut rst = false;

    ALL_SPS.with(|oc| {
      let all_sps = oc.get().unwrap();
      let sel_sp = &all_sps[sel_sp_idx as usize];
      match serialport::new(sel_sp.port_name.as_str(), SP_BAUD_RATE)
        .timeout(SP_TIMEOUT)
        .open()
      {
        Ok(cur_sp) => {
          let replace_sp = cur_sp.try_clone().unwrap();
          CUR_SP.with_borrow_mut(|sp|{
            sp.replace(replace_sp);
          });
          info!(sel_sp_idx = sel_sp_idx, sel_sp = ?sel_sp, "Successfully opened selected sp.");
          WEAK_APP.with(|w|{
            let w = w.get().unwrap().clone();
            CUR_LSN_HNDLR.with(|hndlr|{
              let lsn_task = tokio::spawn(receiver::lsn_sp(cur_sp,w));
              hndlr.replace(Some(lsn_task)).inspect(|h|h.abort());
            });
          });

          CUR_SP_IDX.store(sel_sp_idx, Relaxed);
          rst = true;
        }
        Err(e) => {
          warn!(sel_sp_idx = sel_sp_idx, sel_sp = ?sel_sp, "{}", mk_err_str(e, "Failed to open selected sp!"));
        }
      };
    });
    rst
  });

  app.global::<Options>().on_key_send(|k| {
    let bytes = k.as_bytes();
    if 16 != bytes.len() {
      warn!(key_len = bytes.len(), "Incorrect key length!");
      return;
    }
    if let Err(e) = const_hex::check(bytes) {
      warn!(key = ?k, "{}", mk_err_str(e, "Incorrect key format!"));
      return;
    }

    let send_key = key(bytes.try_into().unwrap());
    debug!(send_key=?send_key);

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_key.as_ref()) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send key to FPGA!"));
      };
    })
  });

  app.global::<Options>().on_send_test(|msg| {
    info!(msg = msg.as_str(), "msg");
    if msg.len() > TX_MSG_MAX_LEN {
      warn!(msg_len = msg.len(), "Message is too long!");
      return;
    };
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(msg.as_bytes()) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
      };
    });
  });

  app.global::<Options>().on_send_sm3(|msg| {
    if msg.len() > TX_MSG_MAX_LEN {
      warn!(msg_len = msg.len(), "Message is too long!");
      return;
    };

    let send_msg = sm3(msg.as_bytes());
    debug!(send_msg=?send_msg.as_ref());

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(send_msg.as_ref()) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
      };
    });
  });

  app.global::<Options>().on_send_sm4e_cbc(|pt, iv| {
    if 16 != iv.len() {
      let e = "Incorrect iv length!";
      error!("{e}");
      return;
    };

    let send_pt = sm4_enc_cbc(pt.as_bytes().try_into().unwrap(), iv.as_bytes().try_into().unwrap());
    debug!(send_pt=?send_pt);
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_pt) {
        let e = mk_err_str(e, "Failed to send message to FPGA!");
        warn!(cur_sp = ?cur_sp, "{e}");
      };
    })
  });

  app.global::<Options>().on_send_sm4e_ecb(|pt| {
    let send_pt = sm4_enc_ecb(pt.as_bytes());
    debug!(send_pt=?send_pt);

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_pt) {
        error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
      };
    })
  });

  app.global::<Options>().on_send_sm4d_cbc(|ct, iv| {
    if 16 != iv.len() {
      let e = "Incorrect iv length!";
      error!("{e}");
      return;
    };
    let send_ct = sm4_dec_cbc(ct.as_bytes(), iv.as_bytes().try_into().unwrap());
    debug!(send_ct=?send_ct);
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_ct) {
        error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
      };
    })
  });

  app.global::<Options>().on_send_sm4d_ecb(|ct| {
    let send_ct = sm4_dec_ecb(ct.as_bytes());
    debug!(send_ct=?send_ct);
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_ct) {
        error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
      };
    })
  });

  trace!("Start running GUI.");

  if let Err(e) = app.show() {
    let e = mk_err_str(e, "Failed to show app_window!");
    error!("{e}");
    we!("{e}");
  };

  if let Err(e) = slint::run_event_loop() {
    let e = mk_err_str(e, "Failed to run slint event loop!");
    error!("{e}");
    we!("{e}");
  };

  debug!("Application finished.");

  Ok(())
}
