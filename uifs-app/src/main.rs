slint::include_modules!();

mod logger;

use bytes::{Buf, BufMut, BytesMut};
use core::cell::RefCell;
use logger::Config;
use mimalloc::MiMalloc;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use slint::ModelRc;
use std::{cell::OnceCell, env, thread::sleep};
use tracing::{debug, error, info, trace, warn};
use uifs::{
  mk_err_str, slint_f, we, Rst, FRM_HEADER_LEN, FRM_PRESERVE_FLAG, FRM_START_FLAG, RX_SM3_RTN_LEN,
  SP_BAUD_RATE, SP_TIMEOUT, TX_MSG_MAX_LEN, TX_SM3_FLAG,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

thread_local! {
  static ALL_SPS_OC: OnceCell<Vec<SerialPortInfo>> = OnceCell::new();
  static CUR_SP_RC: RefCell<Option<Box<dyn SerialPort>>> = RefCell::new(None);
}

#[tokio::main]
async fn main() -> Rst<()> {
  let log_dir = env::var("UIFS_LOG_DIR").unwrap_or("./log".to_string());
  let log_to_file = env::var("UIFS_DIS_LOG_FILE").is_err();
  let log_to_cnsl = env::var("UIFS_ENBL_LOG_CNSL").is_ok();

  let log_conf = Config::new(&log_dir, log_to_file, log_to_cnsl);

  // the result should never be ignored or named `_` !!!
  let _guards = log_conf.init().await?;
  trace!(log_conf = ?log_conf, "Tracing Initialization finished.");

  #[cfg(debug_assertions)]
  debug!("Running on debug mode.");

  let app_ui = match AppWindow::new() {
    Ok(t) => t,
    Err(e) => {
      let e = mk_err_str(e, "Failed to initialize app_window!");
      error!("{e}");
      we!("{e}");
    }
  };

  match available_ports() {
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

      app_ui.set_sps(ModelRc::from(slint_sps.as_slice()));
      ALL_SPS_OC.with(|oc| oc.set(all_sps).unwrap());
      trace!("Serial ports initialization finished.");
    }
    Err(e) => {
      let e = mk_err_str(e, "Failed to get serial ports!");
      error!("{e}");
      we!("{e}");
    }
  };

  app_ui.on_sp_open(|sel_sp_idx| {
		let mut rst = false;
		ALL_SPS_OC.with(|oc| {
			CUR_SP_RC.with(|rc| {
				rc.take();
				let all_sps = oc.get().unwrap();
				let sel_sp = &all_sps[sel_sp_idx as usize];
				match serialport::new(sel_sp.port_name.as_str(), SP_BAUD_RATE)
					.timeout(SP_TIMEOUT)
					.open()
				{
					Ok(cur_sp) => {
						info!(sel_sp_idx = sel_sp_idx, sel_sp = ?sel_sp, "Successfully opened selected sp.");
						rc.replace(Some(cur_sp));
						rst = true;
					}
					Err(e) => {
						warn!(sel_sp_idx = sel_sp_idx, sel_sp = ?sel_sp, "{}", mk_err_str(e, "Failed to open selected sp!"));
					}
				};
			});
		});
		rst
	});

  app_ui.on_send_sm3(|msg| {
    if msg.len() > TX_MSG_MAX_LEN {
      warn!(msg_len = msg.len(), "Message is too long!");
      return slint_f!("消息过长！（大于 65408 位）\n");
    };

    let frm_len = (FRM_HEADER_LEN + msg.len()) as u16;
    let mut buf = BytesMut::with_capacity(frm_len as usize);
    buf.put_u8(FRM_START_FLAG);
    buf.put_u16(frm_len);
    buf.put_u8(TX_SM3_FLAG);
    buf.put_u8(FRM_PRESERVE_FLAG);
    buf.put_slice(msg.as_bytes());

    debug!(frm_head=?buf.split(), "Frame Header Building done.");

    CUR_SP_RC.with(|rc| {
      let mut cur_sp = rc.borrow_mut();
      let cur_sp = cur_sp.as_mut().unwrap();
      cur_sp.write_all(&buf[..]);
      cur_sp.flush();

      if let Err(e) = cur_sp.write_all(&buf[..]) {
        error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "Failed to send message to FPGA!"));
        return slint_f!("Failed to send message to FPGA!");
      };

      buf.clear();
      buf.resize(RX_SM3_RTN_LEN, 0);
      sleep(SP_TIMEOUT);

      match cur_sp.bytes_to_read() {
        Ok(rst_len) => {
          debug!(rst_len = rst_len, "Got bytes to read from FPGA.");
          match cur_sp.read(buf.as_mut()) {
            Ok(read_len) => {
              debug!(read_len = read_len, "Reading bytes finished.");
            }
            Err(e) => {
              error!("{}", mk_err_str(e, "Failed to read bytes!"));
              return slint_f!("Failed to read bytes!");
            }
          };

          if FRM_START_FLAG != buf.get_u8()
            || RX_SM3_RTN_LEN != buf.get_u16() as usize
            || TX_SM3_FLAG != buf.get_u8()
            || FRM_PRESERVE_FLAG != buf.get_u8()
          {
            error!(
                rtn_frame_head = ?buf[0..FRM_HEADER_LEN],
                "FPGA Backend returned invalid frame format!"
            );
            slint_f!("")
          } else {
            trace!(buf=?buf, "Successfully received the result.");
            const_hex::encode(&buf[FRM_HEADER_LEN..]).into()
          }
        }
        Err(e) => {
          error!("{}", mk_err_str(e, "Failed to get bytes to read!"));
          slint_f!("")
        }
      }
    })
  });

  trace!("Start running GUI.");

  if let Err(e) = app_ui.show() {
    let e = mk_err_str(e, "Failed to show app_window!");
    error!("{e}");
    we!("{e}");
  };

  if let Err(e) = slint::run_event_loop() {
    let e = mk_err_str(e, "Failed to run slint event loop!");
    error!("{e}");
    we!("{e}");
  };

  trace!("Application finished.");

  Ok(())
}
