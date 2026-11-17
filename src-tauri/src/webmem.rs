//! Memory for hidden windows. A hidden flyout or settings window still holds its WebView2 renderer;
//! telling WebView2 its "memory usage target" is low lets it trim caches and swap out what it can
//! while nobody is looking, and normal again before it shows.

use tauri::WebviewWindow;

pub fn set_low(w: &WebviewWindow, low: bool) {
    #[cfg(windows)]
    let _ = w.with_webview(move |wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, ICoreWebView2_19,
        };
        use windows_core::Interface;
        unsafe {
            // ICoreWebView2_19 needs WebView2 Runtime 114+ (2023); older runtimes just skip this.
            if let Ok(core) = wv.controller().CoreWebView2() {
                if let Ok(c) = core.cast::<ICoreWebView2_19>() {
                    let level = if low {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                    } else {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                    };
                    let _ = c.SetMemoryUsageTargetLevel(level);
                }
            }
        }
    });
    #[cfg(not(windows))]
    let _ = (w, low);
}
