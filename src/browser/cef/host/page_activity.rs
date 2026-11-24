use ::cef::*;

use super::{expiry, with_host};

const MESSAGE: &str = "Thor.PageActivity";
const SCRIPT: &str = include_str!("page_activity.js");

pub(super) fn install(frame: &Frame, context: &V8Context) {
    if context.enter() != 1 {
        report(frame, true);
        return;
    }
    let mut observer = None;
    let installed = context.eval(
        Some(&CefString::from(SCRIPT)),
        Some(&CefString::from("thor://page-activity")),
        1,
        Some(&mut observer),
        None,
    ) == 1
        && observer.is_some_and(|observer| {
            let mut handler = ActivityReporter::new(frame.clone());
            let Some(callback) = v8_value_create_function(
                Some(&CefString::from("reportActivity")),
                Some(&mut handler),
            ) else {
                return false;
            };
            observer
                .execute_function(None, Some(&[Some(callback)]))
                .is_some()
        });
    context.exit();

    // Keep the page open if this document could not be observed.
    report(frame, !installed);
}

pub(super) fn report(frame: &Frame, protected: bool) {
    let Some(mut message) = process_message_create(Some(&CefString::from(MESSAGE))) else {
        return;
    };
    if let Some(args) = message.argument_list() {
        args.set_bool(0, i32::from(protected));
        frame.send_process_message(ProcessId::BROWSER, Some(&mut message));
    }
}

pub(super) fn receive(
    id: u64,
    frame: Option<&Frame>,
    source: ProcessId,
    message: Option<&ProcessMessage>,
) -> bool {
    let Some(message) = message else {
        return false;
    };
    if CefString::from(&message.name()).to_string() != MESSAGE {
        return false;
    }
    if source == ProcessId::RENDERER {
        if let (Some(frame), Some(args)) = (frame, message.argument_list()) {
            if args.size() == 1 && args.get_type(0) == ValueType::BOOL && frame.is_valid() == 1 {
                set_protection(id, frame, args.bool(0) == 1);
            }
        }
    }
    true
}

fn set_protection(id: u64, frame: &Frame, protected: bool) {
    let frame_id = CefString::from(&frame.identifier()).to_string();
    if !frame_id.is_empty() {
        with_host(|host| host.session.set_frame_protection(id, &frame_id, protected));
        expiry::schedule();
    }
}

wrap_frame_handler! {
    pub(super) struct ActivityFrames { id: u64 }
    impl FrameHandler {
        fn on_frame_destroyed(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>) {
            if let Some(frame) = frame {
                set_protection(self.id, frame, false);
            }
        }
    }
}

wrap_v8_handler! {
    struct ActivityReporter { frame: Frame }
    impl V8Handler {
        fn execute(
            &self,
            _name: Option<&CefString>,
            _receiver: Option<&mut V8Value>,
            args: Option<&[Option<V8Value>]>,
            _result: Option<&mut Option<V8Value>>,
            _exception: Option<&mut CefString>,
        ) -> i32 {
            if let Some([Some(value)]) = args {
                if value.is_bool() == 1 {
                    report(&self.frame, value.bool_value() == 1);
                    return 1;
                }
            }
            0
        }
    }
}
