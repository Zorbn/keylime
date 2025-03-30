use core::str;

pub fn get_digits(mut x: usize, digits: &mut [u8; 20]) -> &str {
    let mut digit_count = 0;

    while x > 0 {
        let Some(digit) = char::from_digit((x % 10) as u32, 10) else {
            break;
        };

        let digit_index = digits.len() - 1 - digit_count;

        digits[digit_index] = digit as u8;

        digit_count += 1;
        x /= 10;
    }

    let start_index = digits.len() - digit_count;

    str::from_utf8(&digits[start_index..]).unwrap()
}
