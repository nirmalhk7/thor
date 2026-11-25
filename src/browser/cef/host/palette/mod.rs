//! Native command menus stay outside untrusted page DOM and renderer IPC.
use std::cell::RefCell;

mod actions;
mod catalog;
mod pin;

use super::{active_browser, theme, with_host};
use crate::dx::bookmarks::BookmarkStore;
use crate::dx::cmd_palette::{CmdPalette, CmdPaletteMode, PaletteItem};
use ::cef::*;
use actions::ChoiceDelegate;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Sites,
    Toolbox,
    Security,
    Extensions,
}

impl Mode {
    fn search_name(self) -> &'static str {
        match self {
            Self::Sites => "Search sites",
            Self::Toolbox => "Search toolbox",
            Self::Security => "Search security analyzers",
            Self::Extensions => "Extensions",
        }
    }

    fn search_placeholder(self) -> &'static str {
        match self {
            Self::Sites => "Search tabs, history, bookmarks",
            Self::Toolbox => "Search developer tools",
            Self::Security => "Search security analyzers",
            Self::Extensions => "Extensions",
        }
    }
}

struct Menu {
    mode: Mode,
    controller: OverlayController,
    scrim: OverlayController,
    search: Textfield,
    results: Panel,
    footer: LabelButton,
    model: CmdPalette,
    matches: Vec<PaletteItem>,
    selected: usize,
    bookmarks_unavailable: bool,
}
thread_local! { static MENU: RefCell<Option<Menu>> = const { RefCell::new(None) }; }

pub(super) fn key(event: &KeyEvent) -> i32 {
    actions::key(event)
}

pub(super) fn dismiss() -> bool {
    let menu = MENU.with(|cell| cell.borrow_mut().take());
    let Some(menu) = menu else {
        return false;
    };
    menu.controller.destroy();
    menu.scrim.destroy();
    true
}

pub(super) fn close() {
    if dismiss() {
        if let Some(browser) = active_browser().and_then(|browser| browser.host()) {
            browser.set_focus(1);
        }
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
    label.set_font_list(Some(&CefString::from(theme::FONT)));
    label.set_text_color(ButtonState::DISABLED, theme::MUTED);
    label.set_background_color(theme::DRAWER);
    label.set_horizontal_alignment(HorizontalAlignment::LEFT);
    Some(label)
}

fn build(mode: Mode) -> anyhow::Result<()> {
    let window = with_host(|host| host.window.clone())
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("No browser window"))?;
    let (model, unavailable) = menu_model(mode);
    let panel = menu_panel()?;
    let search = add_search(&panel, mode)?;
    let results = add_results(&panel, mode)?;
    let footer = add_footer(&panel, unavailable)?;
    let scrim = create_scrim(&window)?;
    let Some(controller) =
        window.add_overlay_view(Some(&mut View::from(&panel)), DockingMode::CUSTOM, 1)
    else {
        scrim.destroy();
        anyhow::bail!("No command drawer");
    };
    MENU.with(|cell| {
        *cell.borrow_mut() = Some(Menu {
            mode,
            controller,
            scrim,
            search: search.clone(),
            results,
            footer,
            model,
            matches: vec![],
            selected: 0,
            bookmarks_unavailable: unavailable,
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
    let items = match mode {
        Mode::Sites => {
            let bookmarks = bookmarks
                .and_then(Result::ok)
                .map(|store| store.to_palette_items())
                .unwrap_or_default();
            with_host(|host| catalog::sites(&host.session, bookmarks)).unwrap_or_default()
        }
        Mode::Toolbox => catalog::toolbox(),
        Mode::Security => catalog::security(),
        Mode::Extensions => Vec::new(),
    };
    let palette_mode = match mode {
        Mode::Sites => CmdPaletteMode::BookmarksAndTabHistory,
        Mode::Toolbox | Mode::Security | Mode::Extensions => CmdPaletteMode::SettingsMenu,
    };
    let model = CmdPalette::new(palette_mode);
    model.set_items(items);
    (model, unavailable)
}

fn menu_panel() -> anyhow::Result<Panel> {
    let mut delegate = MenuPanelDelegate::new(theme::DRAWER);
    let panel =
        panel_create(Some(&mut delegate)).ok_or_else(|| anyhow::anyhow!("No menu panel"))?;
    panel.set_background_color(theme::DRAWER);
    panel.set_to_box_layout(Some(&BoxLayoutSettings {
        cross_axis_alignment: AxisAlignment::STRETCH,
        inside_border_insets: Insets {
            top: 6,
            left: 8,
            bottom: 6,
            right: 8,
        },
        between_child_spacing: 8,
        ..Default::default()
    }));
    Ok(panel)
}

fn add_search(panel: &Panel, mode: Mode) -> anyhow::Result<Textfield> {
    let mut delegate = SearchDelegate::new();
    let search =
        textfield_create(Some(&mut delegate)).ok_or_else(|| anyhow::anyhow!("No menu search"))?;
    search.set_accessible_name(Some(&CefString::from(mode.search_name())));
    search.set_placeholder_text(Some(&CefString::from(mode.search_placeholder())));
    search.set_font_list(Some(&CefString::from(theme::FONT)));
    search.set_background_color(theme::FIELD);
    if mode == Mode::Extensions {
        search.set_enabled(0);
    }
    let header = panel_create(None).ok_or_else(|| anyhow::anyhow!("No drawer header"))?;
    let layout = header
        .set_to_box_layout(Some(&BoxLayoutSettings {
            horizontal: 1,
            cross_axis_alignment: AxisAlignment::CENTER,
            between_child_spacing: 4,
            ..Default::default()
        }))
        .ok_or_else(|| anyhow::anyhow!("No drawer header layout"))?;
    let mut dismiss = ChoiceDelegate::new(None);
    let close = label_button_create(Some(&mut dismiss), Some(&CefString::from("×")))
        .ok_or_else(|| anyhow::anyhow!("No drawer close control"))?;
    close.set_accessible_name(Some(&CefString::from("Close command drawer")));
    close.set_enabled_text_colors(theme::MUTED);
    close.set_background_color(theme::DRAWER);
    close.set_minimum_size(Some(&Size {
        width: 22,
        height: theme::FIELD_HEIGHT,
    }));
    close.set_maximum_size(Some(&Size {
        width: 22,
        height: theme::FIELD_HEIGHT,
    }));
    header.add_child_view(Some(&mut View::from(&search)));
    header.add_child_view(Some(&mut View::from(&close)));
    layout.set_flex_for_view(Some(&mut View::from(&search)), 1);
    panel.add_child_view(Some(&mut View::from(&header)));
    Ok(search)
}

fn add_results(panel: &Panel, mode: Mode) -> anyhow::Result<Panel> {
    let results = panel_create(None).ok_or_else(|| anyhow::anyhow!("No result list"))?;
    results.set_to_box_layout(Some(&BoxLayoutSettings {
        cross_axis_alignment: AxisAlignment::STRETCH,
        between_child_spacing: row_metrics(mode).1,
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

pub(super) fn create_scrim(window: &Window) -> anyhow::Result<OverlayController> {
    let mut delegate = ChoiceDelegate::new(None);
    let shade = label_button_create(Some(&mut delegate), Some(&CefString::from("")))
        .ok_or_else(|| anyhow::anyhow!("No drawer backdrop"))?;
    shade.set_accessible_name(Some(&CefString::from("Dismiss command drawer")));
    shade.set_focusable(0);
    let controller = window
        .add_overlay_view(Some(&mut View::from(&shade)), DockingMode::CUSTOM, 0)
        .ok_or_else(|| anyhow::anyhow!("No drawer backdrop overlay"))?;
    shade.set_background_color(theme::SCRIM);
    Ok(controller)
}

pub(super) fn refresh() {
    refresh_selecting(None);
}

fn refresh_selecting(selected_id: Option<&str>) {
    MENU.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(menu) = state.as_mut() else {
            return;
        };
        menu.model
            .set_query(CefString::from(&menu.search.text()).to_string());
        menu.matches = menu.model.search();
        menu.selected = selected_id
            .and_then(|id| menu.matches.iter().position(|item| item.id == id))
            .unwrap_or_else(|| menu.selected.min(menu.matches.len().saturating_sub(1)));
        let Some(height) = update_bounds(menu) else {
            return;
        };
        render_results(menu, height);
        update_footer(menu);
        menu.controller.set_visible(1);
    });
}

pub(super) fn refresh_sites() {
    refresh_sites_selecting(None);
}

fn refresh_sites_selecting(selected_id: Option<&str>) {
    if !MENU.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|menu| menu.mode == Mode::Sites)
    }) {
        return;
    }
    let (model, unavailable) = menu_model(Mode::Sites);
    MENU.with(|cell| {
        if let Some(menu) = cell.borrow_mut().as_mut() {
            menu.model = model;
            menu.bookmarks_unavailable = unavailable;
        }
    });
    refresh_selecting(selected_id);
}

fn row_metrics(mode: Mode) -> (i32, i32) {
    match mode {
        Mode::Sites => (28, 5),
        _ => (27, 10),
    }
}

fn render_results(menu: &mut Menu, height: i32) {
    menu.results.remove_all_child_views();
    let (row_height, row_gap) = row_metrics(menu.mode);
    let visible = ((height - 80) / (row_height + row_gap)).max(1) as usize;
    let start = (menu.selected / visible) * visible;
    for (index, item) in menu.matches.iter().enumerate().skip(start).take(visible) {
        append_result(
            &menu.results,
            item,
            index == menu.selected,
            index,
            menu.mode,
        );
    }
    if menu.matches.is_empty() {
        let message = if menu.mode == Mode::Extensions {
            "No extension panels available in Thor."
        } else {
            "No matches."
        };
        if let Some(empty) = label(message) {
            menu.results.add_child_view(Some(&mut View::from(&empty)));
        }
    }
}

fn append_result(results: &Panel, item: &PaletteItem, selected: bool, index: usize, mode: Mode) {
    let Some(row) = panel_create(None) else {
        return;
    };
    let Some(layout) = row.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        cross_axis_alignment: AxisAlignment::CENTER,
        between_child_spacing: if mode == Mode::Sites { 12 } else { 5 },
        ..Default::default()
    })) else {
        return;
    };
    let background = if selected {
        theme::FIELD
    } else {
        theme::DRAWER
    };
    row.set_background_color(background);
    let mut delegate = ChoiceDelegate::new(Some(index));
    if let Some(icon) = label_button_create(Some(&mut delegate), Some(&CefString::from(""))) {
        let kind = if mode == Mode::Sites {
            theme::Icon::Site
        } else {
            theme::Icon::Tool
        };
        for state in [
            ButtonState::NORMAL,
            ButtonState::HOVERED,
            ButtonState::PRESSED,
        ] {
            if let Some(mut image) = theme::icon(kind, theme::ICON) {
                icon.set_image(state, Some(&mut image));
            }
        }
        let size = Size {
            width: 27,
            height: row_metrics(mode).0,
        };
        icon.set_minimum_size(Some(&size));
        icon.set_maximum_size(Some(&size));
        icon.set_background_color(background);
        icon.set_focusable(0);
        icon.set_accessible_name(Some(&CefString::from(item.title.as_str())));
        row.add_child_view(Some(&mut View::from(&icon)));
    }
    let Some(text) = panel_create(None) else {
        return;
    };
    text.set_to_box_layout(Some(&BoxLayoutSettings {
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    text.set_background_color(background);
    row.add_child_view(Some(&mut View::from(&text)));
    layout.set_flex_for_view(Some(&mut View::from(&text)), 1);
    let secondary = match item.item_type {
        crate::dx::cmd_palette::PaletteItemType::Action => item.category.as_str(),
        _ => item.path_or_url.as_str(),
    };
    for (caption, color, font, focusable) in [
        (item.title.as_str(), theme::TEXT, theme::FONT, 1),
        (secondary, theme::MUTED, theme::DETAIL_FONT, 0),
    ] {
        let mut delegate = ChoiceDelegate::new(Some(index));
        let Some(button) =
            label_button_create(Some(&mut delegate), Some(&CefString::from(caption)))
        else {
            return;
        };
        button.set_accessible_name(Some(&CefString::from(
            format!("{} — {}", item.title, item.category).as_str(),
        )));
        button.set_tooltip_text(Some(&CefString::from(secondary)));
        button.set_font_list(Some(&CefString::from(font)));
        button.set_horizontal_alignment(HorizontalAlignment::LEFT);
        button.set_insets(Some(&Insets {
            left: 0,
            right: 0,
            ..Default::default()
        }));
        let size = Size {
            width: 0,
            height: if focusable == 1 {
                15
            } else {
                row_metrics(mode).0 - 15
            },
        };
        button.set_minimum_size(Some(&size));
        button.set_maximum_size(Some(&size));
        button.set_focusable(focusable);
        text.add_child_view(Some(&mut View::from(&button)));
        button.set_background_color(background);
        button.set_enabled_text_colors(color);
    }
    pin::append_to_row(&row, mode, item, background);
    results.add_child_view(Some(&mut View::from(&row)));
}

fn update_footer(menu: &Menu) {
    let count = menu.matches.len();
    let text = if count == 0 {
        "Esc Close".to_string()
    } else {
        let pin_hint = pin::shortcut_hint(menu);
        format!(
            "{} of {count}     ↑ ↓ Select     Return Open{pin_hint}     Esc Close",
            menu.selected + 1
        )
    };
    let text = if menu.bookmarks_unavailable {
        format!("Bookmarks unavailable. {text}")
    } else {
        text
    };
    menu.footer
        .set_tooltip_text(Some(&CefString::from(text.as_str())));
    menu.footer.set_text(Some(&CefString::from(text.as_str())));
}

fn update_bounds(menu: &Menu) -> Option<i32> {
    position_drawer(&menu.controller, &menu.scrim)
}

pub(super) fn position_drawer(
    controller: &OverlayController,
    backdrop: &OverlayController,
) -> Option<i32> {
    let content = with_host(|host| host.content.clone()).flatten()?;
    let mut origin = Point::default();
    if content.convert_point_to_window(Some(&mut origin)) != 1 {
        return None;
    }
    let size = content.size();
    let (scrim, drawer) = theme::drawer_bounds(Rect {
        x: origin.x,
        y: origin.y,
        width: size.width,
        height: size.height,
    });
    backdrop.set_bounds(Some(&scrim));
    backdrop.set_visible(i32::from(scrim.width > 0 && scrim.height > 0));
    controller.set_bounds(Some(&drawer));
    Some(drawer.height)
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
        fn preferred_size(&self,_view:Option<&mut View>)->Size {Size {width:190,height:theme::FIELD_HEIGHT}}
        fn minimum_size(&self,_view:Option<&mut View>)->Size {Size {width:0,height:theme::FIELD_HEIGHT}}
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
    use super::actions::PinAction;
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
    #[test]
    fn sites_model_places_pins_first_and_keeps_recent_order_for_equally_relevant_pages() {
        let mut session = BrowserSession::default();
        let ids: Vec<_> = (0..3)
            .map(|_| {
                let tab = session.open_tab(0).unwrap();
                session
                    .record_navigation(tab, "https://same.test", "Same title")
                    .unwrap();
                tab
            })
            .collect();
        session.activate(ids[0]).unwrap();
        session.set_pinned(ids[1], true).unwrap();

        let mut model = CmdPalette::new(CmdPaletteMode::BookmarksAndTabHistory);
        model.set_items(catalog::sites(&session, Vec::new()));
        for query in ["", "Same title"] {
            model.set_query(query.into());
            let pages: Vec<_> = model
                .search()
                .into_iter()
                .filter(|entry| entry.item_type == PaletteItemType::Tab)
                .collect();
            assert_eq!(
                pages
                    .iter()
                    .map(|page| page.id.as_str())
                    .collect::<Vec<_>>(),
                ["tab_2", "tab_1", "tab_3"]
            );
            assert_eq!(PinAction::for_item(&pages[0]).unwrap().label(), "Unpin");
            assert_eq!(PinAction::for_item(&pages[1]).unwrap().label(), "Pin");
        }
        let history = model
            .search()
            .into_iter()
            .find(|entry| entry.item_type == PaletteItemType::TabHistory)
            .unwrap();
        assert!(PinAction::for_item(&history).is_none());
    }
}
