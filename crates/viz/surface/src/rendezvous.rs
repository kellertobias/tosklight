//! Introducing one process's surface to another, on macOS.
//!
//! The surface itself is the easy half and is proved elsewhere in this crate: a texture written on
//! one device is read on another over the same `IOSurface`. What is hard is naming it across a
//! process boundary.
//!
//! The obvious route does not work. `IOSurfaceCreateMachPort` gives a port and
//! `IOSurfaceLookupFromMachPort` resolves one, but a mach port name means nothing outside the task
//! holding it — the number cannot be written down a pipe and read back. `IOSurfaceLookup` by the
//! machine-wide ID would need no channel at all, but it resolves only surfaces created as global,
//! and modern macOS ignores that request; measured on Darwin 25.5, a desk looking up a surface the
//! renderer had just created found nothing.
//!
//! So the right has to be *sent*, in a mach message, over a channel the two processes can find
//! each other on. That is what this is: the desk registers a port under a name nobody else will
//! guess, passes the name to the renderer on its command line, and the renderer sends the
//! surface's port right back through it.
//!
//! # What can go wrong, and what happens then
//!
//! `bootstrap_register` is deprecated, and a sandboxed or otherwise restricted process may not be
//! allowed to register at all. That is why none of this is on a path that must succeed: the desk
//! only announces the shared transport when a rendezvous actually opened, so a machine where this
//! is refused negotiates the copy transport instead and draws the same picture more slowly.

use mach2::bootstrap::{bootstrap_look_up, bootstrap_port, bootstrap_register};
use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_port::{mach_port_allocate, mach_port_deallocate, mach_port_insert_right};
use mach2::message::{
    MACH_MSG_PORT_DESCRIPTOR, MACH_MSG_SUCCESS, MACH_MSG_TIMEOUT_NONE, MACH_MSG_TYPE_COPY_SEND,
    MACH_MSG_TYPE_MAKE_SEND, MACH_MSG_TYPE_MOVE_SEND, MACH_RCV_MSG, MACH_RCV_TIMEOUT,
    MACH_SEND_MSG, mach_msg, mach_msg_body_t, mach_msg_header_t, mach_msg_port_descriptor_t,
    mach_msg_trailer_t,
};
use mach2::port::{MACH_PORT_NULL, MACH_PORT_RIGHT_RECEIVE, mach_port_t};
use mach2::traps::mach_task_self;

/// `MACH_MSGH_BITS_COMPLEX`, which says the message carries descriptors rather than only bytes.
const COMPLEX: u32 = 0x8000_0000;

/// A message carrying exactly one port right and nothing else.
#[repr(C)]
struct PortMessage {
    header: mach_msg_header_t,
    body: mach_msg_body_t,
    port: mach_msg_port_descriptor_t,
}

/// The same, with room for the trailer the kernel appends on receive.
#[repr(C)]
struct ReceivedPortMessage {
    header: mach_msg_header_t,
    body: mach_msg_body_t,
    port: mach_msg_port_descriptor_t,
    trailer: mach_msg_trailer_t,
}

/// The desk's end: a registered port the renderer can find and send a surface to.
pub struct Rendezvous {
    port: mach_port_t,
    name: String,
}

impl Rendezvous {
    /// Register a port under a name only this launch knows.
    ///
    /// The name carries the process id and a counter rather than anything guessable, because a
    /// bootstrap name is visible to the whole session and two desks must never collide.
    pub fn open(unique: &str) -> Result<Self, String> {
        let name = format!("de.tokenet.tosklight.stage-pane.{unique}");
        let task = unsafe { mach_task_self() };
        let mut port: mach_port_t = MACH_PORT_NULL;
        // SAFETY: `task` is this task's own port and `port` is a live out-parameter.
        let allocated = unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &raw mut port) };
        if allocated != KERN_SUCCESS {
            return Err(format!("could not allocate a port: {allocated}"));
        }
        // A send right for the name to hand out, made from the receive right just allocated.
        // SAFETY: `port` holds a receive right, which is what `MAKE_SEND` requires.
        let inserted = unsafe { mach_port_insert_right(task, port, port, MACH_MSG_TYPE_MAKE_SEND) };
        if inserted != KERN_SUCCESS {
            // SAFETY: deallocating a right this function allocated and nothing else holds.
            unsafe { mach_port_deallocate(task, port) };
            return Err(format!("could not make a send right: {inserted}"));
        }

        let service = std::ffi::CString::new(name.clone()).map_err(|error| error.to_string())?;
        // SAFETY: the name is a NUL-terminated C string that outlives the call, and `port` carries
        // the send right the name is being registered for.
        let registered =
            unsafe { bootstrap_register(bootstrap_port, service.as_ptr().cast_mut(), port) };
        if registered != KERN_SUCCESS {
            // SAFETY: as above — nothing else refers to this port.
            unsafe { mach_port_deallocate(task, port) };
            // The common reason, and the one worth naming: a restricted process may not register.
            return Err(format!(
                "this process may not register a name to hand surfaces over ({registered})"
            ));
        }
        Ok(Self { port, name })
    }

    /// The name to give the renderer, so it can find this.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Wait for the renderer to send a surface's port right.
    ///
    /// Returns `Ok(None)` when nothing arrived inside the timeout, which is not a failure: the
    /// renderer sends one per surface, and a surface is created only when the pane's size changes.
    pub fn receive(&self, timeout: std::time::Duration) -> Result<Option<mach_port_t>, String> {
        let mut message = std::mem::MaybeUninit::<ReceivedPortMessage>::zeroed();
        let size = u32::try_from(size_of::<ReceivedPortMessage>()).map_err(|_| "message size")?;
        let milliseconds = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: the buffer is the size passed as `receive_limit`, and the port holds the receive
        // right this rendezvous allocated.
        let result = unsafe {
            mach_msg(
                message.as_mut_ptr().cast::<mach_msg_header_t>(),
                MACH_RCV_MSG | MACH_RCV_TIMEOUT,
                0,
                size,
                self.port,
                milliseconds,
                MACH_PORT_NULL,
            )
        };
        if result != MACH_MSG_SUCCESS {
            // Timed out with nothing waiting, which is the ordinary case between resizes.
            return Ok(None);
        }
        // SAFETY: `mach_msg` succeeded, so the buffer holds a complete message.
        let message = unsafe { message.assume_init() };
        if message.body.msgh_descriptor_count != 1 {
            return Err("the renderer sent a message with no surface in it".to_owned());
        }
        Ok(Some(message.port.name))
    }
}

impl Drop for Rendezvous {
    fn drop(&mut self) {
        // SAFETY: the rendezvous owns this port and is being dropped, so nothing else refers to it.
        unsafe { mach_port_deallocate(mach_task_self(), self.port) };
    }
}

/// The renderer's end: find the desk's port and send it a surface's right.
pub fn send_port(service_name: &str, surface_port: mach_port_t) -> Result<(), String> {
    let service = std::ffi::CString::new(service_name).map_err(|error| error.to_string())?;
    let mut desk: mach_port_t = MACH_PORT_NULL;
    // SAFETY: the name is NUL-terminated and outlives the call; `desk` is a live out-parameter.
    let found =
        unsafe { bootstrap_look_up(bootstrap_port, service.as_ptr().cast_mut(), &raw mut desk) };
    if found != KERN_SUCCESS {
        return Err(format!(
            "the desk is not listening on {service_name} ({found})"
        ));
    }

    let mut message = PortMessage {
        header: mach_msg_header_t {
            // A complex message — it carries a descriptor — sent to a right this process copies
            // rather than gives away, so the name stays usable for the next surface.
            msgh_bits: MACH_MSG_TYPE_COPY_SEND | COMPLEX,
            msgh_size: u32::try_from(size_of::<PortMessage>()).map_err(|_| "message size")?,
            msgh_remote_port: desk,
            msgh_local_port: MACH_PORT_NULL,
            msgh_voucher_port: MACH_PORT_NULL,
            msgh_id: 0,
        },
        body: mach_msg_body_t {
            msgh_descriptor_count: 1,
        },
        port: mach_msg_port_descriptor_t::new(surface_port, MACH_MSG_TYPE_COPY_SEND),
    };
    // SAFETY: the header describes exactly the struct being sent, and the descriptor names a send
    // right this process holds.
    let sent = unsafe {
        mach_msg(
            (&raw mut message).cast::<mach_msg_header_t>(),
            MACH_SEND_MSG,
            message.header.msgh_size,
            0,
            MACH_PORT_NULL,
            MACH_MSG_TIMEOUT_NONE,
            MACH_PORT_NULL,
        )
    };
    // SAFETY: the looked-up right is this process's to release once the message has gone.
    unsafe { mach_port_deallocate(mach_task_self(), desk) };
    if sent != MACH_MSG_SUCCESS {
        return Err(format!(
            "the surface could not be sent to the desk ({sent})"
        ));
    }
    Ok(())
}

/// Silence the unused-import warning for a constant kept for the reader's benefit.
const _: u32 = MACH_MSG_TYPE_MOVE_SEND;
const _: u32 = MACH_MSG_PORT_DESCRIPTOR;

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether this process may register a name at all. Everything else depends on it, and a
    /// machine that refuses is a machine that uses the copy transport rather than a broken one.
    #[test]
    fn a_rendezvous_can_be_opened_and_found() {
        let unique = format!("test-{}", std::process::id());
        let Ok(rendezvous) = Rendezvous::open(&unique) else {
            eprintln!("this process may not register a bootstrap name; skipping");
            return;
        };

        // Send a right to a port of our own through it, which is the same operation an
        // `IOSurface` port takes and needs no GPU to check.
        let task = unsafe { mach_task_self() };
        let mut spare: mach_port_t = MACH_PORT_NULL;
        assert_eq!(
            unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &raw mut spare) },
            KERN_SUCCESS
        );
        assert_eq!(
            unsafe { mach_port_insert_right(task, spare, spare, MACH_MSG_TYPE_MAKE_SEND) },
            KERN_SUCCESS
        );

        send_port(rendezvous.name(), spare).expect("the right crosses the rendezvous");
        let received = rendezvous
            .receive(std::time::Duration::from_secs(2))
            .expect("receiving");
        assert!(
            received.is_some_and(|port| port != MACH_PORT_NULL),
            "a port right arrived"
        );
    }
}
