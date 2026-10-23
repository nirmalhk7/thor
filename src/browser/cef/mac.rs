use std::cell::Cell;

use ::cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::runtime::{AnyObject, Bool, NSObjectProtocol};
use objc2::{define_class, msg_send, ClassType, DefinedClass, MainThreadMarker};
use objc2_app_kit::{NSApp, NSApplication, NSEvent};
use objc2_cef as objc2;

#[derive(Default)]
pub struct ApplicationState {
    sending_event: Cell<Bool>,
}

define_class!(
    #[unsafe(super(NSApplication))]
    #[ivars = ApplicationState]
    pub struct ThorApplication;

    impl ThorApplication {
        #[unsafe(method(sendEvent:))]
        unsafe fn send_event(&self, event: &NSEvent) {
            let previous = self.ivars().sending_event.replace(Bool::YES);
            let _: () = unsafe { msg_send![super(self), sendEvent: event] };
            self.ivars().sending_event.set(previous);
        }

        #[unsafe(method(terminate:))]
        unsafe fn terminate(&self, _sender: Option<&AnyObject>) {
            super::host::close_window();
        }
    }

    unsafe impl CrAppProtocol for ThorApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn is_handling_send_event(&self) -> Bool {
            self.ivars().sending_event.get()
        }
    }

    unsafe impl CrAppControlProtocol for ThorApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn set_handling_send_event(&self, value: Bool) {
            self.ivars().sending_event.set(value);
        }
    }

    unsafe impl CefAppProtocol for ThorApplication {}
);

pub fn initialize() -> anyhow::Result<()> {
    let main_thread = MainThreadMarker::new()
        .ok_or_else(|| anyhow::anyhow!("CEF must start on the main thread"))?;
    let _: *mut AnyObject = unsafe { msg_send![ThorApplication::class(), sharedApplication] };
    if !NSApp(main_thread).isKindOfClass(ThorApplication::class()) {
        anyhow::bail!("NSApplication was initialized before Thor's CEF application");
    }
    Ok(())
}
