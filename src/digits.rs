pub fn get_digits(mut x: usize, digits: &mut [char; 20]) -> &[char] {
    let mut digit_count = 0;

    while x > 0 {
        let Some(digit) = char::from_digit((x % 10) as u32, 10) else {
            break;
        };

        let digit_index = digits.len() - 1 - digit_count;

        digits[digit_index] = digit;

        digit_count += 1;
        x /= 10;
    }

    let start_index = digits.len() - digit_count;

    &digits[start_index..]
}
