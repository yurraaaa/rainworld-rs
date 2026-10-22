//! The window's title and application id, the same for every window the app opens.
//!
//! Mirrors: - (port-only: window identity, set by the Unity player in the game)

use winit::window::WindowAttributes;

const TITLE: &str = "Rain World";
const APP_ID: &str = "rainworld-rs";

pub trait WindowIdentity {
    /// Title "Rain World"; app id `rainworld-rs` (Wayland `app_id`, X11 `WM_CLASS`).
    #[must_use]
    fn with_identity(self) -> Self;
}

impl WindowIdentity for WindowAttributes {
    fn with_identity(self) -> Self {
        let attrs = self.with_title(TITLE);
        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
        let attrs = {
            let attrs = winit::platform::wayland::WindowAttributesExtWayland::with_name(
                attrs, APP_ID, APP_ID,
            );
            winit::platform::x11::WindowAttributesExtX11::with_name(attrs, APP_ID, APP_ID)
        };
        #[cfg(not(all(unix, not(target_os = "macos"), not(target_os = "android"))))]
        let _ = APP_ID;
        attrs
    }
}
