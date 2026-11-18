//! Native command menus stay outside untrusted page DOM and renderer IPC.
use std::cell::RefCell;

mod actions;
mod catalog;

use super::{active_browser, theme, with_host};
use crate::dx::bookmarks::BookmarkStore;
use crate::dx::cmd_palette::{CmdPalette, CmdPaletteMode, PaletteItem};
use ::cef::*;
use actions::ChoiceDelegate;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Sites,
    Toolbox,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Self::Sites => "Sites",
            Self::Toolbox => "Toolbox",
        }
    }

    fn search_name(self) -> &'static str {
        match self {
            Self::Sites => "Search sites",
            Self::Toolbox => "Search toolbox",
        }
    }

    fn search_placeholder(self) -> &'static str {
        match self {
            Self::Sites => "Search open tabs, this tab’s history, bookmarks",
            Self::Toolbox => "Search developer tools and actions",
        }
    }
}

struct Menu {
    mode: Mode,
    controller: OverlayController,
    search: Textfield,
    results: Panel,
    footer: LabelButton,
    model: CmdPalette,
    matches: Vec<PaletteItem>,
    selected: usize,
}
thread_local! { static MENU: RefCell<Option<Menu>> = const { RefCell::new(None) }; }

pub(super) fn key(event: &KeyEvent) -> i32 {
    actions::key(event)
}

pub(super) fn close() {
    let menu = MENU.with(|cell| cell.borrow_mut().take());
    if let Some(menu) = menu {
        menu.controller.destroy();
    }
    if let Some(browser) = active_browser().and_then(|browser| browser.host()) {
        browser.set_focus(1);
    }
}

pub(super) fn open(mode: Mode) {
    let same = MENU.with(|cell| cell.borrow().as_ref().is_some_and(|menu| menu.mode == mode));
    close();
    if same {
        return;
    }
    if let Err(error) = build(mode) {
        crate::thor_warn!("thor::cef", "Command menu unavailable: {error}");
    }
}

fn label(text: &str) -> Option<LabelButton> {
    let mut delegate = ChoiceDelegate::new(None);
    let label = label_button_create(Some(&mut delegate), Some(&CefString::from(text)))?;
    label.set_enabled(0);
    label.set_font_list(Some(&CefString::from("Arial, 12px")));
    label.set_text_color(ButtonState::DISABLED, theme::MUTED);
    label.set_background_color(theme::INSET);
    label.set_horizontal_alignment(HorizontalAlignment::LEFT);
    Some(label)
}

fn build(mode: Mode) -> anyhow::Result<()> {
    let window = with_host(|host| host.window.clone())
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("No browser window"))?;
    let (model, unavailable) = menu_model(mode);
    let panel = menu_panel()?;
    add_header(&panel, mode)?;
    let search = add_search(&panel, mode)?;
    let results = add_results(&panel)?;
    let footer = add_footer(&panel, unavailable)?;
    let controller = create_overlay(&window, &panel)?;
    MENU.with(|cell| {
        *cell.borrow_mut() = Some(Menu {
            mode,
            controller,
            search: search.clone(),
            results,
            footer,
            model,
            matches: vec![],
            selected: 0,
        })
    });
    refresh();
    window.activate();
    search.request_focus();
    Ok(())
}

fn menu_model(mode: Mode) -> (CmdPalette, bool) {
    let bookmarks = (mode == Mode::Sites).then(BookmarkStore::open);
    let unavailable = bookmarks.as_ref().is_some_and(Result::is_err);
    let items = if mode == Mode::Sites {
        let bookmarks = bookmarks
            .and_then(Result::ok)
            .map(|store| store.to_palette_items())
            .unwrap_or_default();
        with_host(|host| catalog::sites(&host.session, bookmarks)).unwrap_or_default()
    } else {
        catalog::toolbox()
    };
    let palette_mode = match mode {
        Mode::Sites => CmdPaletteMode::BookmarksAndTabHistory,
        Mode::Toolbox => CmdPaletteMode::SettingsMenu,
    };
    let model = CmdPalette::new(palette_mode);
    model.set_items(items);
    (model, unavailable)
}

fn menu_panel() -> anyhow::Result<Panel> {
    let mut delegate = MenuPanelDelegate::new(theme::INSET);
    let panel =
        panel_create(Some(&mut delegate)).ok_or_else(|| anyhow::anyhow!("No menu panel"))?;
    panel.set_background_color(theme::INSET);
    panel.set_to_box_layout(Some(&BoxLayoutSettings {
        cross_axis_alignment: AxisAlignment::STRETCH,
        inside_border_insets: Insets {
            top: 12,
            left: 16,
            bottom: 12,
            right: 16,
        },
        between_child_spacing: 8,
        ..Default::default()
    }));
    Ok(panel)
}

fn add_header(panel: &Panel, mode: Mode) -> anyhow::Result<()> {
    let header = panel_create(None).ok_or_else(|| anyhow::anyhow!("No menu header"))?;
    let header_layout = header.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        cross_axis_alignment: AxisAlignment::CENTER,
        ..Default::default()
    }));
    if let Some(title) = label(mode.title()) {
        title.set_text_color(ButtonState::DISABLED, theme::TEXT);
        title.set_font_list(Some(&CefString::from("Arial, 16px")));
        header.add_child_view(Some(&mut View::from(&title)));
        if let Some(layout) = header_layout {
            layout.set_flex_for_view(Some(&mut View::from(&title)), 1);
        }
    }
    let mut dismiss = ChoiceDelegate::new(None);
    let close = label_button_create(Some(&mut dismiss), Some(&CefString::from("Esc · Close")))
        .ok_or_else(|| anyhow::anyhow!("No menu close control"))?;
    close.set_accessible_name(Some(&CefString::from("Close command menu")));
    close.set_enabled_text_colors(theme::MUTED);
    close.set_background_color(theme::INSET);
    header.add_child_view(Some(&mut View::from(&close)));
    panel.add_child_view(Some(&mut View::from(&header)));
    Ok(())
}

fn add_search(panel: &Panel, mode: Mode) -> anyhow::Result<Textfield> {
    let mut delegate = SearchDelegate::new();
    let search =
        textfield_create(Some(&mut delegate)).ok_or_else(|| anyhow::anyhow!("No menu search"))?;
    search.set_accessible_name(Some(&CefString::from(mode.search_name())));
    search.set_placeholder_text(Some(&CefString::from(mode.search_placeholder())));
    search.set_font_list(Some(&CefString::from("Arial, 14px")));
    search.set_background_color(theme::INSET);
    search.set_size(Some(&Size {
        width: 0,
        height: 40,
    }));
    panel.add_child_view(Some(&mut View::from(&search)));
    Ok(search)
}

fn add_results(panel: &Panel) -> anyhow::Result<Panel> {
    let results = panel_create(None).ok_or_else(|| anyhow::anyhow!("No result list"))?;
    results.set_to_box_layout(Some(&BoxLayoutSettings {
        cross_axis_alignment: AxisAlignment::STRETCH,
        between_child_spacing: 2,
        ..Default::default()
    }));
    panel.add_child_view(Some(&mut View::from(&results)));
    Ok(results)
}

fn add_footer(panel: &Panel, unavailable: bool) -> anyhow::Result<LabelButton> {
    let footer = label(if unavailable {
        "Bookmarks unavailable. Open tabs and tab history still work."
    } else {
        "↑ ↓ Select     Return Open     Esc Close"
    })
    .ok_or_else(|| anyhow::anyhow!("No menu guidance"))?;
    panel.add_child_view(Some(&mut View::from(&footer)));
    Ok(footer)
}

fn create_overlay(window: &Window, panel: &Panel) -> anyhow::Result<OverlayController> {
    let mut border_delegate = MenuPanelDelegate::new(0xff34404d);
    let border = panel_create(Some(&mut border_delegate))
        .ok_or_else(|| anyhow::anyhow!("No menu boundary"))?;
    let border_layout = border.set_to_box_layout(Some(&BoxLayoutSettings {
        cross_axis_alignment: AxisAlignment::STRETCH,
        inside_border_insets: Insets {
            top: 1,
            left: 1,
            bottom: 1,
            right: 1,
        },
        ..Default::default()
    }));
    border.add_child_view(Some(&mut View::from(panel)));
    if let Some(layout) = border_layout {
        layout.set_flex_for_view(Some(&mut View::from(panel)), 1);
    }
    let controller = window
        .add_overlay_view(Some(&mut View::from(&border)), DockingMode::CUSTOM, 1)
        .ok_or_else(|| anyhow::anyhow!("No command overlay"))?;
    border.set_background_color(0xff34404d);
    panel.set_background_color(theme::INSET);
    Ok(controller)
}

pub(super) fn refresh() {
    MENU.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(menu) = state.as_mut() else {
            return;
        };
        menu.model
            .set_query(CefString::from(&menu.search.text()).to_string());
        menu.matches = menu.model.search();
        menu.selected = menu.selected.min(menu.matches.len().saturating_sub(1));
        render_results(menu);
        update_footer(menu);
        update_bounds(menu);
        menu.controller.set_visible(1);
    });
}

fn render_results(menu: &mut Menu) {
    menu.results.remove_all_child_views();
    let start = (menu.selected / 7) * 7;
    for (index, item) in menu.matches.iter().enumerate().skip(start).take(7) {
        append_result(&menu.results, item, index == menu.selected, index);
    }
    if menu.matches.is_empty() {
        if let Some(empty) = label("No matches. Try another title or URL.") {
            menu.results.add_child_view(Some(&mut View::from(&empty)));
        }
    }
}

fn append_result(results: &Panel, item: &PaletteItem, selected: bool, index: usize) {
    let mut delegate = ChoiceDelegate::new(Some(index));
    let title = format!("{}    · {}", item.title, item.category);
    let Some(button) =
        label_button_create(Some(&mut delegate), Some(&CefString::from(title.as_str())))
    else {
        return;
    };
    button.set_accessible_name(Some(&CefString::from(
        format!("{} — {}", item.title, item.category).as_str(),
    )));
    button.set_tooltip_text(Some(&CefString::from(item.path_or_url.as_str())));
    button.set_font_list(Some(&CefString::from("Arial, 13px")));
    button.set_horizontal_alignment(HorizontalAlignment::LEFT);
    button.set_minimum_size(Some(&Size {
        width: 0,
        height: 44,
    }));
    results.add_child_view(Some(&mut View::from(&button)));
    button.set_background_color(if selected { 0xff202831 } else { theme::INSET });
    button.set_enabled_text_colors(if selected { theme::ACCENT } else { theme::TEXT });
}

fn update_footer(menu: &Menu) {
    let count = menu.matches.len();
    let text = if count == 0 {
        "Esc Close".to_string()
    } else {
        format!(
            "{} of {count}     ↑ ↓ Select     Return Open     Esc Close",
            menu.selected + 1
        )
    };
    menu.footer.set_text(Some(&CefString::from(text.as_str())));
}

fn update_bounds(menu: &Menu) {
    if let Some(window) = menu.controller.window() {
        let size = window.size();
        let width = (size.width - 48).clamp(300, 680);
        menu.controller.set_bounds(Some(&Rect {
            x: (size.width - width) / 2,
            y: 76,
            width,
            height: 142 + (menu.matches.len().clamp(1, 7) as i32) * 46,
        }));
    }
}

wrap_panel_delegate! {
    struct MenuPanelDelegate {color:u32}
    impl ViewDelegate {
        fn on_theme_changed(&self,view:Option<&mut View>) {if let Some(view)=view {view.set_background_color(self.color);}}
    }
    impl PanelDelegate {}
}

wrap_textfield_delegate! {
    struct SearchDelegate {}
    impl ViewDelegate {
        fn preferred_size(&self,_view:Option<&mut View>)->Size {Size {width:400,height:40}}
        fn minimum_size(&self,_view:Option<&mut View>)->Size {Size {width:0,height:40}}
    }
    impl TextfieldDelegate {
        fn on_after_user_action(&self,_textfield:Option<&mut Textfield>) {
            MENU.with(|cell|{if let Some(menu)=cell.borrow_mut().as_mut(){menu.selected=0;}}); refresh();
        }
        fn on_key_event(&self,_textfield:Option<&mut Textfield>,event:Option<&KeyEvent>)->i32 {event.map_or(0,|event|{let handled=key(event);if handled==1 {1}else{super::shortcut(event)}})}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::session::BrowserSession;
    use crate::dx::cmd_palette::PaletteItemType;
    #[test]
    fn sites_expose_only_active_tab_history_and_remove_closed_tabs() {
        let mut session = BrowserSession::default();
        let first = session.open_tab(0).unwrap();
        session
            .record_navigation(first, "https://first.test", "First")
            .unwrap();
        let second = session.open_tab(0).unwrap();
        session
            .record_navigation(second, "https://second.test", "Second")
            .unwrap();
        session.activate(second).unwrap();
        let bookmark = catalog::item(
            "bookmark",
            "Saved",
            "https://saved.test",
            "Bookmarks",
            PaletteItemType::Bookmark,
        );
        let result = catalog::sites(&session, vec![bookmark.clone()]);
        assert_eq!(
            result
                .iter()
                .filter(|item| item.item_type == PaletteItemType::Tab)
                .count(),
            2
        );
        assert!(result
            .iter()
            .any(|item| item.item_type == PaletteItemType::TabHistory
                && item.path_or_url == "https://second.test"));
        assert!(!result
            .iter()
            .any(|item| item.item_type == PaletteItemType::TabHistory
                && item.path_or_url == "https://first.test"));
        session.close_tab(second).unwrap();
        let result = catalog::sites(&session, vec![bookmark]);
        assert!(!result
            .iter()
            .any(|item| item.path_or_url == "https://second.test"));
        assert!(result
            .iter()
            .any(|item| item.item_type == PaletteItemType::Bookmark));
    }
}
