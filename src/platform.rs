//! What the panel asks of Windows itself: the monitor to fill, one panel at a time, the touch
//! screen's fingers, and whether to keep animations still.

use std::{
    ffi::OsStr,
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    thread,
    time::Duration,
};

use futures::channel::mpsc::UnboundedSender;
use gpui::{Point, TouchEvent, TouchId, TouchPhase, point, px};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, HWND,
        INVALID_HANDLE_VALUE, LPARAM, LRESULT, POINT, RECT, WPARAM,
    },
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW, ScreenToClient,
    },
    Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile},
    System::{
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
            PIPE_TYPE_BYTE, PIPE_WAIT,
        },
        Threading::CreateMutexW,
    },
    UI::{
        Controls::{
            FEEDBACK_GESTURE_PRESSANDTAP, FEEDBACK_TOUCH_PRESSANDHOLD, FEEDBACK_TOUCH_RIGHTTAP,
            SetWindowFeedbackSetting,
        },
        HiDpi::GetDpiForWindow,
        Input::Pointer::{GetPointerInfo, GetPointerType, POINTER_INFO},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            PT_TOUCH, SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW, WM_NCDESTROY,
            WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE,
        },
    },
};

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(Some(0)).collect()
}

/// Whether Windows is set to show fewer animations ("アニメーション効果" off).
pub fn reduced_motion() -> bool {
    let mut enabled: i32 = 1;
    // SAFETY: the call writes one BOOL into `enabled`.
    let read = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0)
    };
    read != 0 && enabled == 0
}

/// The monitor whose display name (`\\.\DISPLAY2`) is `name`, ignoring case, as GPUI's
/// display id (its `HMONITOR`).
pub fn monitor_named(name: &str) -> Option<u64> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> i32 {
        // SAFETY: `data` is the address of the vector below, alive for the enumeration.
        unsafe { (*(data as *mut Vec<HMONITOR>)).push(monitor) };
        1
    }
    let mut monitors: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into `monitors`, which outlives the call.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect),
            (&raw mut monitors) as LPARAM,
        );
    }
    monitors.into_iter().find_map(|monitor| {
        // SAFETY: MONITORINFOEXW is plain data; its size field tells the call which form it is.
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = u32::try_from(std::mem::size_of::<MONITORINFOEXW>()).ok()?;
        // SAFETY: `info` is a valid MONITORINFOEXW with its size set.
        if unsafe { GetMonitorInfoW(monitor, (&raw mut info).cast()) } == 0 {
            return None;
        }
        let end = info
            .szDevice
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(info.szDevice.len());
        let device = String::from_utf16_lossy(&info.szDevice[..end]);
        device
            .eq_ignore_ascii_case(name.trim())
            .then_some(monitor as u64)
    })
}

/// One panel at a time: the first start keeps a named mutex and listens on a named pipe; a
/// later start hands its arguments to it and exits.
pub enum Instance {
    First,
    /// Another panel runs; it has been handed the arguments.
    Second,
}

const NAME: &str = "io.github.miyabisun.windows-deck";

fn pipe_name() -> String {
    format!(r"\\.\pipe\{NAME}")
}

pub fn single_instance(args: &[String]) -> Instance {
    let name = wide(&format!(r"Local\{NAME}"));
    // SAFETY: the name is a NUL-terminated UTF-16 string; the handle is kept for the process's
    // life (never closed), which is what holds the mutex.
    let exists = unsafe {
        let mutex = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        !mutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS
    };
    if !exists {
        return Instance::First;
    }
    for _ in 0..20 {
        if let Ok(mut pipe) = std::fs::OpenOptions::new().write(true).open(pipe_name()) {
            let _ = pipe.write_all(args.join("\n").as_bytes());
            return Instance::Second;
        }
        thread::sleep(Duration::from_millis(100));
    }
    Instance::Second
}

/// Hand each later start's arguments to `sink`.
pub fn listen_for_starts(sink: UnboundedSender<Vec<String>>) {
    let spawned = thread::Builder::new().name("starts".into()).spawn(move || {
        let name = wide(&pipe_name());
        loop {
            // SAFETY: the name is NUL-terminated; the handle is closed below.
            let pipe: HANDLE = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1,
                    0,
                    4096,
                    0,
                    std::ptr::null(),
                )
            };
            if pipe == INVALID_HANDLE_VALUE {
                tracing::warn!("cannot listen for later starts");
                return;
            }
            // SAFETY: `pipe` is a valid pipe handle; a client that connected before the call
            // is reported as ERROR_PIPE_CONNECTED.
            let connected = unsafe {
                ConnectNamedPipe(pipe, std::ptr::null_mut()) != 0
                    || GetLastError() == ERROR_PIPE_CONNECTED
            };
            if connected {
                let text = read_all(pipe);
                let args: Vec<String> = text.lines().map(str::to_owned).collect();
                if sink.unbounded_send(args).is_err() {
                    // SAFETY: closes the handle made above.
                    unsafe { CloseHandle(pipe) };
                    return;
                }
            }
            // SAFETY: disconnects and closes the handle made above, once.
            unsafe {
                DisconnectNamedPipe(pipe);
                CloseHandle(pipe);
            }
        }
    });
    if spawned.is_err() {
        tracing::warn!("cannot listen for later starts");
    }
}

fn read_all(pipe: HANDLE) -> String {
    struct Pipe(HANDLE);
    impl Read for Pipe {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut read = 0;
            let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
            // SAFETY: `buf` has room for `len` bytes and `read` receives the count.
            let ok = unsafe {
                ReadFile(
                    self.0,
                    buf.as_mut_ptr(),
                    len,
                    &raw mut read,
                    std::ptr::null_mut(),
                )
            };
            // A closed client ends the message.
            Ok(if ok == 0 { 0 } else { read as usize })
        }
    }
    let mut text = String::new();
    let _ = Pipe(pipe).take(64 << 10).read_to_string(&mut text);
    text
}

/// Take the touch screen's fingers from Windows' pointer messages to `sink` as GPUI touches,
/// instead of the mouse Windows would make of them. GPUI then recognizes taps, long presses
/// and pans (with their momentum) as on a phone. The pen and the mouse stay as they are.
pub fn take_touches(hwnd: isize, sink: UnboundedSender<TouchEvent>) {
    let hwnd = hwnd as HWND;
    let data = Box::into_raw(Box::new(sink)) as usize;
    // SAFETY: the subclass gets `data` back in every call and frees it on WM_NCDESTROY.
    let set = unsafe { SetWindowSubclass(hwnd, Some(touch_proc), 1, data) };
    if set == 0 {
        // SAFETY: not handed to Windows, so it is still ours to free.
        drop(unsafe { Box::from_raw(data as *mut UnboundedSender<TouchEvent>) });
        tracing::warn!("cannot take the touch screen's input");
        return;
    }
    // The panel shows its own menu on a long press; no ring or right click from Windows.
    let off: i32 = 0;
    for feedback in [
        FEEDBACK_TOUCH_PRESSANDHOLD,
        FEEDBACK_TOUCH_RIGHTTAP,
        FEEDBACK_GESTURE_PRESSANDTAP,
    ] {
        // SAFETY: passes one BOOL for a feedback type of this window.
        unsafe {
            SetWindowFeedbackSetting(hwnd, feedback, 0, 4, (&raw const off).cast());
        }
    }
}

/// The phase of a pointer message, or none for one that is not a touch's (a hover update).
pub fn touch_phase(message: u32, in_contact: bool) -> Option<TouchPhase> {
    match message {
        WM_POINTERDOWN => Some(TouchPhase::Started),
        WM_POINTERUPDATE if in_contact => Some(TouchPhase::Moved),
        WM_POINTERUP => Some(TouchPhase::Ended),
        WM_POINTERCAPTURECHANGED => Some(TouchPhase::Cancelled),
        _ => None,
    }
}

/// A position in the window's device pixels as GPUI's logical pixels.
pub fn logical(x: i32, y: i32, dpi: u32) -> Point<gpui::Pixels> {
    #[allow(clippy::cast_precision_loss)] // Pixel coordinates are far below 2^24.
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
    #[allow(clippy::cast_precision_loss)]
    point(px(x as f32 / scale), px(y as f32 / scale))
}

unsafe extern "system" fn touch_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let sink = data as *mut UnboundedSender<TouchEvent>;
    if matches!(
        message,
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP | WM_POINTERCAPTURECHANGED
    ) {
        let pointer = u32::try_from(wparam & 0xffff).unwrap_or_default();
        let mut kind = 0;
        // SAFETY: reads the type of the pointer this message is about.
        let touch = unsafe { GetPointerType(pointer, &raw mut kind) } != 0 && kind == PT_TOUCH;
        if touch {
            const IN_CONTACT: usize = 0x4;
            let in_contact = (wparam >> 16) & IN_CONTACT != 0;
            if let Some(phase) = touch_phase(message, in_contact) {
                // SAFETY: POINTER_INFO is plain data filled by the call.
                let mut info: POINTER_INFO = unsafe { std::mem::zeroed() };
                // SAFETY: `info` receives this pointer's state.
                unsafe { GetPointerInfo(pointer, &raw mut info) };
                let mut at: POINT = info.ptPixelLocation;
                // SAFETY: converts a screen point to this window's client area.
                unsafe { ScreenToClient(hwnd, &raw mut at) };
                // SAFETY: reads this window's DPI.
                let dpi = unsafe { GetDpiForWindow(hwnd) };
                let event = TouchEvent {
                    id: TouchId(u64::from(pointer)),
                    phase,
                    position: logical(at.x, at.y, dpi),
                    predicted_position: None,
                    force: None,
                };
                // SAFETY: `sink` lives until WM_NCDESTROY below.
                let _ = unsafe { &*sink }.unbounded_send(event);
            }
            return 0;
        }
    }
    if message == WM_NCDESTROY {
        // SAFETY: the subclass is removed once, and `data` was made by `take_touches`.
        unsafe {
            RemoveWindowSubclass(hwnd, Some(touch_proc), 1);
            drop(Box::from_raw(sink));
        }
    }
    // SAFETY: everything else goes on to GPUI's own window procedure.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use gpui::{TouchPhase, px};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE,
    };

    use super::{logical, touch_phase};

    #[test]
    fn a_finger_down_moving_and_up_is_a_touch() {
        assert_eq!(touch_phase(WM_POINTERDOWN, true), Some(TouchPhase::Started));
        assert_eq!(touch_phase(WM_POINTERUPDATE, true), Some(TouchPhase::Moved));
        assert_eq!(
            touch_phase(WM_POINTERUPDATE, false),
            None,
            "hovering is no touch"
        );
        assert_eq!(touch_phase(WM_POINTERUP, false), Some(TouchPhase::Ended));
        assert_eq!(
            touch_phase(WM_POINTERCAPTURECHANGED, false),
            Some(TouchPhase::Cancelled)
        );
    }

    #[test]
    fn device_pixels_become_logical_pixels() {
        let at = logical(300, 150, 144);
        assert_eq!((at.x, at.y), (px(200.), px(100.)));
        assert_eq!(
            logical(10, 20, 0).x,
            px(10.),
            "an unknown DPI is taken as 96"
        );
    }
}
