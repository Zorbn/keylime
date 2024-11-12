mod gfx;
mod matrix;
mod text;
mod window;

use gfx::Color;
use window::Window;

fn main() {
    println!("Hello, world!");

    let mut window = Window::new().unwrap();

    while window.is_running() {
        let dt = window.update();
        // println!("dt: {dt}");

        let gfx = window.gfx();

        gfx.begin_frame(Color::new(255, 255, 255, 255));

        gfx.begin(None);

        gfx.add_text("hello world".chars(), 50.0, 50.0, &Color::new(0, 0, 0, 255));

        gfx.add_text(
            ['h', 'i', ' ', 't', 'h', 'e', 'r', 'e'],
            50.0,
            80.0,
            &Color::new(0, 0, 0, 255),
        );

        gfx.end();

        gfx.end_frame();
    }
}
