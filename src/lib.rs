pub use core::any::type_name_of_val as type_of;
pub use core::fmt::Debug as Dbg;
pub use core::option::Option as Opt;
use core::time::Duration;
pub use serde::{Deserialize as Deser, Serialize as Ser};
pub use slint::SharedString as SlintStr;
pub use snafu::whatever as we;
use std::error::Error;
pub use std::format as f;
pub use std::string::String as Str;

pub type Rst<T, E = snafu::Whatever> = Result<T, E>;

pub const BAUD_RATE: u32 = 115_200;
pub const TIMEOUT: Duration = Duration::from_millis(300);

#[inline]
pub fn mk_err(e: impl Error, desc: &str) -> Str {
    f!("{}: {e:?}; {desc}", type_of(&e))
}
