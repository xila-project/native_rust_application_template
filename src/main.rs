#![cfg_attr(all(feature = "guest", not(test)), no_std)]
#![cfg_attr(all(feature = "guest", not(test)), no_main)]

#[cfg(all(feature = "guest", not(test)))]
mod guest {
    use core::panic::PanicInfo;
    use xila_sys::{XilaStartup, runtime};

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
        // SAFETY: the entry shim validated the startup record before calling main.
        unsafe { hello::run(startup) }
    }
}

#[cfg(all(not(feature = "guest"), not(test)))]
fn main() {
    eprintln!("Build this app for the Xila loader with mise run build.");
}
