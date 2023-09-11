const DARK: &str = "dark";
const LIGHT: &str = "light";
const DARK_MODE_KEY: &str = "dark-mode";

const BODY_INVERT_KEY: &str = "body-invert";
const INVERT: &str = "invert";
const NO_INVERT: &str = "no-invert";

pub fn init_dark_mode() {
    // fetch dark mode setting with media query, override with local storage
    let local_storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
    let dark_mode = match local_storage.get_item(DARK_MODE_KEY).unwrap().as_deref() {
        Some(DARK) => true,
        Some(LIGHT) => false,
        _ => {
            web_sys::window()
                .unwrap()
                .match_media("(prefers-color-scheme: dark)")
                .unwrap()
                .map(|l| l.matches())
                == Some(true)
        }
    };

    let body_invert = match local_storage.get_item(BODY_INVERT_KEY).unwrap().as_deref() {
        Some(INVERT) => true,
        _ => false,
    };

    let body = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap();
    body.class_list()
        .add_1(if dark_mode { DARK } else { LIGHT })
        .unwrap();
    body.class_list()
        .add_1(if body_invert { INVERT } else { NO_INVERT })
        .unwrap();
}

pub fn toggle_dark_mode() {
    let body = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap();
    let dark_mode = body.class_list().contains(DARK);
    let new_mode = if dark_mode { LIGHT } else { DARK };

    body.class_list().remove_2(DARK, LIGHT).unwrap();
    body.class_list().add_1(new_mode).unwrap();

    let local_storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
    local_storage.set_item(DARK_MODE_KEY, new_mode).unwrap()
}

pub fn toggle_body_invert() {
    let body = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap();
    let body_invert = body.class_list().contains(INVERT);
    let new_mode = if body_invert { NO_INVERT } else { INVERT };

    body.class_list().remove_2(NO_INVERT, INVERT).unwrap();
    body.class_list().add_1(new_mode).unwrap();

    let local_storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
    local_storage.set_item(BODY_INVERT_KEY, new_mode).unwrap()
}
