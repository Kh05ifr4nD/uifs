pub use core::fmt::Debug as Dbg;
pub use core::option::Option as Opt;
use core::time::Duration as Durn;
pub use serde::{Deserialize as Deser, Serialize as Ser};
pub use slint::{format as slint_f, SharedString as SlintStr};
pub use snafu::whatever as we;
use std::error::Error;
pub use std::format as f;
pub use std::string::String as Str;

pub type Rst<T, E = snafu::Whatever> = Result<T, E>;

pub const FRM_HEADER_LEN: usize = 1 + 2 + 1 + 1;
pub const FRM_PRESERVE_FLAG: u8 = 0;
pub const FRM_START_FLAG: u8 = 0xC0;
pub const RX_SM3_RTN_LEN: usize = FRM_HEADER_LEN + 32;
pub const SP_BAUD_RATE: u32 = 115_200;
pub const SP_TIMEOUT: Durn = Durn::from_millis(100);
pub const TX_MSG_MAX_LEN: usize = 65408;

#[inline]
pub fn mk_err_str(e: impl Error, desc: &str) -> Str {
  f!("{}: {e:?}; {desc}", core::any::type_name_of_val(&e))
}
