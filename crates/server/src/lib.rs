#![forbid(unsafe_code)]

pub mod command_line;
pub mod highlight;
pub mod update;

mod runtime;

pub use runtime::run;
