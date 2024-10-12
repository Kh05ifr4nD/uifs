use core::time::Duration;

use crate::{
  mk_err_str,
  protocol::{BlockMode, OpFlag},
  AppWindow, Options,
};
use bytes::BytesMut;
use slint::{invoke_from_event_loop, ComponentHandle, Weak};
use tracing::{debug, info, warn};
use uifs_app::{
  slint_f, FRM_HEAD_LEN, FRM_MAX_LEN, FRM_MIN_LEN, FRM_START_FLAG, FRM_TAIL_LEN, SM3_HASH_LEN,
};

async fn handle_key_response(frame_length: usize, no_head_frame: &[u8], weak_app: Weak<AppWindow>) {
  info!("密钥注入响应");
  if FRM_HEAD_LEN + 1 + FRM_TAIL_LEN != frame_length {
    warn!("帧长度有误");
    return;
  }
  if 0x01 != no_head_frame[0] {
    warn!("意外标识：{}", no_head_frame[0]);
    return;
  }
  invoke_from_event_loop(move || {
    weak_app.unwrap().global::<Options>().set_key_ready(true);
    weak_app.unwrap().global::<Options>().invoke_append_dp_text(slint_f!("密钥注入成功"));
  })
  .unwrap();
}

async fn handle_sm3_response(frame_length: usize, no_head_frame: &[u8], weak_app: Weak<AppWindow>) {
  info!("SM3 散列响应");
  if FRM_HEAD_LEN + SM3_HASH_LEN + FRM_TAIL_LEN != frame_length {
    warn!("帧长度有误");
    return;
  }

  let mut hash = vec![0u8; SM3_HASH_LEN];
  hash.clone_from_slice(&no_head_frame[..no_head_frame.len() - FRM_TAIL_LEN]);
  invoke_from_event_loop(move || {
    weak_app
      .unwrap()
      .global::<Options>()
      .invoke_append_dp_text(slint_f!("SM3 结果：{}", const_hex::encode(hash)));
  })
  .unwrap();
}

async fn handle_sm4_enc_response(
  frame_length: usize,
  no_head_frame: &[u8],
  weak_app: Weak<AppWindow>,
  chat: bool,
  mode: BlockMode,
) {
  let no_head_frame = Vec::from(no_head_frame);
  invoke_from_event_loop(move || {
    info!("SM4 加密响应");
    let mut ct = vec![0; frame_length - FRM_HEAD_LEN - FRM_TAIL_LEN];
    ct.clone_from_slice(&no_head_frame[..no_head_frame.len() - FRM_TAIL_LEN]);
    while Some(&0) == ct.last() {
      ct.pop();
    }
    weak_app.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
      "{}：{}",
      if chat {
        if weak_app.unwrap().global::<Options>().get_name() {
          "Alice"
        } else {
          "Bob"
        }
      } else {
        if mode == BlockMode::CBC {
          "SM4 加密结果（CBC模式）"
        } else {
          "SM4 加密结果（ECB模式）"
        }
      },
      if chat {
        String::from_utf8_lossy(ct.as_slice()).into_owned()
      } else {
        const_hex::encode(ct)
      }
    ));
  })
  .unwrap();
}

async fn handle_sm4_dec_response(
  frame_length: usize,
  no_head_frame: &[u8],
  weak_app: Weak<AppWindow>,
  chat: bool,
  mode: BlockMode,
) {
  let no_head_frame = Vec::from(no_head_frame);
  invoke_from_event_loop(move || {
    let name = weak_app.unwrap().global::<Options>().get_name();
    let text = if chat {
      if name {
        "Alice"
      } else {
        "Bob"
      }
    } else {
      if mode == BlockMode::CBC {
        "SM4 解密结果（CBC模式）"
      } else {
        "SM4 解密结果（ECB模式）"
      }
    };
    info!("SM4 解密响应");
    let mut pt = vec![0; frame_length - FRM_HEAD_LEN - FRM_TAIL_LEN];
    pt.clone_from_slice(&no_head_frame[..no_head_frame.len() - FRM_TAIL_LEN]);
    while Some(&0) == pt.last() {
      pt.pop();
    }
    weak_app.unwrap().global::<Options>().invoke_append_dp_text(slint_f!(
      "{}：{}",
      text,
      String::from_utf8_lossy(pt.as_slice())
    ));
  })
  .unwrap();
}

pub async fn lsn_sp(mut sp: Box<dyn serialport::SerialPort>, weak_app: Weak<AppWindow>) {
  debug!("监听端口中……（回显）");
  loop {
    tokio::time::sleep(Duration::from_millis(0)).await;
    let btor = sp.bytes_to_read().unwrap() as usize;
    if 0 == btor {
      continue;
    }
    debug!("有 {} 字节数据可读", btor);
    let mut buf = vec![0u8; btor];
    if let Err(e) = sp.read_exact(&mut buf) {
      mk_err_str(e, "读取串口数据失败");
    };
    debug!("已读取数据：{:?}", const_hex::encode(&buf));
    let weak_app = weak_app.clone();
    invoke_from_event_loop(move || {
      weak_app
        .unwrap()
        .global::<Options>()
        .invoke_append_dp_text(slint_f!("回显：{}", String::from_utf8_lossy(&buf)));
    })
    .unwrap();
  }
}

pub async fn parse_sp(
  mut sp: Box<dyn serialport::SerialPort>,
  weak_app: Weak<AppWindow>,
  chat: bool,
) {
  let mut buf = BytesMut::with_capacity(2 << 20);
  debug!("监听端口中……（解析）");
  loop {
    tokio::time::sleep(Duration::from_millis(0)).await;
    {
      let btor = sp.bytes_to_read().unwrap() as usize;
      if 0 == btor {
        continue;
      }
      debug!("有 {} 字节数据可读", btor);
      let mut tmp_buf = vec![0u8; btor];
      if let Err(e) = sp.read_exact(&mut tmp_buf) {
        mk_err_str(e, "读取串口数据失败");
      };
      debug!("已读取数据：{:?}", const_hex::encode(&tmp_buf));
      buf.extend_from_slice(&tmp_buf);
      debug!("缓冲区：{:?}", const_hex::encode(&buf));
    }

    let mut cursor = 0usize;
    while buf.len() >= FRM_MIN_LEN {
      if FRM_START_FLAG != buf[cursor] {
        if cursor == buf.len() - 1 {
          debug!("未找到帧起始位，清空缓冲区");
          buf.clear();
          break;
        }
        cursor += 1;
        continue;
      }
      let _ = buf.split_to(cursor);
      info!("发现帧起始位：{:?}", const_hex::encode(&buf));
      let frame_length = u16::from_be_bytes([buf[1], buf[2]]) as usize;
      assert!(FRM_MIN_LEN <= frame_length && frame_length <= FRM_MAX_LEN);
      debug!("解析得帧长度：{}", frame_length);
      if frame_length > buf.len() {
        warn!("帧长度 {} 大于剩余有效数据长度 {}，继续等待数据", frame_length, buf.len() + 3);
        break;
      }
      let frame = buf.split_to(frame_length);
      debug!("帧：{:?}", const_hex::encode(&frame));
      let no_head_frame = &frame[FRM_HEAD_LEN..];

      match frame[3].try_into().unwrap() {
        OpFlag::Key => handle_key_response(frame_length, &no_head_frame, weak_app.clone()).await,
        OpFlag::Sm3 => handle_sm3_response(frame_length, &no_head_frame, weak_app.clone()).await,
        OpFlag::Sm4Enc => {
          handle_sm4_enc_response(
            frame_length,
            &no_head_frame,
            weak_app.clone(),
            chat,
            frame[4].try_into().unwrap(),
          )
          .await
        }
        OpFlag::Sm4Dec => {
          handle_sm4_dec_response(
            frame_length,
            &no_head_frame,
            weak_app.clone(),
            chat,
            frame[4].try_into().unwrap(),
          )
          .await
        }
      }
    }
  }
}

pub async fn obsr_sp(mut sp: Box<dyn serialport::SerialPort>, weak_app: Weak<AppWindow>) {
  let mut buf = BytesMut::with_capacity(2 << 20);
  debug!("监听端口中……（观测）");
  loop {
    tokio::time::sleep(Duration::from_millis(0)).await;
    {
      let btor = sp.bytes_to_read().unwrap() as usize;
      if 0 == btor {
        continue;
      }
      debug!("有 {} 字节数据可读", btor);
      let mut tmp_buf = vec![0u8; btor];
      if let Err(e) = sp.read_exact(&mut tmp_buf) {
        mk_err_str(e, "读取串口数据失败");
      };
      debug!("已读取数据：{:?}", const_hex::encode(&tmp_buf));
      buf.extend_from_slice(&tmp_buf);
      debug!("缓冲区：{:?}", const_hex::encode(&buf));
    }

    let mut cursor = 0usize;
    while buf.len() >= FRM_MIN_LEN {
      if FRM_START_FLAG != buf[cursor] {
        if cursor == buf.len() - 1 {
          debug!("未找到帧起始位，清空缓冲区");
          buf.clear();
          break;
        }
        cursor += 1;
        continue;
      }
      let _ = buf.split_to(cursor);
      info!("发现帧起始位：{:?}", const_hex::encode(&buf));
      let frame_length = u16::from_be_bytes([buf[1], buf[2]]) as usize;
      assert!(FRM_MIN_LEN <= frame_length && frame_length <= FRM_MAX_LEN);
      debug!("解析得帧长度：{}", frame_length);
      if frame_length > buf.len() {
        warn!("帧长度 {} 大于剩余有效数据长度 {}，继续等待数据", frame_length, buf.len() + 3);
        break;
      }
      let frame = buf.split_to(frame_length);
      debug!("帧：{:?}", const_hex::encode(&frame));
      let payload = Vec::from(&frame[FRM_HEAD_LEN..frame_length - FRM_TAIL_LEN]);
      let weak_app = weak_app.clone();
      invoke_from_event_loop(move || {
        weak_app
          .unwrap()
          .global::<Options>()
          .invoke_append_dp_text(slint_f!("观测：{}", String::from_utf8_lossy(&payload)));
      })
      .unwrap();
    }
  }
}
