#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    io::Cursor,
    sync::{LazyLock, mpsc::RecvTimeoutError},
    time::Duration,
};

use busylight::{BusyLight, BusyLightState};
use image::RgbaImage;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
};
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum LedState {
    Connected(BusyLightState),
    Disconnected,
}

#[derive(Debug, Clone)]
enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
    LedState(LedState),
}

fn connection_thread(
    events: std::sync::mpsc::Receiver<BusyLightState>,
    event_sender: EventLoopProxy<UserEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut device = None;

    fn try_connect(device: &mut Option<BusyLight>) -> LedState {
        if device.is_none() {
            *device = BusyLight::new().ok();
        }

        let mut new_state = LedState::Disconnected;

        #[allow(clippy::collapsible_if)]
        if let Some(connected_device) = &device {
            if let Ok(state) = connected_device.read_state() {
                new_state = LedState::Connected(state);
            }
        }

        new_state
    }

    let mut previous_state = LedState::Disconnected;
    event_sender.send_event(UserEvent::LedState(previous_state))?;

    previous_state = try_connect(&mut device);
    event_sender.send_event(UserEvent::LedState(previous_state))?;

    loop {
        let event = events.recv_timeout(Duration::from_millis(500));

        match event {
            Ok(state) => {
                if let Some(connected_device) = &mut device {
                    if connected_device.set_state(state).is_err() {
                        device = None;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(RecvTimeoutError::Disconnected.into());
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        let new_state = try_connect(&mut device);
        if new_state != previous_state {
            previous_state = new_state;
            event_sender.send_event(UserEvent::LedState(new_state))?;
        }
    }
}

fn main() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // set a tray event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::TrayIconEvent(event));
    }));

    // set a menu event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
    }));

    let proxy = event_loop.create_proxy();
    let (busylight_setter, busylight_receiver) = std::sync::mpsc::sync_channel::<BusyLightState>(1);
    std::thread::spawn(move || {
        let _ = connection_thread(busylight_receiver, proxy);
    });

    let tray_menu = Menu::new();

    let connected_label = MenuItem::new("Disconnected", false, None);
    let menu_red = IconMenuItem::new("Do not disturb", false, load_menu_icon(&RED_CIRCLE), None);
    let menu_yellow =
        IconMenuItem::new("Concentrated", false, load_menu_icon(&YELLOW_CIRCLE), None);
    let menu_green = IconMenuItem::new("Casual", false, load_menu_icon(&GREEN_CIRCLE), None);
    let menu_off = IconMenuItem::new("Off", false, load_menu_icon(&BLACK_CIRCLE), None);
    let menu_quit = MenuItem::new("Quit", true, None);

    let _ = tray_menu.append_items(&[
        &connected_label,
        &PredefinedMenuItem::separator(),
        &menu_off,
        &PredefinedMenuItem::separator(),
        &menu_red,
        &menu_yellow,
        &menu_green,
        &PredefinedMenuItem::separator(),
        &menu_quit,
    ]);

    let mut tray_icon = None;

    let mut icon: &RgbaImage = &CROSS_MARK;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                // We create the icon once the event loop is actually running
                // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu.clone()))
                        .with_title("Busylight")
                        .with_tooltip("BusyLight")
                        .with_icon(load_icon(icon))
                        .build()
                        .unwrap(),
                );

                // We have to request a redraw here to have the icon actually show up.
                // Tao only exposes a redraw method on the Window so we use core-foundation directly.
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

                    let rl = CFRunLoopGetMain().unwrap();
                    CFRunLoopWakeUp(&rl);
                }
            }

            Event::UserEvent(UserEvent::TrayIconEvent(_event)) => {
                //println!("{event:?}");
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                //println!("{event:?}");

                if event.id == menu_quit.id() {
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                } else if event.id == menu_red.id() {
                    busylight_setter.send(BusyLightState::Red).unwrap();
                } else if event.id == menu_yellow.id() {
                    busylight_setter.send(BusyLightState::Yellow).unwrap();
                } else if event.id == menu_green.id() {
                    busylight_setter.send(BusyLightState::Green).unwrap();
                } else if event.id == menu_off.id() {
                    busylight_setter.send(BusyLightState::Off).unwrap();
                }
            }

            Event::UserEvent(UserEvent::LedState(state)) => {
                println!("{state:?}");
                icon = match &state {
                    LedState::Connected(BusyLightState::Off) => &BLACK_CIRCLE,
                    LedState::Connected(BusyLightState::Green) => &GREEN_CIRCLE,
                    LedState::Connected(BusyLightState::Yellow) => &YELLOW_CIRCLE,
                    LedState::Connected(BusyLightState::Red) => &RED_CIRCLE,
                    LedState::Disconnected => &CROSS_MARK,
                };
                if let Some(tray_icon) = &mut tray_icon {
                    let _ = tray_icon.set_icon(Some(load_icon(icon)));
                }
                if let LedState::Connected(_) = &state {
                    connected_label.set_text("Connected");
                    menu_red.set_enabled(true);
                    menu_yellow.set_enabled(true);
                    menu_green.set_enabled(true);
                    menu_off.set_enabled(true);
                } else {
                    connected_label.set_text("Disconnected");
                    menu_red.set_enabled(false);
                    menu_yellow.set_enabled(false);
                    menu_green.set_enabled(false);
                    menu_off.set_enabled(false);
                }
            }

            _ => {}
        }
    })
}

const BLACK_CIRCLE_WEBP: &[u8] = include_bytes!("../../assets/black-circle.webp");
const RED_CIRCLE_WEBP: &[u8] = include_bytes!("../../assets/red-circle.webp");
const YELLOW_CIRCLE_WEBP: &[u8] = include_bytes!("../../assets/yellow-circle.webp");
const GREEN_CIRCLE_WEBP: &[u8] = include_bytes!("../../assets/green-circle.webp");
const CROSS_MARK_WEBP: &[u8] = include_bytes!("../../assets/cross-mark.webp");

static BLACK_CIRCLE: LazyLock<image::RgbaImage> = LazyLock::new(|| load_webp(BLACK_CIRCLE_WEBP));
static RED_CIRCLE: LazyLock<image::RgbaImage> = LazyLock::new(|| load_webp(RED_CIRCLE_WEBP));
static YELLOW_CIRCLE: LazyLock<image::RgbaImage> = LazyLock::new(|| load_webp(YELLOW_CIRCLE_WEBP));
static GREEN_CIRCLE: LazyLock<image::RgbaImage> = LazyLock::new(|| load_webp(GREEN_CIRCLE_WEBP));
static CROSS_MARK: LazyLock<image::RgbaImage> = LazyLock::new(|| load_webp(CROSS_MARK_WEBP));

fn load_webp(data: &[u8]) -> image::RgbaImage {
    image::ImageReader::with_format(Cursor::new(data), image::ImageFormat::WebP)
        .decode()
        .expect("Failed to decode icon")
        .into_rgba8()
}

fn load_icon(image: &image::RgbaImage) -> tray_icon::Icon {
    let (width, height) = image.dimensions();
    let rgba = image.clone().into_raw();
    tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to open icon")
}

fn load_menu_icon(image: &image::RgbaImage) -> Option<tray_icon::menu::Icon> {
    let (width, height) = image.dimensions();
    let rgba = image.clone().into_raw();
    tray_icon::menu::Icon::from_rgba(rgba, width, height).ok()
}
