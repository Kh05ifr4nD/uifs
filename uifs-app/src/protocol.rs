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

#[repr(u8)]
pub enum Mode {
  CBC,
  ECB,
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
  const BLOCK_BYTES_LEN: usize = 64;
  let pad_len = (m.len() + 1 + 8 + BLOCK_BYTES_LEN - 1) & !(BLOCK_BYTES_LEN - 1);

  let frm_len = FRM_HEAD_LEN + pad_len + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm3 as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_slice(m);
  buf.put_u8(SM3_PAD_FLAG);
  buf.put_bytes(0, pad_len - m.len() - 1 - 8);
  buf.put_u64(m.len() as u64 * 8);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_enc_cbc(pt: &[u8], iv: &[u8; 16]) -> Bytes {
  const BLOCK_BYTES_LEN: usize = 16;
  let pad_len = (pt.len() + BLOCK_BYTES_LEN - 1) & !(BLOCK_BYTES_LEN - 1);

  let frm_len = FRM_HEAD_LEN + pad_len + 1 + 16 + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Enc as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_u8(Mode::CBC as u8);
  buf.put_slice(iv);
  buf.put_slice(pt);
  buf.put_bytes(0, pad_len - pt.len());
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_enc_ecb(ct: &[u8]) -> Bytes {
  const BLOCK_BYTES_LEN: usize = 16;
  let pad_len = (ct.len() + BLOCK_BYTES_LEN - 1) & !(BLOCK_BYTES_LEN - 1);
  let frm_len = FRM_HEAD_LEN + pad_len + 1 + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Enc as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_u8(Mode::ECB as u8);
  buf.put_slice(ct);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_dec_cbc(ct: &[u8], iv: &[u8; 16]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + ct.len() + 1 + 16 + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Dec as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_u8(Mode::CBC as u8);
  buf.put_slice(iv);
  buf.put_slice(ct);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}

pub fn sm4_dec_ecb(ct: &[u8]) -> Bytes {
  let frm_len = FRM_HEAD_LEN + ct.len() + 1 + FRM_TAIL_LEN;
  let mut buf = BytesMut::with_capacity(frm_len);
  buf.put_u8(FRM_START_FLAG);
  buf.put_u16(frm_len as u16);
  buf.put_u8(OpFlag::Sm4Dec as u8);
  buf.put_u8(FRM_PRESERVE_FLAG);
  buf.put_u8(Mode::ECB as u8);
  buf.put_slice(ct);
  buf.put_u16(FRM_PAR_FLAG);
  buf.freeze()
}
