use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    PauseAll,
    ResumeAll,
    Quit,
}

pub struct OrbitTray {
    tray_icon: TrayIcon,
    show_id: MenuId,
    pause_all_id: MenuId,
    resume_all_id: MenuId,
    quit_id: MenuId,
    status_item: MenuItem,
    last_status: RefCell<String>,
    last_show: Cell<Option<Instant>>,
}

impl OrbitTray {
    pub fn new(icon_data: &eframe::egui::IconData) -> anyhow::Result<Self> {
        let show = MenuItem::with_id("orbit.tray.show", "Show Orbit", true, None);
        let status = MenuItem::with_id("orbit.tray.status", "No active transfers", false, None);
        let pause_all = MenuItem::with_id("orbit.tray.pause-all", "Pause all", true, None);
        let resume_all = MenuItem::with_id("orbit.tray.resume-all", "Resume all", true, None);
        let quit = MenuItem::with_id("orbit.tray.quit", "Quit Orbit", true, None);

        let menu = Menu::with_items(&[&show, &status, &pause_all, &resume_all, &quit])?;
        let icon =
            tray_icon::Icon::from_rgba(icon_data.rgba.clone(), icon_data.width, icon_data.height)?;
        let tray_icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Orbit")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .build()?;

        Ok(Self {
            tray_icon,
            show_id: show.id().clone(),
            pause_all_id: pause_all.id().clone(),
            resume_all_id: resume_all.id().clone(),
            quit_id: quit.id().clone(),
            status_item: status,
            last_status: RefCell::new(String::new()),
            last_show: Cell::new(None),
        })
    }

    pub fn set_status(&self, status: &str) {
        if self.last_status.borrow().as_str() == status {
            return;
        }
        self.status_item.set_text(status);
        let _ = self
            .tray_icon
            .set_tooltip(Some(format!("Orbit — {status}")));
        *self.last_status.borrow_mut() = status.to_owned();
    }

    pub fn drain_actions(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let action = if event.id == self.show_id {
                Some(TrayAction::Show)
            } else if event.id == self.pause_all_id {
                Some(TrayAction::PauseAll)
            } else if event.id == self.resume_all_id {
                Some(TrayAction::ResumeAll)
            } else if event.id == self.quit_id {
                Some(TrayAction::Quit)
            } else {
                None
            };

            if let Some(action) = action {
                self.push_action(&mut actions, action);
            }
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if event.id() != self.tray_icon.id() {
                continue;
            }

            let should_show = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );

            if should_show {
                self.push_action(&mut actions, TrayAction::Show);
            }
        }

        actions
    }

    fn push_action(&self, actions: &mut Vec<TrayAction>, action: TrayAction) {
        if action != TrayAction::Show {
            actions.push(action);
            return;
        }

        let now = Instant::now();
        let is_duplicate = self
            .last_show
            .get()
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(450));

        if !is_duplicate {
            self.last_show.set(Some(now));
            actions.push(TrayAction::Show);
        }
    }
}
