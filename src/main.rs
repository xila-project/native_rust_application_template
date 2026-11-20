#![cfg_attr(all(feature = "guest", not(test)), no_std)]
#![cfg_attr(all(feature = "guest", not(test)), no_main)]

#[cfg(all(feature = "guest", not(test)))]
mod guest {
    use core::{panic::PanicInfo, slice};
    use xila_sys::{File, Status, XilaApi, XilaStartup, runtime};

    xila_sys::entry!(main);

    // The allocator is optional: remove it, and with it `alloc`, if the app does not
    // allocate. It only works while the entry runs, because the host owns the heap
    // and takes it back when the entry returns.
    #[global_allocator]
    static HEAP: runtime::ProcessAllocator = runtime::ProcessAllocator;

    // Prints `panicked` to stderr and exits the process with status 134. The host then
    // reclaims the heap and the image. Destructors do not run (`panic = "abort"`).
    #[panic_handler]
    fn panic(_: &PanicInfo<'_>) -> ! {
        runtime::panic_abort()
    }

    /// Called by the entry shim with the host's startup record for this launch.
    unsafe fn main(startup: *const XilaStartup) -> i32 {
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

#[cfg(all(not(feature = "guest"), not(test)))]
fn main() {
    eprintln!("Build this app for the Xila loader with mise run build.");
}
