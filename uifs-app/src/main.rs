slint::include_modules!();

mod logger;
mod protocol;
mod receiver;

use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use slint::{ModelRc, Weak, Window};
use std::env;
use tracing::{debug, error, info, trace, warn};
use uifs_app::{
  mk_err_str, slint_f, we, Opt, Rst, FRM_HEAD_LEN, FRM_PRESERVE_FLAG, FRM_START_FLAG,
  RX_SM3_RTN_LEN, SP_BAUD_RATE, SP_TIMEOUT, TX_MSG_MAX_LEN,
};

use crate::protocol::{key, sm3, sm4_dec_cbc, sm4_dec_ecb, sm4_enc_cbc, sm4_enc_ecb};

use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use core::cell::{OnceCell, RefCell};
thread_local! {
  static ALL_SPS: OnceCell<Vec<SerialPortInfo>> = OnceCell::new();
  static CUR_LSN_HNDLR: RefCell<Opt<tokio::task::JoinHandle<()>>> = RefCell::new(None);
  static WEAK_APP: OnceCell<Weak<AppWindow>> = OnceCell::new();
}

use core::sync::atomic::AtomicI32;
static CUR_SP_IDX: AtomicI32 = AtomicI32::new(-1);

#[tokio::main(worker_threads = 1)]
async fn main() -> Rst<()> {
  // !!! the result should never be ignored or named `_` !!!
  let _guards = {
    let log_dir = env::var("UIFS_LOG_DIR").unwrap_or("./log".to_string());
    let log_to_file = env::var("UIFS_DIS_LOG_FILE").is_err();
    let log_to_cnsl = env::var("UIFS_ENBL_LOG_CNSL").is_ok();
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

  // app.global::<Options>().on_key_send(|k| {
  //   let bytes = k.as_bytes();
  //   if 16 != bytes.len() {
  //     warn!(key_len = bytes.len(), "Incorrect key length!");
  //     return false;
  //   }
  //   if let Err(e) = const_hex::check(bytes) {
  //     warn!(key = ?k, "{}", mk_err_str(e, "Incorrect key format!"));
  //     return false;
  //   }
  //   let send_key = key(bytes.try_into().unwrap());

  //   CUR_SP_RC.with(|rc| {
  //     let mut cur_sp = rc.borrow_mut();
  //     let cur_sp = cur_sp.as_mut().unwrap();

  //     if let Err(e) = cur_sp.write_all(&send_key.as_ref()) {
  //       error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send key to FPGA!"));
  //       return false;
  //     };

  //     sleep(SP_TIMEOUT);

  //     match cur_sp.bytes_to_read() {
  //       Ok(rst_len) => {
  //         debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
  //         let mut buf = BytesMut::with_capacity(RX_SM3_RTN_LEN);
  //         match cur_sp.read(buf.as_mut()) {
  //           Ok(read_len) => {
  //             debug!(read_len = read_len, "Reading bytes finished.");
  //           }
  //           Err(e) => {
  //             error!("{}", mk_err_str(e, "Failed to read bytes!"));
  //             return false;
  //           }
  //         };

  //         if FRM_START_FLAG != buf.get_u8()
  //           || 9 != buf.get_u16() as usize
  //           || protocol::OpFlag::Key as u8 != buf.get_u8()
  //           || FRM_PRESERVE_FLAG != buf.get_u8()
  //         {
  //           error!(
  //               rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
  //               "FPGA Backend returned invalid frame format!"
  //           );
  //           false
  //         } else {
  //           trace!(buf=?buf, "Successfully received the result.");
  //           true
  //         }
  //       }
  //       Err(e) => {
  //         error!("{}", mk_err_str(e, "Failed to get bytes to read!"));
  //         false
  //       }
  //     }f
  //   })
  // });

  // app.global::<Options>().on_send_sm3(|msg| {
  //   if msg.len() > TX_MSG_MAX_LEN {
  //     warn!(msg_len = msg.len(), "Message is too long!");
  //     return slint_f!("消息过长！（大于 65408 位）\n");
  //   };

  //   let send_msg = sm3(msg.as_bytes());

  //   CUR_SP_RC.with(|rc| {
  //     let mut cur_sp = rc.borrow_mut();
  //     let cur_sp = cur_sp.as_mut().unwrap();

  //     if let Err(e) = cur_sp.write_all(&send_msg.as_ref()) {
  //       error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
  //       return slint_f!("Failed to send message to FPGA!");
  //     };

  //     sleep(SP_TIMEOUT);

  //     match cur_sp.bytes_to_read() {
  //       Ok(rst_len) => {
  //         debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
  //         let mut buf = BytesMut::with_capacity(RX_SM3_RTN_LEN);
  //         match cur_sp.read(buf.as_mut()) {
  //           Ok(read_len) => {
  //             debug!(read_len = read_len, "Reading bytes finished.");
  //           }
  //           Err(e) => {
  //             let e = mk_err_str(e, "Failed to read bytes!");
  //             error!("{e}");
  //             return slint_f!("{e}");
  //           }
  //         };

  //         if FRM_START_FLAG != buf.get_u8()
  //           || RX_SM3_RTN_LEN != buf.get_u16() as usize
  //           || protocol::OpFlag::Sm3 as u8 != buf.get_u8()
  //           || FRM_PRESERVE_FLAG != buf.get_u8()
  //         {
  //           error!(
  //               rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
  //               "FPGA Backend returned invalid frame format!"
  //           );
  //           slint_f!("FPGA Backend returned invalid frame format!")
  //         } else {
  //           trace!(buf=?buf, "Successfully received the result.");
  //           const_hex::encode(&buf[FRM_HEADER_LEN..buf.len() - 2]).into()
  //         }
  //       }
  //       Err(e) => {
  //         let e = mk_err_str(e, "Failed to get bytes to read!");
  //         error!("{e}");
  //         slint_f!("{e}")
  //       }
  //     }
  //   })
  // });

  // app.global::<Options>().on_send_sm4e_cbc(|pt, iv| {
  //   if 16 != iv.len() {
  //     let e = "Incorrect iv length!";
  //     error!("{e}");
  //     return slint_f!("{e}");
  //   }
  //   let send_pt = sm4_enc_cbc(pt.as_bytes().try_into().unwrap(), iv.as_bytes().try_into().unwrap());

  //   CUR_SP_RC.with(|rc| {
  //     let mut cur_sp = rc.borrow_mut();
  //     let cur_sp = cur_sp.as_mut().unwrap();

  //     if let Err(e) = cur_sp.write_all(&send_pt) {
  //       let e = mk_err_str(e, "Failed to send message to FPGA!");
  //       error!(cur_sp = ?cur_sp, "{e}");
  //       return slint_f!("{e}");
  //     };

  //     sleep(SP_TIMEOUT);

  //     match cur_sp.bytes_to_read() {
  //       Ok(rst_len) => {
  //         debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
  //         let mut buf = BytesMut::with_capacity(rst_len as usize);
  //         match cur_sp.read(buf.as_mut()) {
  //           Ok(read_len) => {
  //             debug!(read_len = read_len, "Reading bytes finished.");
  //           }
  //           Err(e) => {
  //             let e = mk_err_str(e, "Failed to read bytes!");
  //             error!("{e}");
  //             return slint_f!("{e}");
  //           }
  //         };

  //         if FRM_START_FLAG != buf.get_u8()
  //           || FRM_HEADER_LEN < buf.get_u16() as usize
  //           || protocol::OpFlag::Sm4Enc as u8 != buf.get_u8()
  //           || FRM_PRESERVE_FLAG != buf.get_u8()
  //         {
  //           error!(
  //               rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
  //               "FPGA Backend returned invalid frame format!"
  //           );
  //           slint_f!("FPGA Backend returned invalid frame format!")
  //         } else {
  //           trace!(buf=?buf, "Successfully received the result.");
  //           const_hex::encode(&buf[FRM_HEADER_LEN..buf.len() - 2]).into()
  //         }
  //       }
  //       Err(e) => {
  //         error!("{}", mk_err_str(e, "Failed to get bytes to read!"));
  //         slint_f!("")
  //       }
  //     }
  //   })
  // });

  // app.global::<Options>().on_send_sm4e_ecb(|pt| {
  //   let ct = sm4_enc_ecb(pt.as_bytes());

  //   CUR_SP_RC.with(|rc| {
  //     let mut cur_sp = rc.borrow_mut();
  //     let cur_sp = cur_sp.as_mut().unwrap();

  //     if let Err(e) = cur_sp.write_all(&ct) {
  //       error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
  //       return slint_f!("Failed to send message to FPGA!");
  //     };

  //     sleep(SP_TIMEOUT);

  //     match cur_sp.bytes_to_read() {
  //       Ok(rst_len) => {
  //         debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
  //         let mut buf = BytesMut::with_capacity(rst_len as usize);
  //         match cur_sp.read(buf.as_mut()) {
  //           Ok(read_len) => {
  //             debug!(read_len = read_len, "Reading bytes finished.");
  //           }
  //           Err(e) => {
  //             let e = mk_err_str(e, "Failed to read bytes!");
  //             error!("{e}");
  //             return slint_f!("{e}");
  //           }
  //         };

  //         if FRM_START_FLAG != buf.get_u8()
  //           || FRM_HEADER_LEN < buf.get_u16() as usize
  //           || protocol::OpFlag::Sm4Dec as u8 != buf.get_u8()
  //           || FRM_PRESERVE_FLAG != buf.get_u8()
  //         {
  //           error!(
  //               rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
  //               "FPGA Backend returned invalid frame format!"
  //           );
  //           slint_f!("FPGA Backend returned invalid frame format!")
  //         } else {
  //           trace!(buf=?buf, "Successfully received the result.");
  //           const_hex::encode(&buf[FRM_HEADER_LEN..buf.len() - 2]).into()
  //         }
  //       }
  //       Err(e) => {
  //         error!("{}", mk_err_str(e, "Failed to get bytes to read!"));
  //         slint_f!("")
  //       }
  //     }
  //   })
  // });

  // app.global::<Options>().on_send_sm4d_cbc(|ct, iv| {
  //   let ct = sm4_dec_cbc(ct.as_bytes(), iv.as_bytes().try_into().unwrap());

  //   CUR_SP_RC.with(|rc| {
  //     let mut cur_sp = rc.borrow_mut();
  //     let cur_sp = cur_sp.as_mut().unwrap();

  //     if let Err(e) = cur_sp.write_all(&ct) {
  //       error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
  //       return slint_f!("Failed to send message to FPGA!");
  //     };

  //     sleep(SP_TIMEOUT);

  //     match cur_sp.bytes_to_read() {
  //       Ok(rst_len) => {
  //         debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
  //         let mut buf = BytesMut::with_capacity(rst_len as usize);
  //         match cur_sp.read(buf.as_mut()) {
  //           Ok(read_len) => {
  //             debug!(read_len = read_len, "Reading bytes finished.");
  //           }
  //           Err(e) => {
  //             let e = mk_err_str(e, "Failed to read bytes!");
  //             error!("{e}");
  //             return slint_f!("{e}");
  //           }
  //         };

  //         if FRM_START_FLAG != buf.get_u8()
  //           || FRM_HEADER_LEN < buf.get_u16() as usize
  //           || protocol::OpFlag::Sm4Dec as u8 != buf.get_u8()
  //           || FRM_PRESERVE_FLAG != buf.get_u8()
  //         {
  //           error!(
  //               rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
  //               "FPGA Backend returned invalid frame format!"
  //           );
  //           slint_f!("FPGA Backend returned invalid frame format!")
  //         } else {
  //           trace!(buf=?buf, "Successfully received the result.");
  //           const_hex::encode(&buf[FRM_HEADER_LEN..buf.len() - 2]).into()
  //         }
  //       }trace
  //         error!("{}", mk_err_str(e, "Failed to get bytes to read!"));
  //         slint_f!("")
  //       }
  //     }
  //   })
  // });

  // app.global::<Options>().on_send_sm4d_ecb(|ct| {
  //   let ct = sm4_dec_ecb(ct.as_bytes());

  //   CUR_SP_RC.with(|rc| {
  //     let mut cur_sp = rc.borrow_mut();
  //     let cur_sp = cur_sp.as_mut().unwrap();

  //     if let Err(e) = cur_sp.write_all(&ct) {
  //       error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
  //       return slint_f!("Failed to send message to FPGA!");
  //     };

  //     sleep(SP_TIMEOUT);

  //     match cur_sp.bytes_to_read() {
  //       Ok(rst_len) => {
  //         debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
  //         let mut buf = BytesMut::with_capacity(rst_len as usize);
  //         match cur_sp.read(buf.as_mut()) {
  //           Ok(read_len) => {
  //             debug!(read_len = read_len, "Reading bytes finished.");
  //           }
  //           Err(e) => {
  //             let e = mk_err_str(e, "Failed to read bytes!");
  //             error!("{e}");
  //             return slint_f!("{e}");
  //           }
  //         };

  //         if FRM_START_FLAG != buf.get_u8()
  //           || FRM_HEADER_LEN < buf.get_u16() as usize
  //           || protocol::OpFlag::Sm4Dec as u8 != buf.get_u8()
  //           || FRM_PRESERVE_FLAG != buf.get_u8()
  //         {
  //           error!(
  //               rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
  //               "FPGA Backend returned invalid frame format!"
  //           );
  //           slint_f!("FPGA Backend returned invalid frame format!")
  //         } else {
  //           trace!(buf=?buf, "Successfully received the result.");
  //           const_hex::encode(&buf[FRM_HEADER_LEN..buf.len() - 2]).into()
  //         }
  //       }
  //       Err(e) => {
  //         error!("{}", mk_err_str(e, "Failed to get bytes to read!"));
  //         slint_f!("")
  //       }
  //     }
  //   })
  // });

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

mod test {
  use core::time::Duration;

  use bytes::BytesMut;

  #[tokio::test]
  async fn name() {
    let mut buf = BytesMut::with_capacity(65536);
    loop {
      tokio::time::sleep(Duration::from_secs(1)).await;
      let mut sp = serialport::new("COM3", 115_200).open().unwrap();
      match sp.read(&mut buf) {
        Ok(0) => {
          eprintln!("0");
          panic!();
        }
        Ok(num) => {
          eprint!("{num}");
          panic!();
        }
        Err(e) => {
          panic!("{e}");
        }
      }
    }
  }
}
