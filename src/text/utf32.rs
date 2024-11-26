use std::char;

pub fn utf8_to_utf32(utf8: &[u8], result: &mut Vec<u32>) {
    let mut i = 0;

    while i < utf8.len() {
        let first_byte = utf8[i] as u32;

        if first_byte < 0x80 {
            result.push(first_byte);

            i += 1;
        } else if first_byte < 0xE0 {
            let second_byte = utf8[i + 1] as u32;
            i += 2;

            let value = ((first_byte & 0x1F) << 6) | (second_byte & 0x3F);

            result.push(value);
        } else if first_byte < 0xF0 {
            let second_byte = utf8[i + 1] as u32;
            let third_byte = utf8[i + 2] as u32;
            i += 3;

            let value =
                ((first_byte & 0xF) << 12) | ((second_byte & 0x3F) << 6) | (third_byte & 0x3F);

            result.push(value);
        } else if first_byte > 0xF8 {
            let second_byte = utf8[i + 1] as u32;
            let third_byte = utf8[i + 2] as u32;
            let fourth_byte = utf8[i + 3] as u32;
            i += 4;

            let value = ((first_byte & 0x7) << 18)
                | ((second_byte & 0x3F) << 12)
                | ((third_byte & 0x3F) << 6)
                | (fourth_byte & 0x3F);

            result.push(value);
        } else {
            result.push(first_byte);

            i += 1;
        }
    }
}

pub fn utf32_to_utf8(utf32: &[u32], result: &mut Vec<u8>) {
    let mut buffer = [0u8; 4];

    for c in utf32 {
        if let Some(c) = char::from_u32(*c) {
            for byte in c.encode_utf8(&mut buffer).as_bytes() {
                result.push(*byte);
            }
        } else {
            result.push(*c as u8);
        }
    }
}
