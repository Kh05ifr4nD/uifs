use bytes::{BufMut, Bytes, BytesMut};
use uifs_app::*;

#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum OpFlag {
  Key = 1,
  Sm3 = 2,
  Sm4Enc = 3,
  Sm4Dec = 4,
}
#[derive(num_enum::TryFromPrimitive, PartialEq)]
#[repr(u8)]
pub enum BlockMode {
  ECB = 1,
  CBC = 2,
}

pub fn key(k: &[u8; 16]) -> Bytes {
  const FRM_LEN: usize = FRM_HEAD_LEN + KEY_LEN + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(FRM_LEN);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(FRM_LEN as u16);
  buf.put_u8(OpFlag::Key as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_slice(k);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm3(m: &[u8]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + m.len() + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm3 as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_slice(m);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_enc_cbc(pt: &[u8], iv: &[u8; 16]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + 16 + pt.len() + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Enc as u8);
  buf.put_u8(BlockMode::CBC as u8);
  buf.put_slice(iv);
  buf.put_slice(pt);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_enc_ecb(pt: &[u8]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + pt.len() + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Enc as u8);
  buf.put_u8(BlockMode::ECB as u8);
  buf.put_slice(pt);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_dec_cbc(ct: &[u8], iv: &[u8; 16]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + 16 + ct.len() + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Dec as u8);
  buf.put_u8(BlockMode::CBC as u8);
  buf.put_slice(iv);
  buf.put_slice(ct);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_dec_ecb(ct: &[u8]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + ct.len() + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Dec as u8);
  buf.put_u8(BlockMode::ECB as u8);
  buf.put_slice(ct);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}
