//! Keeping an Architect document and the Control show it came from in step.
//!
//! A document opened from a desk is *bound* to that desk's show: the binding names the desk
//! installation, the show UUID and an association identity that scopes every sync request. The
//! binding is installation state on this computer — never part of the portable show — so a copy
//! of the file carries no binding and can never write into the desk by accident.

pub(crate) mod binding;

pub(crate) use binding::{SyncBinding, SyncBindingStore};
