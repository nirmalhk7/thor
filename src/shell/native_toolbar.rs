//! Native AppKit toolbar — WKWebView siblings eat toolbar hits on macOS.
//! Minimal strip: traffic lights · new-tab · glass omnibox.
//! Tool modes (e.g. Locust via ⌘⇧L) shrink the omnibox and host inputs in the bar.

#![cfg(target_os = "macos")]

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use block::ConcreteBlock;
use cocoa::appkit::NSApp;
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::EventLoopProxy;
use tao::platform::macos::WindowExtMacOS;
use tao::window::Window;

use super::{ToolbarCmd, UserEvent, CHROME_HEIGHT};

const TOOLBAR_CONTENT_LEADING: f64 = 76.0;
const TRAFFIC_LIGHT_SIZE: f64 = 12.0;
const TRAFFIC_LIGHT_X: f64 = 14.0;
const ROW_H: f64 = 28.0;
const FIELD_INSET_Y: f64 = 5.0; // vertical pad so caret/baseline sits mid-glass
const TOOLBAR_TRANSITION_SECS: f64 = 0.14;

fn prefers_reduced_motion() -> bool {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return false;
        }
        let selector = sel!(accessibilityDisplayShouldReduceMotion);
        let responds: bool = msg_send![workspace, respondsToSelector: selector];
        responds && msg_send![workspace, accessibilityDisplayShouldReduceMotion]
    }
}

fn set_view_frame(view: id, frame: NSRect) {
    unsafe {
        let _: () = msg_send![view, setFrame: frame];
    }
}

fn cue_mode_change(view: id, animated: bool) {
    if !animated || prefers_reduced_motion() {
        return;
    }
    unsafe {
        // A short opacity settle makes deliberate mode changes legible without
        // animating layout or competing with the resize path.
        let _: () = msg_send![view, setAlphaValue: 0.82f64];
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let context: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![context, setDuration: TOOLBAR_TRANSITION_SECS];
        let animator: id = msg_send![view, animator];
        let _: () = msg_send![animator, setAlphaValue: 1.0f64];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    }
}

fn set_omnibox_focus_appearance(field: id, focused: bool) {
    unsafe {
        let glass: id = msg_send![field, superview];
        if glass == nil {
            return;
        }
        let layer: id = msg_send![glass, layer];
        if layer == nil {
            return;
        }
        let border = if focused {
            ns_color_rgba(0.23, 0.51, 0.96, 0.82)
        } else {
            ns_color_rgba(1.0, 1.0, 1.0, 0.10)
        };
        let border: id = msg_send![border, CGColor];
        let _: () = msg_send![layer, setBorderColor: border];
        let _: () = msg_send![layer, setBorderWidth: if focused { 1.25f64 } else { 1.0f64 }];
        let _: () = msg_send![layer, setShadowOpacity: if focused { 0.32f32 } else { 0.0f32 }];
        let _: () = msg_send![layer, setShadowRadius: if focused { 6.0f64 } else { 0.0f64 }];
        let _: () = msg_send![layer, setShadowOffset: NSSize::new(0.0, 1.0)];
    }
}

fn traffic_light_inset_y() -> f64 {
    // Tao positions the lights from the titlebar's upper-left corner. Match
    // their 12pt frame to the vertical center of the 28pt toolbar row.
    row_y() + (ROW_H - TRAFFIC_LIGHT_SIZE) / 2.0
}

fn row_y() -> f64 {
    ((CHROME_HEIGHT - ROW_H) / 2.0).max(0.0)
}

static PROXY: OnceLock<Mutex<Option<EventLoopProxy<UserEvent>>>> = OnceLock::new();

fn proxy_slot() -> &'static Mutex<Option<EventLoopProxy<UserEvent>>> {
    PROXY.get_or_init(|| Mutex::new(None))
}

fn send(cmd: ToolbarCmd) {
    if let Ok(guard) = proxy_slot().lock() {
        if let Some(proxy) = guard.as_ref() {
            let _ = proxy.send_event(UserEvent::Toolbar(cmd));
        }
    }
}

fn nsstring(s: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(s) }
}

fn ns_color_rgba(r: f64, g: f64, b: f64, a: f64) -> id {
    unsafe {
        msg_send![
            class!(NSColor),
            colorWithCalibratedRed: r green: g blue: b alpha: a
        ]
    }
}

fn read_nsstring(field: id) -> String {
    unsafe {
        let val: id = msg_send![field, stringValue];
        if val == nil {
            return String::new();
        }
        let utf8: *const std::os::raw::c_char = msg_send![val, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned()
    }
}

fn make_flush_button(title: &str, tag: i64, frame: NSRect) -> id {
    unsafe {
        let btn: id = msg_send![class!(NSButton), alloc];
        let btn: id = msg_send![btn, initWithFrame: frame];
        let _: () = msg_send![btn, setTitle: nsstring(title)];
        let _: () = msg_send![btn, setBezelStyle: 15u64];
        let _: () = msg_send![btn, setBordered: NO];
        let _: () = msg_send![btn, setTag: tag];
        let _: () = msg_send![btn, setTarget: toolbar_target()];
        let _: () = msg_send![btn, setAction: sel!(thorToolbarClick:)];
        let _: () = msg_send![btn, setFocusRingType: 1u64];
        let _: () = msg_send![btn, setToolTip: nsstring(match tag {
            4 => "New Tab (⌘T)",
            42 => "Close tool (Esc)",
            _ => title,
        })];
        let _: () = msg_send![btn, setAccessibilityLabel: nsstring(match tag {
            4 => "New Tab",
            42 => "Close tool",
            _ => title,
        })];
        let _: () = msg_send![btn, setWantsLayer: YES];
        let font: id = msg_send![class!(NSFont), systemFontOfSize: 15.0f64];
        let _: () = msg_send![btn, setFont: font];
        let layer: id = msg_send![btn, layer];
        if layer != nil {
            let _: () = msg_send![layer, setCornerRadius: 6.0f64];
        }
        let _: () = msg_send![btn, setContentTintColor: ns_color_rgba(0.58, 0.64, 0.72, 1.0)];
        btn
    }
}

fn style_inline_field(field: id, placeholder: &str) {
    unsafe {
        // Match the glass omnibox: translucent fill, hairline border, 6px radius.
        let _: () = msg_send![field, setBezeled: NO];
        let _: () = msg_send![field, setBordered: NO];
        let _: () = msg_send![field, setDrawsBackground: NO];
        let _: () = msg_send![field, setEditable: YES];
        let _: () = msg_send![field, setSelectable: YES];
        let _: () = msg_send![field, setRefusesFirstResponder: NO];
        let _: () = msg_send![field, setFocusRingType: 1u64];
        let font: id = msg_send![class!(NSFont), systemFontOfSize: 13.0f64];
        let _: () = msg_send![field, setFont: font];
        let _: () = msg_send![field, setTextColor: ns_color_rgba(0.95, 0.96, 0.98, 1.0)];
        let _: () = msg_send![field, setToolTip: nsstring(placeholder)];
        let _: () = msg_send![field, setAccessibilityLabel: nsstring(placeholder)];
        let _: () = msg_send![field, setAlignment: 0u64]; // left — same as URL field
        let _: () = msg_send![field, setWantsLayer: YES];
        let layer: id = msg_send![field, layer];
        if layer != nil {
            let _: () = msg_send![layer, setCornerRadius: 6.0f64];
            let _: () = msg_send![layer, setMasksToBounds: YES];
            let bg = ns_color_rgba(1.0, 1.0, 1.0, 0.08);
            let cg: id = msg_send![bg, CGColor];
            let _: () = msg_send![layer, setBackgroundColor: cg];
            let _: () = msg_send![layer, setBorderWidth: 1.0f64];
            let border = ns_color_rgba(1.0, 1.0, 1.0, 0.10);
            let bcg: id = msg_send![border, CGColor];
            let _: () = msg_send![layer, setBorderColor: bcg];
        }
        let cell: id = msg_send![field, cell];
        if cell != nil {
            let _: () = msg_send![cell, setWraps: NO];
            let _: () = msg_send![cell, setScrollable: YES];
            let _: () = msg_send![cell, setUsesSingleLineMode: YES];
            let _: () = msg_send![cell, setLineBreakMode: 5u64];
            let _: () = msg_send![cell, setPlaceholderString: nsstring(placeholder)];
        }
    }
}

/// Glass omnibox shell + stock NSTextField (no custom cell — that broke typing).
/// Returns `(glass, field, load_bar)`.
fn make_glass_omnibox(frame: NSRect) -> (id, id, id) {
    unsafe {
        let glass: id = msg_send![class!(NSView), alloc];
        let glass: id = msg_send![glass, initWithFrame: frame];
        let _: () = msg_send![glass, setWantsLayer: YES];
        let layer: id = msg_send![glass, layer];
        if layer != nil {
            let _: () = msg_send![layer, setCornerRadius: 6.0f64];
            let _: () = msg_send![layer, setMasksToBounds: YES];
            let bg = ns_color_rgba(1.0, 1.0, 1.0, 0.08);
            let cg: id = msg_send![bg, CGColor];
            let _: () = msg_send![layer, setBackgroundColor: cg];
            let _: () = msg_send![layer, setBorderWidth: 1.0f64];
            let border = ns_color_rgba(1.0, 1.0, 1.0, 0.10);
            let bcg: id = msg_send![border, CGColor];
            let _: () = msg_send![layer, setBorderColor: bcg];
        }
        let _: () = msg_send![glass, setAutoresizingMask: (1u64 << 1)];

        let inner_h = (frame.size.height - FIELD_INSET_Y * 2.0).max(16.0);
        let field_rect = NSRect::new(
            NSPoint::new(10.0, FIELD_INSET_Y),
            NSSize::new((frame.size.width - 20.0).max(40.0), inner_h),
        );
        let field: id = msg_send![omnibox_field_class(), alloc];
        let field: id = msg_send![field, initWithFrame: field_rect];
        let _: () = msg_send![field, setStringValue: nsstring("https://www.google.com")];
        let _: () = msg_send![field, setBezeled: NO];
        let _: () = msg_send![field, setBordered: NO];
        let _: () = msg_send![field, setDrawsBackground: NO];
        let _: () = msg_send![field, setEditable: YES];
        let _: () = msg_send![field, setSelectable: YES];
        let _: () = msg_send![field, setEnabled: YES];
        let _: () = msg_send![field, setRefusesFirstResponder: NO];
        let _: () = msg_send![field, setFocusRingType: 1u64];
        let font: id = msg_send![class!(NSFont), systemFontOfSize: 13.0f64];
        let _: () = msg_send![field, setFont: font];
        let _: () = msg_send![field, setTextColor: ns_color_rgba(0.95, 0.96, 0.98, 1.0)];
        let _: () = msg_send![field, setToolTip: nsstring("Address or search query")];
        let _: () = msg_send![field, setAccessibilityLabel: nsstring("Address or search query")];
        let _: () = msg_send![field, setAlignment: 0u64]; // left
        let cell: id = msg_send![field, cell];
        if cell != nil {
            let _: () = msg_send![cell, setWraps: NO];
            let _: () = msg_send![cell, setScrollable: YES];
            let _: () = msg_send![cell, setUsesSingleLineMode: YES];
            let _: () = msg_send![cell, setLineBreakMode: 5u64];
            let _: () = msg_send![cell, setEditable: YES];
            let _: () = msg_send![cell, setSelectable: YES];
        }
        let _: () = msg_send![field, setTarget: toolbar_target()];
        let _: () = msg_send![field, setAction: sel!(thorOmniboxAction:)];
        let _: () = msg_send![field, setAutoresizingMask: (1u64 << 1)];
        let _: () = msg_send![glass, addSubview: field];

        // Red load bar along the bottom edge of the omnibar.
        let load_h = 2.5f64;
        let load_bar: id = msg_send![class!(NSView), alloc];
        let load_bar: id = msg_send![
            load_bar,
            initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, load_h))
        ];
        let _: () = msg_send![load_bar, setWantsLayer: YES];
        let load_layer: id = msg_send![load_bar, layer];
        if load_layer != nil {
            // #ef4444
            let red = ns_color_rgba(0.937, 0.267, 0.267, 1.0);
            let rcg: id = msg_send![red, CGColor];
            let _: () = msg_send![load_layer, setBackgroundColor: rcg];
        }
        let _: () = msg_send![load_bar, setHidden: YES];
        let _: () = msg_send![glass, addSubview: load_bar];

        (glass, field, load_bar)
    }
}

fn sync_omnibox_inner(glass: id, field: id) {
    unsafe {
        let gf: NSRect = msg_send![glass, frame];
        let inner_h = (gf.size.height - FIELD_INSET_Y * 2.0).max(16.0);
        let fr = NSRect::new(
            NSPoint::new(10.0, FIELD_INSET_Y),
            NSSize::new((gf.size.width - 20.0).max(40.0), inner_h),
        );
        let _: () = msg_send![field, setFrame: fr];
    }
}

fn sync_omnibox_load_bar(glass: id, load_bar: id, progress: f64) {
    unsafe {
        let gf: NSRect = msg_send![glass, frame];
        let load_h = 2.5f64;
        let pct = progress.clamp(0.0, 1.0);
        let w = if pct <= 0.0 {
            0.0
        } else {
            (gf.size.width * pct).max(4.0)
        };
        let fr = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, load_h));
        let _: () = msg_send![load_bar, setFrame: fr];
        let _: () = msg_send![load_bar, setHidden: if pct <= 0.0 { YES } else { NO }];
    }
}

fn toolbar_target() -> id {
    static TARGET: OnceLock<usize> = OnceLock::new();
    let ptr = *TARGET.get_or_init(|| unsafe {
        let cls = Class::get("ThorToolbarTarget").unwrap_or_else(|| {
            let mut decl = ClassDecl::new("ThorToolbarTarget", class!(NSObject)).unwrap();
            decl.add_method(
                sel!(thorToolbarClick:),
                toolbar_click as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(thorOmniboxAction:),
                omnibox_action as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(thorOpenCmdK:),
                menu_cmd_k as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(thorOpenCmdShiftK:),
                menu_cmd_shift_k as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(thorLocustStart:),
                locust_start_action as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(thorLocustToggleMenu:),
                menu_locust_toggle as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(thorToggleProfileMenu:),
                menu_toggle_profile as extern "C" fn(&Object, Sel, id),
            );
            decl.register()
        });
        let obj: id = msg_send![cls, alloc];
        let obj: id = msg_send![obj, init];
        // Retain forever — NSControl targets are assign/weak.
        let _: id = msg_send![obj, retain];
        obj as usize
    });
    ptr as id
}

fn omnibox_field_class() -> &'static Class {
    static CLASS: OnceLock<&'static Class> = OnceLock::new();
    CLASS.get_or_init(|| {
        Class::get("ThorOmniboxTextField").unwrap_or_else(|| {
            let mut decl = ClassDecl::new("ThorOmniboxTextField", class!(NSTextField)).unwrap();
            unsafe {
                decl.add_method(
                    sel!(mouseDown:),
                    omnibox_mouse_down as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(becomeFirstResponder),
                    omnibox_become_first_responder as extern "C" fn(&Object, Sel) -> bool,
                );
                decl.add_method(
                    sel!(resignFirstResponder),
                    omnibox_resign_first_responder as extern "C" fn(&Object, Sel) -> bool,
                );
            }
            decl.register()
        })
    })
}

extern "C" fn omnibox_mouse_down(this: &Object, _sel: Sel, event: id) {
    unsafe {
        let _: () = msg_send![super(this, class!(NSTextField)), mouseDown: event];
        let window: id = msg_send![this, window];
        if window == nil {
            return;
        }
        let editor: id = msg_send![window, fieldEditor: YES forObject: this];
        if editor != nil {
            let _: () = msg_send![editor, selectAll: nil];
        }
    }
}

extern "C" fn omnibox_become_first_responder(this: &Object, _sel: Sel) -> bool {
    unsafe {
        let accepted: bool = msg_send![super(this, class!(NSTextField)), becomeFirstResponder];
        if accepted {
            set_omnibox_focus_appearance(this as *const Object as id, true);
        }
        accepted
    }
}

extern "C" fn omnibox_resign_first_responder(this: &Object, _sel: Sel) -> bool {
    unsafe {
        let accepted: bool = msg_send![super(this, class!(NSTextField)), resignFirstResponder];
        if accepted {
            set_omnibox_focus_appearance(this as *const Object as id, false);
        }
        accepted
    }
}

extern "C" fn toolbar_click(_this: &Object, _sel: Sel, sender: id) {
    let tag: i64 = unsafe { msg_send![sender, tag] };
    match tag {
        4 => send(ToolbarCmd::NewTab),
        42 => send(ToolbarCmd::ToolCancel),
        _ => {}
    }
}

extern "C" fn omnibox_action(_this: &Object, _sel: Sel, _sender: id) {
    send(ToolbarCmd::SubmitOmnibox);
}

extern "C" fn locust_start_action(_this: &Object, _sel: Sel, _sender: id) {
    send(ToolbarCmd::ToolGo);
}

extern "C" fn menu_locust_toggle(_this: &Object, _sel: Sel, _sender: id) {
    send(ToolbarCmd::LocustToggle);
}

extern "C" fn menu_toggle_profile(_this: &Object, _sel: Sel, _sender: id) {
    send(ToolbarCmd::ToggleProfile);
}

extern "C" fn menu_cmd_k(_this: &Object, _sel: Sel, _sender: id) {
    send(ToolbarCmd::OpenCmdK);
}

extern "C" fn menu_cmd_shift_k(_this: &Object, _sel: Sel, _sender: id) {
    send(ToolbarCmd::OpenCmdShiftK);
}

fn install_cmdk_key_monitor() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    let block = ConcreteBlock::new(|event: id| -> id {
        unsafe {
            let flags: u64 = msg_send![event, modifierFlags];
            let key_code: u16 = msg_send![event, keyCode];
            const CMD: u64 = 1 << 20;
            const SHIFT: u64 = 1 << 17;
            const KEY_K: u16 = 40;
            const KEY_L: u16 = 37;
            const KEY_T: u16 = 17;
            const KEY_W: u16 = 13;
            const KEY_R: u16 = 15;
            const KEY_P: u16 = 35;
            const KEY_ESC: u16 = 53;
            if key_code == KEY_ESC {
                send(ToolbarCmd::ToolCancel);
                return event; // let page see Esc too
            }
            if (flags & CMD) == 0 {
                return event;
            }
            if key_code == KEY_K {
                if (flags & SHIFT) != 0 {
                    send(ToolbarCmd::OpenCmdShiftK);
                } else {
                    send(ToolbarCmd::OpenCmdK);
                }
                return nil;
            }
            if key_code == KEY_L && (flags & SHIFT) != 0 {
                send(ToolbarCmd::LocustToggle);
                return nil;
            }
            if key_code == KEY_P && (flags & SHIFT) != 0 {
                send(ToolbarCmd::ToggleProfile);
                return nil;
            }
            if key_code == KEY_T && (flags & SHIFT) == 0 {
                send(ToolbarCmd::NewTab);
                return nil;
            }
            if key_code == KEY_W && (flags & SHIFT) == 0 {
                // Close tab — routed via toolbar proxy as Reload-adjacent lifecycle.
                if let Ok(guard) = proxy_slot().lock() {
                    if let Some(proxy) = guard.as_ref() {
                        let _ = proxy.send_event(UserEvent::CloseActiveTab);
                    }
                }
                return nil;
            }
            if key_code == KEY_R && (flags & SHIFT) == 0 {
                send(ToolbarCmd::Reload);
                return nil;
            }
            event
        }
    });
    let block = block.copy();
    unsafe {
        let mask: u64 = 1 << 10;
        let _: id = msg_send![
            class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: mask
            handler: &*block
        ];
    }
    std::mem::forget(block);
}

fn install_palette_key_equivalents() {
    unsafe {
        let app = NSApp();
        let main_menu: id = msg_send![app, mainMenu];
        if main_menu == nil {
            return;
        }
        let view_item: id = msg_send![class!(NSMenuItem), alloc];
        let view_item: id = msg_send![
            view_item,
            initWithTitle: nsstring("View")
            action: nil
            keyEquivalent: nsstring("")
        ];
        let view_menu: id = msg_send![class!(NSMenu), alloc];
        let view_menu: id = msg_send![view_menu, initWithTitle: nsstring("View")];
        let target = toolbar_target();

        let cmd_k: id = msg_send![class!(NSMenuItem), alloc];
        let cmd_k: id = msg_send![
            cmd_k,
            initWithTitle: nsstring("Command Palette")
            action: sel!(thorOpenCmdK:)
            keyEquivalent: nsstring("k")
        ];
        let _: () = msg_send![cmd_k, setTarget: target];
        let _: () = msg_send![cmd_k, setKeyEquivalentModifierMask: (1u64 << 20)];

        let cmd_shift_k: id = msg_send![class!(NSMenuItem), alloc];
        let cmd_shift_k: id = msg_send![
            cmd_shift_k,
            initWithTitle: nsstring("Settings Palette")
            action: sel!(thorOpenCmdShiftK:)
            keyEquivalent: nsstring("k")
        ];
        let _: () = msg_send![cmd_shift_k, setTarget: target];
        let _: () = msg_send![
            cmd_shift_k,
            setKeyEquivalentModifierMask: (1u64 << 20) | (1u64 << 17)
        ];

        let _: () = msg_send![view_menu, addItem: cmd_k];
        let _: () = msg_send![view_menu, addItem: cmd_shift_k];

        let load_item: id = msg_send![class!(NSMenuItem), alloc];
        let load_item: id = msg_send![
            load_item,
            initWithTitle: nsstring("Load Testing")
            action: sel!(thorLocustToggleMenu:)
            keyEquivalent: nsstring("l")
        ];
        let _: () = msg_send![load_item, setTarget: target];
        let _: () = msg_send![
            load_item,
            setKeyEquivalentModifierMask: (1u64 << 20) | (1u64 << 17)
        ];

        let prof_item: id = msg_send![class!(NSMenuItem), alloc];
        let prof_item: id = msg_send![
            prof_item,
            initWithTitle: nsstring("Toggle Profile")
            action: sel!(thorToggleProfileMenu:)
            keyEquivalent: nsstring("p")
        ];
        let _: () = msg_send![prof_item, setTarget: target];
        let _: () = msg_send![
            prof_item,
            setKeyEquivalentModifierMask: (1u64 << 20) | (1u64 << 17)
        ];

        let _: () = msg_send![view_menu, addItem: load_item];
        let _: () = msg_send![view_menu, addItem: prof_item];
        let _: () = msg_send![view_item, setSubmenu: view_menu];
        let _: () = msg_send![main_menu, addItem: view_item];
    }
}

pub fn configure_unified_titlebar(window: &Window) {
    unsafe {
        let ns_window = window.ns_window() as id;
        if ns_window == nil {
            return;
        }
        let _: () = msg_send![ns_window, setTitleVisibility: 1u64];
        let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: YES];
        let _: () = msg_send![ns_window, setStyleMask: {
            let mask: u64 = msg_send![ns_window, styleMask];
            mask | (1u64 << 15)
        }];
        let _: () = msg_send![ns_window, setBackgroundColor: ns_color_rgba(0.07, 0.075, 0.09, 1.0)];
    }
    window.set_fullsize_content_view(true);
    window.set_titlebar_transparent(true);
    window.set_traffic_light_inset(LogicalPosition::new(
        TRAFFIC_LIGHT_X,
        traffic_light_inset_y(),
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeToolMode {
    Browse,
    Locust,
    DevTools,
    Engine,
    Profile,
    IdeVscode,
    IdeCursor,
    IdeWebstorm,
    Ssl,
    Shield,
    ClearHistory,
    Memory,
    WebVitals,
    Sla,
    Idor,
    Vault,
}

impl ChromeToolMode {
    pub fn from_action_key(key: &str) -> Option<Self> {
        match key {
            "locust_active_origin" | "toggle_locust_panel" => Some(Self::Locust),
            "toggle_devtools" => Some(Self::DevTools),
            "engine_status" => Some(Self::Engine),
            "toggle_profile" => Some(Self::Profile),
            "open_ide_vscode" => Some(Self::IdeVscode),
            "open_ide_cursor" => Some(Self::IdeCursor),
            "open_ide_webstorm" => Some(Self::IdeWebstorm),
            "show_ssl_details" => Some(Self::Ssl),
            "toggle_security_shield" => Some(Self::Shield),
            "clear_history" => Some(Self::ClearHistory),
            "trigger_memory_compaction" => Some(Self::Memory),
            "show_web_vitals" => Some(Self::WebVitals),
            "show_sla_metrics" => Some(Self::Sla),
            "run_idor_matrix" | "idor" => Some(Self::Idor),
            "open_bitwarden_vault" | "bitwarden_vault" | "toggle_vault" => Some(Self::Vault),
            _ => None,
        }
    }

    pub fn is_browse(self) -> bool {
        matches!(self, Self::Browse)
    }
}

/// Locust params read from the inline chrome fields.
#[derive(Debug, Clone)]
pub struct LocustBarParams {
    pub users: u32,
    pub ramp_up_secs: u32,
    pub runtime_secs: u64,
    pub profile: String,
}

pub struct NativeToolbar {
    pub root: id,
    pub url_glass: id,
    pub url_field: id,
    load_bar: id,
    load_progress: Cell<f64>,
    plus_btn: id,
    tool_f0: id,
    tool_f1: id,
    tool_f2: id,
    tool_f3: id,
    status_label: id,
    go_btn: id,
    cancel_btn: id,
    mode: ChromeToolMode,
    bar_width: f64,
}

impl NativeToolbar {
    pub(crate) fn attach(window: &Window, proxy: EventLoopProxy<UserEvent>) -> Self {
        *proxy_slot().lock().unwrap() = Some(proxy);
        configure_unified_titlebar(window);
        // Tool modes keep their controls readable instead of allowing the
        // omnibox and action buttons to overlap at compressed window widths.
        window.set_min_inner_size(Some(LogicalSize::new(760.0, 480.0)));

        let ns_view = window.ns_view() as id;
        let scale = window.scale_factor();
        let logical = window.inner_size().to_logical::<f64>(scale);
        let w = logical.width;
        let y = row_y();

        unsafe {
            let app = NSApp();
            let _: () = msg_send![app, activateIgnoringOtherApps: YES];

            let parent_frame: NSRect = msg_send![ns_view, frame];
            let bar_rect = NSRect::new(
                NSPoint::new(0.0, parent_frame.size.height - CHROME_HEIGHT),
                NSSize::new(parent_frame.size.width, CHROME_HEIGHT),
            );

            let root: id = msg_send![class!(NSView), alloc];
            let root: id = msg_send![root, initWithFrame: bar_rect];
            let _: () = msg_send![root, setWantsLayer: YES];
            let layer: id = msg_send![root, layer];
            if layer != nil {
                let cg: id = msg_send![ns_color_rgba(0.07, 0.075, 0.09, 1.0), CGColor];
                let _: () = msg_send![layer, setBackgroundColor: cg];
            }
            let _: () = msg_send![root, setAutoresizingMask: (1u64 << 1) | (1u64 << 3)];

            let mut x = TOOLBAR_CONTENT_LEADING;
            let plus = make_flush_button(
                "+",
                4,
                NSRect::new(NSPoint::new(x, y), NSSize::new(28.0, ROW_H)),
            );
            let _: () = msg_send![root, addSubview: plus];
            x += 28.0 + 8.0;

            let trail = 12.0;
            let field_w = (w - x - trail).max(160.0);
            let (glass, field, load_bar) =
                make_glass_omnibox(NSRect::new(NSPoint::new(x, y), NSSize::new(field_w, ROW_H)));
            let _: () = msg_send![root, addSubview: glass];

            let mk_field = |placeholder: &str, value: &str, fx: f64, fw: f64| -> id {
                let f: id = msg_send![class!(NSTextField), alloc];
                let f: id = msg_send![
                    f,
                    initWithFrame: NSRect::new(NSPoint::new(fx, y), NSSize::new(fw, ROW_H))
                ];
                style_inline_field(f, placeholder);
                let _: () = msg_send![f, setStringValue: nsstring(value)];
                let _: () = msg_send![f, setHidden: YES];
                let _: () = msg_send![root, addSubview: f];
                f
            };

            let tool_f0 = mk_field("users", "5", x, 52.0);
            let tool_f1 = mk_field("ramp", "1", x, 52.0);
            let tool_f2 = mk_field("secs", "30", x, 52.0);
            let tool_f3 = mk_field("profile", "constant", x, 72.0);

            let status_label: id = msg_send![class!(NSTextField), alloc];
            let status_label: id = msg_send![
                status_label,
                initWithFrame: NSRect::new(NSPoint::new(x, y), NSSize::new(200.0, ROW_H))
            ];
            let _: () = msg_send![status_label, setBezeled: NO];
            let _: () = msg_send![status_label, setBordered: NO];
            let _: () = msg_send![status_label, setDrawsBackground: NO];
            let _: () = msg_send![status_label, setEditable: NO];
            let _: () = msg_send![status_label, setSelectable: NO];
            let _: () = msg_send![status_label, setFocusRingType: 1u64];
            let sfont: id = msg_send![class!(NSFont), systemFontOfSize: 11.0f64];
            let _: () = msg_send![status_label, setFont: sfont];
            let _: () = msg_send![status_label, setTextColor: ns_color_rgba(0.75, 0.80, 0.88, 1.0)];
            let _: () = msg_send![status_label, setStringValue: nsstring("")];
            let _: () = msg_send![status_label, setHidden: YES];
            let _: () = msg_send![root, addSubview: status_label];

            let go_btn = make_flush_button(
                "Go",
                50,
                NSRect::new(NSPoint::new(x, y), NSSize::new(48.0, ROW_H)),
            );
            let _: () = msg_send![go_btn, setAction: sel!(thorLocustStart:)];
            let _: () = msg_send![go_btn, setHidden: YES];
            let _: () = msg_send![root, addSubview: go_btn];

            let cancel_btn = make_flush_button(
                "✕",
                42,
                NSRect::new(NSPoint::new(x, y), NSSize::new(28.0, ROW_H)),
            );
            let _: () = msg_send![cancel_btn, setHidden: YES];
            let _: () = msg_send![root, addSubview: cancel_btn];

            let _: () = msg_send![ns_view, addSubview: root positioned: 1 relativeTo: nil];
            window.set_traffic_light_inset(LogicalPosition::new(
                TRAFFIC_LIGHT_X,
                traffic_light_inset_y(),
            ));

            println!("⚡ [Thor] Minimal chrome ({w}×{CHROME_HEIGHT}) — + · omnibox · tools");
            install_palette_key_equivalents();
            install_cmdk_key_monitor();

            let tb = Self {
                root,
                url_glass: glass,
                url_field: field,
                load_bar,
                load_progress: Cell::new(0.0),
                plus_btn: plus,
                tool_f0,
                tool_f1,
                tool_f2,
                tool_f3,
                status_label,
                go_btn,
                cancel_btn,
                mode: ChromeToolMode::Browse,
                bar_width: w,
            };
            tb.layout_browse(false);
            tb.raise();
            tb
        }
    }

    fn content_leading() -> f64 {
        TOOLBAR_CONTENT_LEADING
    }

    fn plus_w() -> f64 {
        28.0
    }

    fn hide_tool_controls(&self) {
        unsafe {
            for f in [
                self.tool_f0,
                self.tool_f1,
                self.tool_f2,
                self.tool_f3,
                self.status_label,
                self.go_btn,
                self.cancel_btn,
            ] {
                let _: () = msg_send![f, setHidden: YES];
            }
        }
    }

    fn layout_browse(&self, animated: bool) {
        unsafe {
            let y = row_y();
            let plus_x = Self::content_leading();
            set_view_frame(
                self.plus_btn,
                NSRect::new(NSPoint::new(plus_x, y), NSSize::new(Self::plus_w(), ROW_H)),
            );
            let leading = plus_x + Self::plus_w() + 8.0;
            let trail = 12.0;
            let field_w = (self.bar_width - leading - trail).max(160.0);
            set_view_frame(
                self.url_glass,
                NSRect::new(NSPoint::new(leading, y), NSSize::new(field_w, ROW_H)),
            );
            let _: () = msg_send![self.url_glass, setHidden: NO];
            sync_omnibox_inner(self.url_glass, self.url_field);
            sync_omnibox_load_bar(self.url_glass, self.load_bar, self.load_progress.get());
            self.hide_tool_controls();
            cue_mode_change(self.url_glass, animated);
        }
    }

    fn place_row(
        &self,
        animated: bool,
        omni_frac: f64,
        fields: &[(id, f64)],
        status_w: f64,
        go_title: &str,
        go_w: f64,
    ) {
        unsafe {
            let y = row_y();
            let plus_x = Self::content_leading();
            set_view_frame(
                self.plus_btn,
                NSRect::new(NSPoint::new(plus_x, y), NSSize::new(Self::plus_w(), ROW_H)),
            );
            let leading = plus_x + Self::plus_w() + 8.0;
            let trail = 12.0;
            let avail = (self.bar_width - leading - trail).max(200.0);
            let tools_min: f64 = fields.iter().map(|(_, w)| *w + 4.0).sum::<f64>()
                + if status_w > 0.0 { status_w + 4.0 } else { 0.0 }
                + go_w
                + 4.0
                + 28.0;
            let omni_w = (avail * omni_frac)
                .max(100.0)
                .min((avail - tools_min).max(100.0));
            set_view_frame(
                self.url_glass,
                NSRect::new(NSPoint::new(leading, y), NSSize::new(omni_w, ROW_H)),
            );
            let _: () = msg_send![self.url_glass, setHidden: NO];
            sync_omnibox_inner(self.url_glass, self.url_field);
            sync_omnibox_load_bar(self.url_glass, self.load_bar, self.load_progress.get());

            self.hide_tool_controls();
            let mut x = leading + omni_w + 8.0;
            for (field, fw) in fields {
                set_view_frame(
                    *field,
                    NSRect::new(NSPoint::new(x, y), NSSize::new(*fw, ROW_H)),
                );
                let _: () = msg_send![*field, setHidden: NO];
                x += *fw + 4.0;
            }
            if status_w > 0.0 {
                set_view_frame(
                    self.status_label,
                    NSRect::new(NSPoint::new(x, y), NSSize::new(status_w, ROW_H)),
                );
                let _: () = msg_send![self.status_label, setHidden: NO];
                x += status_w + 4.0;
            }
            let _: () = msg_send![self.go_btn, setTitle: nsstring(go_title)];
            set_view_frame(
                self.go_btn,
                NSRect::new(NSPoint::new(x, y), NSSize::new(go_w, ROW_H)),
            );
            let _: () = msg_send![self.go_btn, setHidden: NO];
            x += go_w + 4.0;
            set_view_frame(
                self.cancel_btn,
                NSRect::new(NSPoint::new(x, y), NSSize::new(28.0, ROW_H)),
            );
            let _: () = msg_send![self.cancel_btn, setHidden: NO];
            cue_mode_change(self.url_glass, animated);
        }
    }

    fn set_field(&self, field: id, value: &str, placeholder: &str, editable: bool) {
        unsafe {
            let _: () = msg_send![field, setStringValue: nsstring(value)];
            let _: () = msg_send![field, setEditable: if editable { YES } else { NO }];
            let cell: id = msg_send![field, cell];
            if cell != nil {
                let _: () = msg_send![cell, setPlaceholderString: nsstring(placeholder)];
            }
        }
    }

    pub fn set_status(&self, text: &str) {
        unsafe {
            let _: () = msg_send![self.status_label, setStringValue: nsstring(text)];
        }
    }

    pub fn tool_mode(&self) -> ChromeToolMode {
        self.mode
    }

    pub fn set_tool_mode(&mut self, mode: ChromeToolMode) {
        let changed = self.mode != mode;
        self.mode = mode;
        self.apply_mode_layout(changed);
    }

    /// Back-compat for Locust toggle.
    pub fn set_locust_editor(&mut self, open: bool) {
        self.set_tool_mode(if open {
            ChromeToolMode::Locust
        } else {
            ChromeToolMode::Browse
        });
    }

    fn apply_mode_layout(&self, animated: bool) {
        match self.mode {
            ChromeToolMode::Browse => self.layout_browse(animated),
            ChromeToolMode::Locust => {
                self.set_field(self.tool_f0, "5", "users", true);
                self.set_field(self.tool_f1, "1", "ramp", true);
                self.set_field(self.tool_f2, "30", "secs", true);
                self.set_field(self.tool_f3, "constant", "profile", true);
                self.place_row(
                    animated,
                    0.32,
                    &[
                        (self.tool_f0, 56.0),
                        (self.tool_f1, 56.0),
                        (self.tool_f2, 56.0),
                        (self.tool_f3, 88.0),
                    ],
                    0.0,
                    "Go",
                    36.0,
                );
            }
            ChromeToolMode::DevTools => {
                self.set_status("WebKit Inspector");
                self.place_row(animated, 0.45, &[], 160.0, "Toggle", 56.0);
            }
            ChromeToolMode::Engine => {
                self.set_status("chromium · M2 · CDP");
                self.place_row(animated, 0.40, &[], 220.0, "Refresh", 56.0);
            }
            ChromeToolMode::Profile => {
                self.set_status("DevHost ↔ Stealth");
                self.place_row(animated, 0.45, &[], 160.0, "Switch", 56.0);
            }
            ChromeToolMode::IdeVscode | ChromeToolMode::IdeCursor | ChromeToolMode::IdeWebstorm => {
                let name = match self.mode {
                    ChromeToolMode::IdeVscode => "VS Code",
                    ChromeToolMode::IdeCursor => "Cursor",
                    _ => "WebStorm",
                };
                self.set_field(self.tool_f0, ".", "path", true);
                self.set_status(name);
                self.place_row(animated, 0.35, &[(self.tool_f0, 160.0)], 80.0, "Open", 48.0);
            }
            ChromeToolMode::Ssl => {
                self.set_status("ECDSA P-256 local CA");
                self.place_row(animated, 0.40, &[], 180.0, "Install", 56.0);
            }
            ChromeToolMode::Shield => {
                self.set_status("Strip utm_*/gclid/fbclid");
                self.place_row(animated, 0.40, &[], 180.0, "Toggle", 56.0);
            }
            ChromeToolMode::ClearHistory => {
                self.set_status("Clear all tab visit history?");
                self.place_row(animated, 0.40, &[], 200.0, "Clear", 52.0);
            }
            ChromeToolMode::Memory => {
                self.set_status("Close idle tabs (≥24h)");
                self.place_row(animated, 0.40, &[], 180.0, "Run", 48.0);
            }
            ChromeToolMode::WebVitals => {
                self.set_status("LCP / CLS / INP from page");
                self.place_row(animated, 0.35, &[], 180.0, "Capture", 56.0);
            }
            ChromeToolMode::Sla => {
                self.set_status("Honest M1 capability readout");
                self.place_row(animated, 0.35, &[], 200.0, "Show", 48.0);
            }
            ChromeToolMode::Idor => {
                self.set_status("Run IDOR Matrix Scan");
                self.place_row(animated, 0.40, &[], 180.0, "Scan", 52.0);
            }
            ChromeToolMode::Vault => {
                self.set_field(self.tool_f0, "", "master password", true);
                self.set_status("Bitwarden vault");
                self.place_row(
                    animated,
                    0.30,
                    &[(self.tool_f0, 220.0)],
                    60.0,
                    "Unlock",
                    56.0,
                );
            }
        }
        self.raise();
    }

    pub fn read_locust_params(&self) -> LocustBarParams {
        let users = read_nsstring(self.tool_f0)
            .parse::<u32>()
            .unwrap_or(5)
            .max(1);
        let ramp = read_nsstring(self.tool_f1).parse::<u32>().unwrap_or(1);
        let runtime = read_nsstring(self.tool_f2)
            .parse::<u64>()
            .unwrap_or(30)
            .max(1);
        let profile = {
            let p = read_nsstring(self.tool_f3);
            if p.is_empty() {
                "constant".into()
            } else {
                p
            }
        };
        LocustBarParams {
            users,
            ramp_up_secs: ramp,
            runtime_secs: runtime,
            profile,
        }
    }

    pub fn read_ide_path(&self) -> String {
        let p = read_nsstring(self.tool_f0);
        if p.is_empty() {
            ".".into()
        } else {
            p
        }
    }

    /// Master password from Vault tool field (never logged by callers).
    pub fn read_vault_password(&self) -> String {
        read_nsstring(self.tool_f0)
    }

    pub fn set_url(&self, url: &str) {
        unsafe {
            let _: () = msg_send![self.url_field, setStringValue: nsstring(url)];
        }
    }

    /// Show/hide the red omnibar load bar. `true` → indeterminate mid-fill; `false` → clear.
    pub fn set_omnibox_loading(&self, loading: bool) {
        if loading {
            self.set_omnibox_progress(0.35);
        } else {
            self.set_omnibox_progress(1.0);
            self.set_omnibox_progress(0.0);
        }
    }

    /// `progress` in 0.0..=1.0. Zero hides the red bar.
    pub fn set_omnibox_progress(&self, progress: f64) {
        let pct = progress.clamp(0.0, 1.0);
        self.load_progress.set(pct);
        sync_omnibox_load_bar(self.url_glass, self.load_bar, pct);
    }

    pub fn read_url(&self) -> String {
        read_nsstring(self.url_field)
    }

    pub fn raise(&self) {
        unsafe {
            let superview: id = msg_send![self.root, superview];
            if superview != nil {
                let _: () =
                    msg_send![superview, addSubview: self.root positioned: 1 relativeTo: nil];
            }
        }
    }

    pub fn focus_omnibox(&self) {
        unsafe {
            self.raise();
            let window: id = msg_send![self.root, window];
            if window != nil {
                let _: () = msg_send![self.url_field, setEditable: YES];
                let _: () = msg_send![self.url_field, setEnabled: YES];
                let ok: bool = msg_send![window, makeFirstResponder: self.url_field];
                if ok {
                    // Chrome-style: select all text when the omnibox gains focus
                    // so the user can immediately start typing a new URL.
                    let editor: id = msg_send![window, fieldEditor: YES forObject: self.url_field];
                    if editor != nil {
                        let _: () = msg_send![editor, selectAll: nil];
                    }
                } else {
                    println!("⚡ [Thor] omnibox failed to become first responder");
                }
            }
        }
    }

    pub fn focus_tool_field(&self) {
        unsafe {
            let window: id = msg_send![self.root, window];
            if window == nil {
                return;
            }
            let target = match self.mode {
                ChromeToolMode::Locust
                | ChromeToolMode::IdeVscode
                | ChromeToolMode::IdeCursor
                | ChromeToolMode::IdeWebstorm
                | ChromeToolMode::Vault => self.tool_f0,
                _ => self.go_btn,
            };
            let _: () = msg_send![window, makeFirstResponder: target];
        }
    }

    pub fn focus_locust_users(&self) {
        self.focus_tool_field();
    }

    pub fn reflow(&mut self, parent_width: f64, parent_height: f64) {
        unsafe {
            let bar_rect = NSRect::new(
                NSPoint::new(0.0, parent_height - CHROME_HEIGHT),
                NSSize::new(parent_width, CHROME_HEIGHT),
            );
            let _: () = msg_send![self.root, setFrame: bar_rect];
        }
        self.bar_width = parent_width;
        self.raise();
        self.apply_mode_layout(false);
    }
}
