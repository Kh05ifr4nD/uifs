use core::time::Duration;
use std::time::SystemTime;

use crate::{AppWindow, Options};
use slint::{invoke_from_event_loop, ComponentHandle, Weak};
use uifs_app::SlintStr;

pub async fn lsn_sp(mut sp: Box<dyn serialport::SerialPort>, weak_app: Weak<AppWindow>) {
  let mut buf = [0u8; 1 << 16];
  loop {
    tokio::time::sleep(Duration::from_millis(100)).await;
    match sp.read(&mut buf) {
      Ok(num) => {
        let weak_app = weak_app.clone();
        invoke_from_event_loop(move || {
          weak_app.unwrap().global::<Options>().invoke_update_dp_text("1".into());
        });
      }
      Err(_) => {}
    };
  }
}
