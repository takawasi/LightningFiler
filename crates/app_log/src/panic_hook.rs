//! Panic hook for crash reporting

use backtrace::Backtrace;
use chrono::Local;
use std::panic::PanicHookInfo;

/// Initialize the panic hook for crash reporting
pub fn init_panic_hook() {
    std::panic::set_hook(Box::new(panic_handler));
    tracing::debug!("Panic hook initialized");
}

fn panic_handler(info: &PanicHookInfo) {
    let backtrace = Backtrace::new();
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>");
    let timestamp = Local::now().to_rfc3339();

    // Build the crash report
    let report = format!(
        "=== CRITICAL PANIC ===\n\
         Timestamp: {}\n\
         Thread: {}\n\
         Location: {:?}\n\
         Payload: {:?}\n\n\
         Stack Trace:\n{:?}",
        timestamp,
        thread_name,
        info.location(),
        info.payload().downcast_ref::<&str>().unwrap_or(&"<unknown>"),
        backtrace
    );

    // 1. Log to stderr (always available)
    eprintln!("{}", report);

    // 2. Log via tracing (may fail if async runtime is dead)
    tracing::error!("{}", report);

    // 3. Write crash dump file
    let dump_filename = format!(
        "lightning_filer_crash_{}.txt",
        Local::now().format("%Y%m%d_%H%M%S")
    );
    let dump_path = std::env::temp_dir().join(&dump_filename);

    if let Err(e) = std::fs::write(&dump_path, &report) {
        eprintln!("Failed to write crash dump: {}", e);
    }

    // 4. Show error dialog on Windows
    #[cfg(windows)]
    show_error_dialog(&dump_path, info);
}

#[cfg(windows)]
fn show_error_dialog(dump_path: &std::path::Path, info: &PanicHookInfo) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let msg = format!(
        "An unexpected error occurred.\n\n\
         Log file: {}\n\n\
         Error: {:?}",
        dump_path.display(),
        info.payload().downcast_ref::<&str>().unwrap_or(&"Unknown error")
    );

    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(msg),
            &HSTRING::from("LightningFiler - Fatal Error"),
            MB_ICONERROR | MB_OK,
        );
    }
}
