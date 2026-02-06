/// Returns seconds since last user input (mouse/keyboard), or `None` if
/// it cannot be determined.

#[cfg(target_os = "linux")]
pub fn system_idle_secs() -> Option<u64> {
    use x11rb::connection::Connection;
    use x11rb::protocol::screensaver;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;
    let reply = screensaver::query_info(&conn, root).ok()?.reply().ok()?;
    Some(u64::from(reply.ms_since_user_input) / 1000)
}

#[cfg(target_os = "macos")]
pub fn system_idle_secs() -> Option<u64> {
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(
            source_state_id: i32,
            event_type: u32,
        ) -> f64;
    }
    // kCGEventSourceStateCombinedSessionState = 0
    // kCGAnyInputEventType = ~0u
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
    if secs >= 0.0 {
        Some(secs as u64)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
pub fn system_idle_secs() -> Option<u64> {
    #[repr(C)]
    struct LASTINPUTINFO {
        cb_size: u32,
        dw_time: u32,
    }
    unsafe extern "system" {
        fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
        fn GetTickCount() -> u32;
    }
    let mut info = LASTINPUTINFO {
        cb_size: size_of::<LASTINPUTINFO>() as u32,
        dw_time: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut info) != 0 {
            let now = GetTickCount();
            Some(u64::from(now.wrapping_sub(info.dw_time)) / 1000)
        } else {
            None
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn system_idle_secs() -> Option<u64> {
    None
}
