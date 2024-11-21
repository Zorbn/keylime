pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near_z: f32, far_z: f32) -> [f32; 16] {
    let mut result = [0.0; 16];

    let inv_width = 1.0 / (right - left);
    let inv_height = 1.0 / (top - bottom);
    let inv_depth = -1.0 / (far_z - near_z);

    result[0] = 2.0 * inv_width;
    // Flip the y-axis.
    result[1 + 4] = -2.0 * inv_height;
    result[2 + 2 * 4] = 2.0 * inv_depth;
    result[3 * 4] = -(right + left) * inv_width;
    // Adjust for flipped y-axis.
    result[1 + 3 * 4] = (top + bottom) * inv_height;
    result[2 + 3 * 4] = (far_z + near_z) * inv_depth;
    result[3 + 3 * 4] = 1.0;

    result
}
