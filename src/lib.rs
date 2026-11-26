//! Shared `no_std` application logic for Xila native applications.
//!
//! The same [`run`] body backs both native modes:
//!
//! - `guest`: `src/main.rs` builds a loadable static-PIE ELF whose `_xila_start`
//!   entry shim calls it.
//! - `linked`: this crate exports `xila_entry`, which the firmware mounts directly
//!   with `xila::executable::linked_executable!`.
#![cfg_attr(not(test), no_std)]

#[cfg(any(feature = "guest", feature = "linked"))]
mod app {
    use core::slice;
    use xila_sys::{File, Status, XilaApi, XilaStartup, runtime};

    /// The application body, shared by the loadable (`guest`) and statically linked
    /// (`linked`) entries.
    ///
    /// # Safety
    /// The host must supply the startup data described by Xila's native ABI, as the
    /// entry shim emitted by `xila_sys::entry!` does: a validated startup record whose
    /// borrowed streams live until the entry returns.
    pub unsafe fn run(startup: *const XilaStartup) -> i32 {
        let api = runtime::api().expect("entry initialized the Xila API");
        // SAFETY: The startup record and its borrowed streams live until entry returns.
        // A stream pointer may be null. The host then rejects it with an error status
        // and `write_all` returns false, so the app exits with status 2.
        let startup = unsafe { &*startup };
        let stdout = |bytes| {
            // SAFETY: stdout is borrowed from this launch and is never closed here. It
            // may be null, which the host rejects, making `write_all` return false.
            unsafe { write_all(api, startup.standard_output, bytes) }
        };
        if !stdout(b"Hello from Rust on Xila!\n") {
            return 2;
        }

        let arguments = if startup.argument_count == 0 {
            &[]
        } else {
            // SAFETY: A nonempty array contains argument_count host-owned XilaStrings.
            unsafe { slice::from_raw_parts(startup.arguments, startup.argument_count) }
        };
        let mut fail = false;
        for (index, argument) in arguments.iter().enumerate() {
            let bytes = if argument.length == 0 {
                &[]
            } else {
                // SAFETY: Each nonempty string points to length initialized bytes.
                unsafe { slice::from_raw_parts(argument.data.cast::<u8>(), argument.length) }
            };
            if index == 0 {
                fail = bytes == b"--fail";
            }
            if !(stdout(b"arg: ") && stdout(bytes) && stdout(b"\n")) {
                return 2;
            }
        }

        if fail {
            // SAFETY: stderr is borrowed from this launch and is never closed here. It
            // may be null, which the host rejects, making `write_all` return false.
            if unsafe {
                write_all(
                    api,
                    startup.standard_error,
                    b"Requested failure (--fail).\n",
                )
            } {
                return 1;
            }
            return 2;
        }
        0
    }

    /// Write all bytes, retrying short writes.
    ///
    /// The stream must be a writable capability borrowed from this launch, or null: the
    /// host rejects a null stream with an error status, and this returns false.
    unsafe fn write_all(api: &XilaApi, stream: *mut File, mut bytes: &[u8]) -> bool {
        while !bytes.is_empty() {
            let mut written = 0;
            // SAFETY: The stream is live or null (the host rejects null), bytes is readable,
            // and written is an out parameter.
            let status = unsafe {
                (api.file_write)(stream, bytes.as_ptr().cast(), bytes.len(), &mut written)
            };
            if status != Status::Ok || written == 0 || written > bytes.len() {
                return false;
            }
            bytes = &bytes[written..];
        }
        true
    }
}

#[cfg(any(feature = "guest", feature = "linked"))]
pub use app::run;

// The entry of the statically linked build (`linked` feature): a C-ABI function that
// the firmware passes to its linked-program adapter. Unlike the guest entry it emits
// no ELF note and no `_xila_start`, and it installs no allocator and no panic handler:
// the firmware's own apply, and `xila-sys` does not compile them in this mode. Mount
// it with `xila::executable::linked_executable!(unsafe Hello => hello::xila_entry)`.
#[cfg(all(feature = "linked", not(test)))]
xila_sys::entry!(run);
