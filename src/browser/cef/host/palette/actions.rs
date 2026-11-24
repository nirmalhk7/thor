use super::super::{activate, active_browser, perform, with_host, Action};
use super::{close, open, refresh, refresh_sites_selecting, Mode, MENU};
use crate::dx::bookmarks::BookmarkStore;
use crate::dx::cmd_palette::{PaletteItem, PaletteItemType};
use ::cef::*;

pub(super) fn key(event: &KeyEvent) -> i32 {
    if !is_key_down(event) || MENU.with(|cell| cell.borrow().is_none()) {
        return 0;
    }
    match event.windows_key_code {
        27 => defer(None),
        13 if search_has_focus() => {
            if event.modifiers & sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0 != 0
                && MENU.with(|cell| {
                    cell.borrow()
                        .as_ref()
                        .is_some_and(|menu| menu.mode == Mode::Sites)
                })
            {
                // Holding Option+Return should not toggle the page repeatedly.
                if event.modifiers & sys::cef_event_flags_t::EVENTFLAG_IS_REPEAT.0 == 0 {
                    pin_selected();
                }
            } else {
                choose_selected();
            }
        }
        38 | 40 if search_has_focus() => move_selection(event.windows_key_code == 40),
        _ => return 0,
    }
    1
}

fn search_has_focus() -> bool {
    MENU.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|menu| menu.search.has_focus() == 1)
    })
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

fn pin_selected() {
    let pin = MENU.with(|cell| {
        let state = cell.borrow();
        let menu = state.as_ref().filter(|menu| menu.mode == Mode::Sites)?;
        PinAction::for_item(menu.matches.get(menu.selected)?)
    });
    if let Some(pin) = pin {
        defer_pin(pin);
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

fn snapshot_choice(index: usize) -> Option<(Mode, PaletteItem)> {
    MENU.with(|cell| {
        let state = cell.borrow();
        let menu = state.as_ref()?;
        Some((menu.mode, menu.matches.get(index)?.clone()))
    })
}

fn choose(mode: Mode, item: PaletteItem) {
    let still_available = MENU.with(|cell| {
        cell.borrow().as_ref().is_some_and(|menu| {
            menu.mode == mode
                && menu
                    .matches
                    .iter()
                    .any(|current| current.id == item.id && current.path_or_url == item.path_or_url)
        })
    });
    if !still_available {
        return;
    }
    close();
    match mode {
        Mode::Sites => choose_site(item),
        Mode::Toolbox | Mode::Security => choose_tool(item),
        Mode::Extensions => {}
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

#[derive(Clone, Copy)]
pub(super) struct PinAction {
    id: u64,
    pin: bool,
}

impl PinAction {
    pub(super) fn for_item(item: &PaletteItem) -> Option<Self> {
        if item.item_type != PaletteItemType::Tab {
            return None;
        }
        Some(Self {
            id: tab_id(item)?,
            pin: item.badge.as_deref() != Some("Pinned"),
        })
    }

    pub(super) fn label(self) -> &'static str {
        if self.pin {
            "Pin"
        } else {
            "Unpin"
        }
    }

    fn run(self) {
        let search = MENU.with(|cell| {
            let state = cell.borrow();
            state
                .as_ref()
                .filter(|menu| menu.mode == Mode::Sites)
                .map(|menu| menu.search.clone())
        });
        let Some(search) = search else {
            return;
        };
        if let Some(Ok(())) = with_host(|host| host.session.set_pinned(self.id, self.pin)) {
            super::super::expiry::schedule();
            refresh_sites_selecting(Some(&format!("tab_{}", self.id)));
            search.request_focus();
        }
    }
}

fn defer_pin(pin: PinAction) {
    let mut task = PinTask::new(pin);
    if post_task(ThreadId::UI, Some(&mut task)) != 1 {
        crate::thor_warn!("thor::cef", "CEF rejected a Sites pin action");
    }
}

wrap_button_delegate! {
    pub(super) struct PinDelegate {pin:PinAction}
    impl ViewDelegate {}
    impl ButtonDelegate {fn on_button_pressed(&self,_button:Option<&mut Button>){defer_pin(self.pin);}}
}

wrap_task! {
    struct PinTask {pin:PinAction}
    impl Task {fn execute(&self){self.pin.run();}}
}

fn choose_tool(item: PaletteItem) {
    match item.id.as_str() {
        "inspect" => perform(&Action::Inspect),
        "new_tab" => perform(&Action::New),
        "bookmark" => bookmark_current_page(),
        "api" => perform(&Action::OpenTool(item.path_or_url)),
        "security" => open(Mode::Security),
        "extensions" => open(Mode::Extensions),
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
    let choice = match index {
        Some(index) => {
            let Some(choice) = snapshot_choice(index) else {
                return;
            };
            Some(choice)
        }
        None => None,
    };
    let mut task = ChoiceTask::new(choice);
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
    struct ChoiceTask {choice:Option<(Mode, PaletteItem)>}
    impl Task {
        fn execute(&self) {
            if let Some((mode,item)) = &self.choice { choose(*mode, item.clone()); } else { close(); }
        }
    }
}
