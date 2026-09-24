//! The operating system's permission for this process to capture microphone input.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophonePermission {
    NotRequired,
    NotDetermined,
    Denied,
    Restricted,
    Granted,
}

// The AVFoundation entry points are Objective-C calls. Keep the only unsafe code in this
// platform adapter; the capture callback and analysis remain safe Rust.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos {
    use super::MicrophonePermission;
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {}

    fn capture_class() -> &'static objc2::runtime::AnyClass {
        class!(AVCaptureDevice)
    }

    pub fn status() -> MicrophonePermission {
        let device = capture_class();
        let audio = NSString::from_str("soun");
        let status: isize = unsafe { msg_send![device, authorizationStatusForMediaType: &*audio] };
        match status {
            0 => MicrophonePermission::NotDetermined,
            1 => MicrophonePermission::Restricted,
            2 => MicrophonePermission::Denied,
            3 => MicrophonePermission::Granted,
            _ => MicrophonePermission::Restricted,
        }
    }

    pub fn request() -> Result<MicrophonePermission, String> {
        if status() != MicrophonePermission::NotDetermined {
            return Ok(status());
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let completion = RcBlock::new(move |granted: Bool| {
            let _ = sender.send(granted.as_bool());
        });
        let audio = NSString::from_str("soun");
        unsafe {
            let _: () = msg_send![capture_class(), requestAccessForMediaType: &*audio, completionHandler: &*completion];
        }
        receiver
            .recv()
            .map_err(|_| "macOS did not finish the microphone permission request".to_owned())?;
        Ok(status())
    }
}

pub fn status() -> MicrophonePermission {
    #[cfg(target_os = "macos")]
    return macos::status();
    #[cfg(not(target_os = "macos"))]
    MicrophonePermission::NotRequired
}

pub fn request() -> Result<MicrophonePermission, String> {
    #[cfg(target_os = "macos")]
    return macos::request();
    #[cfg(not(target_os = "macos"))]
    Ok(MicrophonePermission::NotRequired)
}
