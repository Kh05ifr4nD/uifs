pub use core::any::type_name_of_val as type_of;
pub use core::fmt::Debug as Dbg;
pub use core::option::Option as Opt;
pub use serde::{Deserialize as Deser, Serialize as Ser};
pub use snafu::whatever as we;
pub use std::format as f;
pub use std::string::String as Str;

pub type Rst<T, E = snafu::Whatever> = Result<T, E>;
