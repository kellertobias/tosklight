#![forbid(unsafe_code)]

pub mod highlight;
pub use light_application::programming_update as update;

mod runtime;
mod tolerant_json;

pub use runtime::run;
