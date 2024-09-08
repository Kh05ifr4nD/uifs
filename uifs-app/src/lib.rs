use core::error::Error;
pub use core::fmt::Debug as Dbg;
pub use core::option::Option as Opt;
use core::time::Duration as Durn;
pub use slint::{format as slint_f, SharedString as SlintStr};
pub use snafu::whatever as we;
pub use std::format as f;
pub use std::string::String as Str;

pub type Rst<T, E = snafu::Whatever> = Result<T, E>;

pub const FRM_HEAD_LEN: usize = 1 + 2 + 1 + 1;
pub const FRM_MIN_LEN: usize = FRM_HEAD_LEN + FRM_TAIL_LEN;
pub const FRM_MAX_LEN: usize = FRM_HEAD_LEN + 65408 + FRM_TAIL_LEN;
pub const FRM_PRESERVE_FLAG: u8 = 0;
pub const FRM_PAR_FLAG: u16 = 0;
pub const FRM_START_FLAG: u8 = 0xC0;
pub const FRM_TAIL_LEN: usize = 2;
pub const IV_LEN: usize = 16;
pub const KEY_LEN: usize = 16;
pub const RX_SM3_RTN_LEN: usize = FRM_HEAD_LEN + 32;
pub const SM3_HASH_LEN: usize = 32;
pub const SM3_PAD_FLAG: u8 = 0x80;
pub const SP_BAUD_RATE: u32 = 115_200;
pub const SP_TIMEOUT: Durn = Durn::from_secs(1);
pub const TX_MSG_MAX_LEN: usize = 65408;

#[inline]
pub fn mk_err_str(e: impl Error, desc: &str) -> Str {
  f!("{}: {e:?}; {desc}", core::any::type_name_of_val(&e))
}
