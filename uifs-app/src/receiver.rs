use core::time::Duration;

use crate::{mk_err_str, protocol::OpFlag, AppWindow, Options};
use bytes::{Buf, BytesMut};
use slint::{invoke_from_event_loop, ComponentHandle, Weak};
use tracing::debug;
use uifs_app::{
  FRM_HEAD_LEN, FRM_MAX_LEN, FRM_PRESERVE_FLAG, FRM_START_FLAG, FRM_TAIL_LEN, SM3_HASH_LEN,
};

pub async fn lsn_sp(mut sp: Box<dyn serialport::SerialPort>, weak_app: Weak<AppWindow>) {
  let mut buf = BytesMut::from([0u8; 65536].as_slice());
  debug!("Start to listen.");
  loop {
    tokio::time::sleep(Duration::from_millis(0)).await;
    if 0 == sp.bytes_to_read().unwrap() {
      continue;
    }
    match sp.read(&mut buf) {
      Ok(num) => {
        debug!(num = num, "num");
        debug!(buf = ?buf[..num], "buf");
        if num < FRM_HEAD_LEN + FRM_TAIL_LEN || num > FRM_MAX_LEN {
          continue;
        }
        if FRM_START_FLAG != buf.get_u8() {
          buf.clear();
          continue;
        }
        let len = buf.get_u16() as usize;
        debug!(len = len, "len");
        let tp = buf.get_u8();
        debug!(tp = tp, "tp");
        if FRM_PRESERVE_FLAG != buf.get_u8() {
          buf.clear();
          continue;
        }
        debug!(buf=?buf[..num]);
        match tp.try_into().unwrap() {
          OpFlag::Key => {
            if FRM_HEAD_LEN + 2 + FRM_TAIL_LEN != len {
              debug!("Incorrect response len!");
              buf.clear();
              continue;
            }
            if 0 == buf.get_u8() && 1 == buf.get_u8() {
              let weak_app = weak_app.clone();
              invoke_from_event_loop(move || {
                weak_app.unwrap().global::<Options>().set_key_ready(true);
              })
              .unwrap();
            } else {
              debug!("Incorrect key flag!");
              buf.clear();
              continue;
            }
          }
          OpFlag::Sm3 => {
            if len != FRM_HEAD_LEN + SM3_HASH_LEN + FRM_TAIL_LEN {
              debug!("Error sm3 len!");
              buf.clear();
              continue;
            }

            let mut hash = vec![0u8; SM3_HASH_LEN];
            hash.clone_from_slice(&buf[..SM3_HASH_LEN]);
            let weak_app = weak_app.clone();
            invoke_from_event_loop(move || {
              weak_app
                .unwrap()
                .global::<Options>()
                .invoke_append_dp_text(const_hex::encode(hash).into());
            })
            .unwrap();
          }
          OpFlag::Sm4Enc => {
            let weak_app = weak_app.clone();
            let mut ct = vec![0; len - FRM_HEAD_LEN - FRM_TAIL_LEN];
            ct.clone_from_slice(&buf[..len - FRM_HEAD_LEN - FRM_TAIL_LEN]);
            invoke_from_event_loop(move || {
              weak_app
                .unwrap()
                .global::<Options>()
                .invoke_append_dp_text(const_hex::encode(ct).into());
            })
            .unwrap();
          }
          OpFlag::Sm4Dec => {
            let weak_app = weak_app.clone();
            let mut pt = vec![0; len - FRM_HEAD_LEN - FRM_TAIL_LEN];
            pt.clone_from_slice(&buf[..len - FRM_HEAD_LEN - FRM_TAIL_LEN]);
            invoke_from_event_loop(move || {
              weak_app
                .unwrap()
                .global::<Options>()
                .invoke_append_dp_text(const_hex::encode(pt).into());
            })
            .unwrap();
          }
        }
      }
      Err(e) => {
        debug!("{}", mk_err_str(e, "Failed to read bytes!"));
      }
    };
  }
}
