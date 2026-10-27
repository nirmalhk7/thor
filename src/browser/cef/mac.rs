use std::cell::Cell;

use ::cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::runtime::{AnyObject, Bool, NSObjectProtocol};
use objc2::{define_class, msg_send, ClassType, DefinedClass, MainThreadMarker};
use objc2_app_kit::{NSApp, NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_cef as objc2;

fn shortcut_action(key_code: u16, modifiers: NSEventModifierFlags) -> Option<super::host::Action> {
    if !modifiers.contains(NSEventModifierFlags::Command) {
        return None;
    }
    match key_code {
        40 if modifiers.contains(NSEventModifierFlags::Shift) => Some(super::host::Action::Toolbox),
        40 => Some(super::host::Action::Sites),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_k_opens_sites_and_command_shift_k_opens_toolbox() {
        let command = NSEventModifierFlags::Command;
        assert!(matches!(
            shortcut_action(40, command),
            Some(super::super::host::Action::Sites)
        ));
        assert!(matches!(
            shortcut_action(40, command | NSEventModifierFlags::Shift),
            Some(super::super::host::Action::Toolbox)
        ));
        assert!(shortcut_action(40, NSEventModifierFlags::Shift).is_none());
        assert!(shortcut_action(39, command).is_none());
    }
}

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
            if event.r#type() == NSEventType::KeyDown && !event.isARepeat() {
                if let Some(action) = shortcut_action(event.keyCode(), event.modifierFlags()) {
                    super::host::queue_action(action);
                    return;
                }
            }
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
