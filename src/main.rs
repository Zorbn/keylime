mod cursor;
mod doc;
mod editor;
mod gfx;
mod key;
mod keybind;
mod line_pool;
mod matrix;
mod mouse_button;
mod mousebind;
mod position;
mod text;
mod visual_position;
mod window;

use editor::Editor;
use gfx::Color;
use line_pool::LinePool;
use window::Window;

fn main() {
    println!("Hello, world!");

    let mut line_pool = LinePool::new();

    let mut editor = Editor::new(&mut line_pool);

    let mut window = Window::new().unwrap();

    while window.is_running() {
        let _ = window.update();
        // println!("dt: {dt}");

        editor.update(&mut window, &mut line_pool);

        let gfx = window.gfx();

        gfx.begin_frame(Color::new(245, 245, 245, 255));

        editor.draw(gfx);

        gfx.end_frame();
    }
}
