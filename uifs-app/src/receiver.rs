use core::time::Duration;

use crate::{protocol::OpFlag, AppWindow, Options};
use bytes::{Buf, BytesMut};
use slint::{invoke_from_event_loop, ComponentHandle, Weak};
use tracing::debug;
use uifs_app::{
  FRM_HEAD_LEN, FRM_MAX_LEN, FRM_PRESERVE_FLAG, FRM_START_FLAG, FRM_TAIL_LEN, SM3_HASH_LEN,
};

pub async fn lsn_sp(mut sp: Box<dyn serialport::SerialPort>, weak_app: Weak<AppWindow>) {
  let mut buf = BytesMut::with_capacity(65536);
  loop {
    tokio::time::sleep(Duration::from_millis(100)).await;
    match sp.read(&mut buf) {
      Ok(0) => {}
      Ok(num) => {
        debug!(num = num, "num");
        if num < FRM_HEAD_LEN + FRM_TAIL_LEN || num > FRM_MAX_LEN {
          buf.clear();
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

        match tp.try_into().unwrap() {
          OpFlag::Key => {
            if 0 == buf.get_u8() && 1 == buf.get_u8() {
              let weak_app = weak_app.clone();
              invoke_from_event_loop(move || {
                weak_app.unwrap().global::<Options>().set_key_ready(true);
              });
            }
          }
          OpFlag::Sm3 => {
            if len != FRM_HEAD_LEN + SM3_HASH_LEN + FRM_TAIL_LEN {
              debug!("error sm3 len!");
              buf.clear();
              continue;
            }
            let mut hash = Vec::with_capacity(SM3_HASH_LEN);
            hash.clone_from_slice(&buf[FRM_HEAD_LEN..FRM_HEAD_LEN + SM3_HASH_LEN]);
            let weak_app = weak_app.clone();
            invoke_from_event_loop(move || {
              weak_app
                .unwrap()
                .global::<Options>()
                .invoke_append_dp_text(const_hex::encode(hash).into());
            });
          }
          OpFlag::Sm4Enc => {
            let weak_app = weak_app.clone();
            let mut ct = Vec::with_capacity(len - FRM_HEAD_LEN - FRM_TAIL_LEN);
            ct.clone_from_slice(&buf[FRM_HEAD_LEN..len - FRM_TAIL_LEN]);
            invoke_from_event_loop(move || {
              weak_app
                .unwrap()
                .global::<Options>()
                .invoke_append_dp_text(const_hex::encode(ct).into());
            });
          }
          OpFlag::Sm4Dec => {
            let weak_app = weak_app.clone();
            let mut pt = Vec::with_capacity(len - FRM_HEAD_LEN - FRM_TAIL_LEN);
            pt.clone_from_slice(&buf[FRM_HEAD_LEN..len - FRM_TAIL_LEN]);
            invoke_from_event_loop(move || {
              weak_app
                .unwrap()
                .global::<Options>()
                .invoke_append_dp_text(const_hex::encode(pt).into());
            });
          }
        }
        let weak_app = weak_app.clone();
        invoke_from_event_loop(move || {
          weak_app.unwrap().global::<Options>().invoke_append_dp_text("1".into());
        });
      }
      Err(_) => {}
    };
  }
}
