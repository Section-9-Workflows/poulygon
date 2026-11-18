// SPDX-License-Identifier: MIT
//! Linux input events. Console text remains separate so the user's keymap owns typing.
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Event {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

extern "C" {
    fn facet_input_open(path: *const std::ffi::c_char) -> i32;
    fn facet_input_read(fd: i32, event: *mut Event) -> i32;
    fn facet_input_keys(fd: i32, keys: *mut u8, size: u32) -> i32;
    fn facet_input_close(fd: i32);
}

/// Owns one keyboard or pointer, without grabbing it away from the Linux console.
pub struct Device {
    fd: i32,
    keys: [u8; 96],
    dropped: bool,
}

impl Device {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        let fd = unsafe { facet_input_open(path.as_ptr()) };
        if fd < 0 {
            return Err(std::io::Error::from_raw_os_error(-fd));
        }
        let mut device = Self {
            fd,
            keys: [0; 96],
            dropped: false,
        };
        device.refresh_keys()?;
        Ok(device)
    }

    pub fn held(&self, code: u16) -> bool {
        let code = usize::from(code);
        !self.dropped
            && self
                .keys
                .get(code / 8)
                .is_some_and(|byte| byte & (1 << (code % 8)) != 0)
    }

    /// A dropped kernel queue releases controls until SYN_REPORT restores actual held keys.
    pub fn read(&mut self) -> std::io::Result<Option<Event>> {
        loop {
            let mut event = Event::default();
            let result = unsafe { facet_input_read(self.fd, &mut event) };
            if result < 0 {
                self.keys.fill(0);
                return Err(std::io::Error::from_raw_os_error(-result));
            }
            if result == 0 {
                return Ok(None);
            }
            if event.kind == 0 && event.code == 3 {
                self.dropped = true;
                self.keys.fill(0);
                return Ok(Some(event));
            }
            if self.dropped {
                if event.kind == 0 && event.code == 0 {
                    self.refresh_keys()?;
                    self.dropped = false;
                    return Ok(Some(event));
                }
                continue;
            }
            if event.kind == 1 {
                let code = usize::from(event.code);
                if let Some(byte) = self.keys.get_mut(code / 8) {
                    if event.value == 0 {
                        *byte &= !(1 << (code % 8));
                    } else {
                        *byte |= 1 << (code % 8);
                    }
                }
            }
            return Ok(Some(event));
        }
    }

    fn refresh_keys(&mut self) -> std::io::Result<()> {
        let result =
            unsafe { facet_input_keys(self.fd, self.keys.as_mut_ptr(), self.keys.len() as u32) };
        if result < 0 {
            Err(std::io::Error::from_raw_os_error(-result))
        } else {
            Ok(())
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { facet_input_close(self.fd) };
    }
}
