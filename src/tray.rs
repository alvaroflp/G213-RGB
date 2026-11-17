//! Linux StatusNotifierItem integration. The tray thread only reports requests;
//! the eframe event loop owns the window and application state.

use egui::{Context, ViewportCommand};
use ksni::{MenuItem, blocking::TrayMethods, menu::StandardItem};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Copy)]
enum Action {
    Open,
    Quit,
}

struct Indicator {
    ctx: Context,
    actions: mpsc::Sender<Action>,
    online: Arc<AtomicBool>,
    icon: ksni::Icon,
}

impl Indicator {
    fn send(&self, action: Action) {
        if self.actions.send(action).is_ok() {
            self.ctx.request_repaint();
        }
    }

    fn set_online(&self, online: bool) {
        self.online.store(online, Ordering::Relaxed);
        self.ctx.request_repaint();
    }
}

impl ksni::Tray for Indicator {
    fn id(&self) -> String {
        "g213-rgb".into()
    }

    fn title(&self) -> String {
        "G213 RGB".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(Action::Open);
    }

    // Desktop panels that use AppIndicator menus can open the UI from this
    // entry even when they do not invoke StatusNotifierItem.Activate.
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Abrir G213 RGB".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Action::Open)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Sair".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Action::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn watcher_online(&self) {
        self.set_online(true);
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        self.set_online(false);
        true
    }
}

pub struct Tray {
    handle: ksni::blocking::Handle<Indicator>,
    online: Arc<AtomicBool>,
    actions: mpsc::Receiver<Action>,
    start_hidden: bool,
    hidden: bool,
    quitting: bool,
}

impl Tray {
    pub fn start(ctx: &Context, start_hidden: bool) -> Option<Self> {
        let icon = crate::app_icon()?;
        let (tx, rx) = mpsc::channel();
        let online = Arc::new(AtomicBool::new(true));
        let mut data = icon.rgba;
        // Egui stores RGBA, whereas StatusNotifierItem expects network-order ARGB.
        for pixel in data.as_chunks_mut::<4>().0 {
            pixel.rotate_right(1);
        }
        let indicator = Indicator {
            ctx: ctx.clone(),
            actions: tx,
            online: online.clone(),
            icon: ksni::Icon {
                width: icon.width as i32,
                height: icon.height as i32,
                data,
            },
        };
        match indicator.spawn() {
            Ok(handle) => Some(Self {
                handle,
                online,
                actions: rx,
                start_hidden,
                hidden: false,
                quitting: false,
            }),
            Err(err) => {
                eprintln!("Bandeja indisponível: {err}");
                None
            }
        }
    }

    pub fn available(&self) -> bool {
        self.online.load(Ordering::Relaxed) && !self.handle.is_closed()
    }

    pub fn update(&mut self, ctx: &Context, keep_in_tray: bool, reopen: bool) {
        let mut open = reopen;
        for action in self.actions.try_iter() {
            match action {
                Action::Open => open = true,
                Action::Quit => self.quitting = true,
            }
        }
        if self.quitting {
            if !ctx.input(|input| input.viewport().close_requested()) {
                ctx.send_viewport_cmd(ViewportCommand::Close);
                ctx.request_repaint();
            }
            return;
        }

        let close_requested = ctx.input(|input| input.viewport().close_requested());
        let available = self.available();
        let start_hidden = std::mem::take(&mut self.start_hidden);
        if open || (self.hidden && !available) {
            if close_requested {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            self.hidden = false;
            show_window(ctx, open);
        } else if available && ((close_requested && keep_in_tray) || start_hidden) {
            if close_requested {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            if !self.hidden {
                self.hidden = true;
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
        }
    }
}

pub fn host_available() -> zbus::Result<bool> {
    let connection = zbus::blocking::Connection::session()?;
    let watcher = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )?;
    watcher.get_property("IsStatusNotifierHostRegistered")
}

pub fn show_window(ctx: &Context, focus: bool) {
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    if focus {
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let _shutdown = self.handle.shutdown();
    }
}
