//! Native-only update entry point; Sparkle verifies and installs signed releases.

use std::cell::RefCell;
use std::ffi::CString;

use cocoa::appkit::{NSApp, NSApplication, NSMenu, NSMenuItem};
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::NSString;
use objc::rc::StrongPtr;
use objc::runtime::Class;
use objc::{class, msg_send, sel, sel_impl};

thread_local! {
    // NSMenuItem's target is weak. Keep its controller alive on the main thread.
    static CONTROLLER: RefCell<Option<StrongPtr>> = const { RefCell::new(None) };
}

pub(super) fn setup_menu() {
    let controller = load_controller()
        .map_err(|error| {
            crate::thor_warn!("thor::updates", "Updates unavailable: {error}");
            error
        })
        .ok();
    unsafe {
        let menu = StrongPtr::new(NSMenu::new(nil));
        let app_item = StrongPtr::new(NSMenuItem::new(nil));
        let app_menu = StrongPtr::new(NSMenu::new(nil));
        let title = StrongPtr::new(NSString::alloc(nil).init_str("Check for Updates…"));
        let key = StrongPtr::new(NSString::alloc(nil).init_str(""));
        let item = StrongPtr::new(NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            *title,
            sel!(checkForUpdates:),
            *key,
        ));
        if let Some(controller) = &controller {
            item.setTarget_(**controller);
        } else {
            let _: () = msg_send![*item, setAction: std::ptr::null::<std::ffi::c_void>()];
        }
        app_menu.addItem_(*item);
        let title = StrongPtr::new(NSString::alloc(nil).init_str("Quit Thor"));
        let key = StrongPtr::new(NSString::alloc(nil).init_str("q"));
        let quit = StrongPtr::new(NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            *title,
            sel!(terminate:),
            *key,
        ));
        app_menu.addItem_(*quit);
        app_item.setSubmenu_(*app_menu);
        menu.addItem_(*app_item);
        NSApp().setMainMenu_(*menu);
    }
    CONTROLLER.with(|cell| *cell.borrow_mut() = controller);
}

fn load_controller() -> anyhow::Result<StrongPtr> {
    if std::env::args_os().any(|arg| arg == "--developer-mode") {
        anyhow::bail!("Developer Mode uses local builds");
    }
    let executable = std::env::current_exe()?;
    let contents = executable
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| anyhow::anyhow!("App bundle missing"))?;
    let framework = contents.join("Frameworks/Sparkle.framework/Sparkle");
    let path = CString::new(framework.as_os_str().as_encoded_bytes())?;
    unsafe {
        // Keep the framework loaded until process exit: ObjC classes and its
        // installer callbacks must outlive all controllers and menu items.
        if libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL).is_null() {
            anyhow::bail!("Bundled Sparkle framework could not be loaded");
        }
        let class = Class::get("SPUStandardUpdaterController")
            .ok_or_else(|| anyhow::anyhow!("Sparkle updater class missing"))?;
        let bundle: id = msg_send![class!(NSBundle), mainBundle];
        let key = StrongPtr::new(NSString::alloc(nil).init_str("SUPublicEDKey"));
        let public_key: id = msg_send![bundle, objectForInfoDictionaryKey: *key];
        if public_key.is_null() {
            anyhow::bail!("Update signing key not configured in this build");
        }
        let allocated: id = msg_send![class, alloc];
        let controller: id = msg_send![allocated, initWithStartingUpdater: NO
            updaterDelegate: nil userDriverDelegate: nil];
        if controller.is_null() {
            anyhow::bail!("Sparkle updater initialization failed");
        }
        let controller = StrongPtr::new(controller);
        let updater: id = msg_send![*controller, updater];
        let mut error: id = nil;
        let started: cocoa::base::BOOL = msg_send![updater, startUpdater: &mut error];
        if started != YES {
            anyhow::bail!("Sparkle rejected this application's update configuration");
        }
        Ok(controller)
    }
}
