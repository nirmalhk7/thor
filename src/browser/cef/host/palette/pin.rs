use super::actions::{PinAction, PinDelegate};
use super::{row_metrics, theme, Menu, Mode};
use crate::dx::cmd_palette::PaletteItem;
use ::cef::*;

pub(super) fn append_to_row(row: &Panel, mode: Mode, item: &PaletteItem, background: u32) {
    if mode != Mode::Sites {
        return;
    }
    let Some(pin) = PinAction::for_item(item) else {
        return;
    };
    let mut delegate = PinDelegate::new(pin);
    let Some(button) =
        label_button_create(Some(&mut delegate), Some(&CefString::from(pin.label())))
    else {
        return;
    };
    let name = format!("{} {}", pin.label(), item.title);
    button.set_accessible_name(Some(&CefString::from(name.as_str())));
    button.set_tooltip_text(Some(&CefString::from(name.as_str())));
    button.set_font_list(Some(&CefString::from(theme::FONT)));
    button.set_background_color(background);
    button.set_enabled_text_colors(theme::MUTED);
    button.set_focusable(1);
    let size = Size {
        width: 54,
        height: row_metrics(mode).0,
    };
    button.set_minimum_size(Some(&size));
    button.set_maximum_size(Some(&size));
    row.add_child_view(Some(&mut View::from(&button)));
}

pub(super) fn shortcut_hint(menu: &Menu) -> String {
    if menu.mode != Mode::Sites {
        return String::new();
    }
    menu.matches
        .get(menu.selected)
        .and_then(PinAction::for_item)
        .map_or_else(String::new, |pin| format!("     ⌥↵ {}", pin.label()))
}
