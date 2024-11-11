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

        gfx.begin_frame(Color::new(255, 125, 0, 255));

        gfx.begin(None);

        gfx.add_sprite(
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 160.0, 160.0],
            Color::new(0, 125, 125, 255),
        );

        gfx.end();

        gfx.end_frame();
    }
}
