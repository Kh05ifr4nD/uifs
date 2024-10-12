#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
slint::include_modules!();

mod logger;
mod protocol;
mod receiver;

use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use slint::{invoke_from_event_loop, ModelRc, Weak};

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
use core::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::AtomicU8;
static CUR_SP_IDX: AtomicI32 = AtomicI32::new(-1);
static CUR_MODE: AtomicU8 = AtomicU8::new(0);

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
      let e = mk_err_str(e, "应用窗口初始化失败");
      error!("{e}");
      we!("{e}");
    }
  };

  WEAK_APP.with(|weak| {
    let _ = weak.set(app.as_weak());
  });

  match serialport::available_ports() {
    Ok(all_sps) => {
      debug!(all_sps = ?all_sps, "可用串口查询成功");
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
      trace!("串口列表设置成功");
    }
    Err(e) => {
      let e = mk_err_str(e, "无法获取可用串口");
      error!("{e}");
      we!("{e}");
    }
  };

  app.global::<Options>().on_sp_open(|sel_sp_idx| {
    if sel_sp_idx == CUR_SP_IDX.load(Relaxed) {
      return true;
    };

    let mut rst = false;

    ALL_SPS.with(|oc| {
      let all_sps = oc.get().unwrap();
      let sel_sp = &all_sps[sel_sp_idx as usize];
      match serialport::new(sel_sp.port_name.as_str(), SP_BAUD_RATE).timeout(SP_TIMEOUT).open() {
        Ok(cur_sp) => {
          let replace_sp = cur_sp.try_clone().unwrap();
          CUR_SP.with_borrow_mut(|sp| {
            sp.replace(replace_sp);
          });
          info!(sel_sp_idx = sel_sp_idx, sel_sp = ?sel_sp, "成功打开选择的串口");
          WEAK_APP.with(|w| {
            let w = w.get().unwrap().clone();
            CUR_LSN_HNDLR.with(|hndlr| {
              let lsn_task = tokio::spawn(receiver::lsn_sp(cur_sp, w));
              hndlr.replace(Some(lsn_task)).inspect(|h| h.abort());
            });
          });

          CUR_SP_IDX.store(sel_sp_idx, Relaxed);
          rst = true;
        }
        Err(e) => {
          warn!(sel_sp_idx = sel_sp_idx, sel_sp = ?sel_sp, "{}", mk_err_str(e, "无法打开此串口"));
        }
      };
    });
    rst
  });

  app.global::<Options>().on_lsn(|| {
    if 0 != CUR_MODE.load(Relaxed) {
      CUR_SP.with_borrow_mut(|sp| {
        if let Some(sp) = sp.as_mut() {
          let sp = sp.try_clone().unwrap();
          WEAK_APP.with(|w| {
            let w = w.get().unwrap().clone();
            CUR_LSN_HNDLR.with(|hndlr| {
              let lsn_task = tokio::spawn(receiver::lsn_sp(sp, w));
              hndlr.replace(Some(lsn_task)).inspect(|h| h.abort());
            });
          });
          CUR_MODE.store(0, Relaxed);
        }
      });
    }
  });

  app.global::<Options>().on_parse(|chat| {
    CUR_SP.with_borrow_mut(|sp| {
      if let Some(sp) = sp.as_mut() {
        let sp = sp.try_clone().unwrap();
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          CUR_LSN_HNDLR.with(|hndlr| {
            let lsn_task = tokio::spawn(receiver::parse_sp(sp, w, chat));
            hndlr.replace(Some(lsn_task)).inspect(|h| h.abort());
          });
        });
        CUR_MODE.store(1, Relaxed);
      }
    });
  });

  app.global::<Options>().on_obsr(|| {
    if 2 != CUR_MODE.load(Relaxed) {
      CUR_SP.with_borrow_mut(|sp| {
        if let Some(sp) = sp.as_mut() {
          let sp = sp.try_clone().unwrap();
          WEAK_APP.with(|w| {
            let w = w.get().unwrap().clone();
            CUR_LSN_HNDLR.with(|hndlr| {
              let lsn_task = tokio::spawn(receiver::obsr_sp(sp, w));
              hndlr.replace(Some(lsn_task)).inspect(|h| h.abort());
            });
          });
          CUR_MODE.store(2, Relaxed);
        }
      });
    }
  });

  app.global::<Options>().on_key_send(|k| {
    let bytes = k.as_bytes();
    if 32 != bytes.len() {
      warn!(key_len = bytes.len(), "输入密钥长不为 128 位");
      return;
    }
    if let Err(e) = const_hex::check(bytes) {
      warn!(key = ?k, "{}", mk_err_str(e, "出现非密钥字符"));
      return;
    }
    let bytes = const_hex::decode(k.clone()).unwrap();

    let send_key = key(bytes.as_slice().try_into().unwrap());

    debug!(send_key = ?send_key, "发送密钥");

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_key.as_ref()) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "密钥发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!("{}：{}", "密钥注入", k));
          })
          .unwrap();
        })
      }
    })
  });

  app.global::<Options>().on_send_test(|msg| {
    if msg.len() > TX_MSG_MAX_LEN {
      warn!(msg_len = msg.len(), "消息过长");
      return;
    };
    debug!(msg = ?msg.as_bytes(), "发送测试消息");
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(msg.as_bytes()) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "测试消息发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              "测试消息",
              msg
            ));
          })
          .unwrap();
        })
      }
    });
  });

  app.global::<Options>().on_send_sm3(|msg| {
    if msg.len() > TX_MSG_MAX_LEN {
      warn!(msg_len = msg.len(), "消息过长");
      return;
    };

    let send_msg = sm3(msg.as_bytes());
    info!(send_msg = ?send_msg, "发送 SM3 消息");

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(send_msg.as_ref()) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "消息发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              "消息",
              msg
            ));
          })
          .unwrap();
        })
      }
    });
  });

  app.global::<Options>().on_send_sm4e_cbc(|pt, iv| {
    if 32 != iv.len() {
      warn!(iv = ?iv, "输入初始向量长不为 128 位");
      return;
    };
    let iv = const_hex::decode(iv).unwrap();

    let send_pt = sm4_enc_cbc(pt.as_bytes().try_into().unwrap(), iv.as_slice().try_into().unwrap());
    debug!(send_pt =? send_pt);
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_pt) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "CBC 加密请求发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              "明文",
              pt
            ));
          })
          .unwrap();
        })
      }
    })
  });

  app.global::<Options>().on_send_sm4e_ecb(|pt| {
    let send_pt = sm4_enc_ecb(pt.as_bytes());
    debug!(send_pt =? send_pt[..]);

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_pt) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "ECB 加密请求发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              "明文",
              pt
            ));
          })
          .unwrap();
        })
      };
    })
  });

  app.global::<Options>().on_chat_send_sm4e_ecb(|pt| {
    let send_pt = sm4_enc_ecb(pt.as_bytes());
    debug!(send_pt =? send_pt[..]);

    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_pt) {
        warn!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "ECB 加密请求发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              if w.unwrap().global::<Options>().get_name() { "Bob" } else { "Alice" },
              pt
            ));
          })
          .unwrap();
        })
      };
    })
  });

  app.global::<Options>().on_send_sm4d_cbc(|ct, iv| {
    if 32 != iv.len() {
      let e = "输入初始向量长不为 128 位";
      warn!("{e}");
      return;
    };
    let ct_bytes = const_hex::decode(ct.clone()).unwrap();
    let iv = const_hex::decode(iv).unwrap();
    let send_ct = sm4_dec_cbc(ct_bytes.as_slice(), iv.as_slice().try_into().unwrap());
    debug!(send_ct =? send_ct);
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_ct) {
        error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "CBC 解密请求发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              "密文",
              ct
            ));
          })
          .unwrap();
        })
      }
    })
  });

  app.global::<Options>().on_send_sm4d_ecb(|ct| {
    let ct_bytes = const_hex::decode(ct.clone()).unwrap();
    let send_ct = sm4_dec_ecb(ct_bytes.as_slice());
    debug!(send_ct =? send_ct);
    CUR_SP.with_borrow_mut(|cur_sp| {
      if let Err(e) = cur_sp.as_mut().unwrap().write_all(&send_ct) {
        error!(cur_sp = ?cur_sp, "{}", mk_err_str(e, "ECB 解密请求发送失败"));
      } else {
        WEAK_APP.with(|w| {
          let w = w.get().unwrap().clone();
          invoke_from_event_loop(move || {
            w.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
              "{}：{}",
              "密文",
              ct
            ));
          })
          .unwrap();
        })
      }
    })
  });

  if let Err(e) = app.show() {
    let e = mk_err_str(e, "窗口打开失败");
    error!("{e}");
    we!("{e}");
  };

  trace!("开始运行 GUI");

  if let Err(e) = slint::run_event_loop() {
    let e = mk_err_str(e, "事件循环运行失败");
    error!("{e}");
    we!("{e}");
  };

  trace!("应用结束");

  Ok(())
}
