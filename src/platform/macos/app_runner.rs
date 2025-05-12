use objc2::{runtime::ProtocolObject, sel, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::ns_string;

use super::{delegate::AppDelegate, result::Result};

macro_rules! add_menu_item {
    ($title:expr, $action:expr, $mods:expr, $c:expr, $menu:expr, $mtm:expr) => {{
        let menu_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                $mtm.alloc(),
                ns_string!($title),
                $action,
                ns_string!($c),
            )
        };

        if let Some(mods) = $mods {
            menu_item.setKeyEquivalentModifierMask(mods);
        }

        $menu.addItem(&menu_item);

        menu_item
    }};
}

fn add_menu_items(app: &NSApplication, mtm: MainThreadMarker) {
    let menubar = NSMenu::new(mtm);

    let app_menu_item = NSMenuItem::new(mtm);
    menubar.addItem(&app_menu_item);

    app.setMainMenu(Some(&menubar));

    let app_menu = NSMenu::new(mtm);

    add_menu_item!(
        "Quit Keylime",
        Some(sel!(terminate:)),
        None,
        "q",
        app_menu,
        mtm
    );

    app_menu_item.setSubmenu(Some(&app_menu));

    let file_menu_item = add_menu_item!("File", None, None, "", menubar, mtm);
    let file_menu = NSMenu::new(mtm);

    add_menu_item!(
        "New Window",
        Some(sel!(newWindow)),
        Some(NSEventModifierFlags::Command | NSEventModifierFlags::Shift),
        "n",
        file_menu,
        mtm
    );

    add_menu_item!(
        "Close Window",
        Some(sel!(performClose:)),
        Some(NSEventModifierFlags::Command | NSEventModifierFlags::Shift),
        "w",
        file_menu,
        mtm
    );

    file_menu_item.setSubmenu(Some(&file_menu));

    let window_menu_item = add_menu_item!("Window", None, None, "", menubar, mtm);
    let window_menu = NSMenu::new(mtm);

    add_menu_item!(
        "Minimize",
        Some(sel!(performMiniaturize:)),
        None,
        "m",
        window_menu,
        mtm
    );

    add_menu_item!(
        "",
        Some(sel!(toggleFullScreen:)),
        Some(NSEventModifierFlags::Command | NSEventModifierFlags::Control),
        "f",
        window_menu,
        mtm
    );

    window_menu_item.setSubmenu(Some(&window_menu));
}

pub fn run_app() -> Result<()> {
    let mtm = MainThreadMarker::new().unwrap();

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    add_menu_items(&app, mtm);

    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);

    app.setDelegate(Some(object));
    app.run();

    Ok(())
}
