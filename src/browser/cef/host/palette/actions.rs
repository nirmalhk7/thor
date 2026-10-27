use super::super::{activate, active_browser, perform, with_host, Action};
use super::{close, open, refresh, Mode, MENU};
use crate::dx::bookmarks::BookmarkStore;
use crate::dx::cmd_palette::{PaletteItem, PaletteItemType};
use ::cef::*;

pub(super) fn key(event: &KeyEvent) -> i32 {
    if !is_key_down(event) || MENU.with(|cell| cell.borrow().is_none()) {
        return 0;
    }
    match event.windows_key_code {
        27 => defer(None),
        13 => choose_selected(),
        38 | 40 => move_selection(event.windows_key_code == 40),
        _ => return 0,
    }
    1
}

fn is_key_down(event: &KeyEvent) -> bool {
    matches!(
        event.type_,
        KeyEventType::RAWKEYDOWN | KeyEventType::KEYDOWN
    )
}

fn choose_selected() {
    if let Some(index) = MENU.with(|cell| cell.borrow().as_ref().map(|menu| menu.selected)) {
        defer(Some(index));
    }
}

fn move_selection(down: bool) {
    MENU.with(|cell| {
        if let Some(menu) = cell.borrow_mut().as_mut() {
            let len = menu.matches.len();
            if len > 0 {
                menu.selected = if down {
                    (menu.selected + 1) % len
                } else {
                    (menu.selected + len - 1) % len
                };
            }
        }
    });
    refresh();
}

fn choose(index: usize) {
    let selected = MENU.with(|cell| {
        cell.borrow().as_ref().and_then(|menu| {
            menu.matches
                .get(index)
                .cloned()
                .map(|item| (menu.mode, item))
        })
    });
    let Some((mode, item)) = selected else {
        return;
    };
    close();
    match mode {
        Mode::Sites => choose_site(item),
        Mode::Toolbox => choose_tool(item),
    }
}

fn choose_site(item: PaletteItem) {
    if item.item_type == PaletteItemType::Tab {
        if let Some(id) = tab_id(&item) {
            activate(id);
        }
        return;
    }
    if let Some(frame) = active_browser().and_then(|browser| browser.main_frame()) {
        frame.load_url(Some(&CefString::from(item.path_or_url.as_str())));
    }
}

fn tab_id(item: &PaletteItem) -> Option<u64> {
    item.id.strip_prefix("tab_")?.parse().ok()
}

fn choose_tool(item: PaletteItem) {
    match item.id.as_str() {
        "inspect" => perform(&Action::Inspect),
        "new_tab" => perform(&Action::New),
        "bookmark" => bookmark_current_page(),
        _ => perform(&Action::OpenTool(item.path_or_url)),
    }
}

fn bookmark_current_page() {
    let visit = with_host(|host| {
        host.session
            .summaries()
            .into_iter()
            .find(|tab| Some(tab.target.id) == host.session.active_tab())
    });
    let Some(Some(tab)) = visit else {
        return;
    };
    let result = BookmarkStore::open().and_then(|mut store| store.add(&tab.title, &tab.url));
    open(Mode::Sites);
    MENU.with(|cell| {
        if let Some(menu) = cell.borrow().as_ref() {
            let message = match result {
                Ok(_) => "Bookmark saved to Thor".to_string(),
                Err(error) => format!("Bookmark not saved: {error}"),
            };
            menu.footer
                .set_text(Some(&CefString::from(message.as_str())));
        }
    });
}

pub(super) fn defer(index: Option<usize>) {
    let mut task = ChoiceTask::new(index);
    if post_task(ThreadId::UI, Some(&mut task)) != 1 {
        crate::thor_warn!("thor::cef", "CEF rejected a command menu action");
    }
}

wrap_button_delegate! {
    pub(super) struct ChoiceDelegate {index:Option<usize>}
    impl ViewDelegate {}
    impl ButtonDelegate {fn on_button_pressed(&self,_button:Option<&mut Button>){defer(self.index);}}
}

wrap_task! {
    struct ChoiceTask {index:Option<usize>}
    impl Task {fn execute(&self){if let Some(index)=self.index{choose(index);}else{close();}}}
}
