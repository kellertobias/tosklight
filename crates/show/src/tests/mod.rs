mod desk_configuration;
mod show_library;
mod show_store;

use std::path::PathBuf;
use uuid::Uuid;

pub(super) fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-{name}-{}.sqlite", Uuid::new_v4()))
}
