/// Set the macOS app activation policy. When the user has hidden the dock icon
/// (`show_in_dock == false`) the app runs as an accessory (no dock icon).
#[cfg(target_os = "macos")]
pub fn set_activation_policy(show_in_dock: bool) {
    use objc2_app_kit::{NSApplicationActivationPolicyAccessory, NSApplicationActivationPolicyRegular, NSApplication};
    use objc2::rc::autoreleasepool;

    autoreleasepool(|pool| {
        let app = NSApplication::sharedApplication(pool);
        if show_in_dock {
            unsafe { app.setActivationPolicy(NSApplicationActivationPolicyRegular) };
        } else {
            unsafe { app.setActivationPolicy(NSApplicationActivationPolicyAccessory) };
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn set_activation_policy(_show_in_dock: bool) {}