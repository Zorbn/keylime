mod cursor;
mod doc;
mod editor;
mod gfx;
mod key;
mod keybind;
mod line_pool;
mod matrix;
mod position;
mod text;
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
        let dt = window.update();
        // println!("dt: {dt}");

        editor.update(&mut window, &mut line_pool);

        let gfx = window.gfx();

        gfx.begin_frame(Color::new(255, 255, 255, 255));

        editor.draw(gfx);

        //         gfx.begin(None);
        //
        //         gfx.add_text("hello world".chars(), 50.0, 50.0, &Color::new(0, 0, 0, 255));
        //
        //         gfx.add_text(
        //             ['h', 'i', ' ', 't', 'h', 'e', 'r', 'e'],
        //             50.0,
        //             80.0,
        //             &Color::new(0, 0, 0, 255),
        //         );
        //
        //         gfx.add_rect(90.0, 80.0, 32.0, 32.0, &Color::new(255, 0, 0, 255));
        //
        //         gfx.end();

        gfx.end_frame();
    }
}
