//! Product-owned consent admission, before any synchronous native input setup.
//! A worker may wait for the OS reply; the UI thread never waits or joins it.

#[cfg(any(target_os = "macos", test))]
fn requires_request(status: isize) -> Result<bool, String> {
    match status {
        3 => Ok(false),
        0 => Ok(true),
        2 => Err("Microphone access is denied. Allow Woodshed in System Settings → Privacy & Security → Microphone, then try Record again. Stored takes are unchanged.".into()),
        1 => Err("Microphone access is restricted by macOS. Stored takes are unchanged.".into()),
        _ => Err("macOS returned an unknown microphone authorization state. Input was not opened; stored takes are unchanged.".into()),
    }
}

pub(super) fn authorize(waiting: &mut dyn FnMut()) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::runtime::Bool;
        use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};
        // Apple's constant is retained for the lifetime of the framework. Only
        // its documented audio type is passed to the authorization methods.
        let media = unsafe { AVMediaTypeAudio }.ok_or("macOS microphone authorization is unavailable.")?;
        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) };
        eprintln!("[woodshed-input] microphone authorization status={}", status.0);
        if !requires_request(status.0)? { return Ok(()); }
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        let completion = block2::RcBlock::new(move |granted: Bool| {
            let _ = reply.send(granted.as_bool());
        });
        waiting();
        eprintln!("[woodshed-input] waiting for macOS microphone authorization");
        // AVFoundation copies this block and invokes it on its own queue. Only
        // the Send channel crosses that boundary; no app state or UI is touched.
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &completion) };
        let result = receiver.recv();
        eprintln!("[woodshed-input] microphone authorization reply={result:?}");
        match result {
            Ok(true) => {
                let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) };
                if requires_request(status.0)? {
                    Err("Microphone permission is still undecided. Input was not opened.".into())
                } else { Ok(()) }
            },
            Ok(false) => Err("Microphone access was not granted. Allow Woodshed in System Settings → Privacy & Security → Microphone, then try Record again. Stored takes are unchanged.".into()),
            Err(_) => Err("macOS microphone authorization reply was lost. Input was not opened.".into()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = waiting;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_undecided_consent_requests_access_and_only_authorized_bypasses_it() {
        assert_eq!(requires_request(0), Ok(true));
        assert_eq!(requires_request(3), Ok(false));
        for status in [1, 2, -1, 4, isize::MAX] {
            assert!(requires_request(status).is_err());
        }
    }
}
