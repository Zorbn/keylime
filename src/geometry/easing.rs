pub fn ease_out_quart(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powf(4.0)
}
