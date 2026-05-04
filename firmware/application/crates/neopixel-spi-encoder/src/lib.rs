#![cfg_attr(not(test), no_std)]

fn bit(val: u8, pos: u8) -> u8 {
    (val >> pos) & 1
}

pub const fn buffer_size(num_leds: usize) -> usize {
    12 * num_leds + 2
}

pub fn fill_with_color(data: &mut [u8], color: (u8, u8, u8)) -> &[u8] {
    let num_leds = data.len().checked_sub(2).and_then(|v| v.checked_div(12));

    let Some(num_leds) = num_leds else {
        return &[];
    };

    let data = &mut data[..buffer_size(num_leds)];

    data.fill(0);

    for led_id in 0..num_leds {
        for (color_pos, &color_val) in [color.1, color.0, color.2].iter().enumerate() {
            let data_start = led_id * 12 + color_pos * 4 + 1;
            data[data_start] = 0x88 + 0x60 * bit(color_val, 7) + 0x06 * bit(color_val, 6);
            data[data_start + 1] = 0x88 + 0x60 * bit(color_val, 5) + 0x06 * bit(color_val, 4);
            data[data_start + 2] = 0x88 + 0x60 * bit(color_val, 3) + 0x06 * bit(color_val, 2);
            data[data_start + 3] = 0x88 + 0x60 * bit(color_val, 1) + 0x06 * bit(color_val, 0);
        }
    }

    data
}

#[cfg(test)]
mod tests {

    #[test]
    fn buffer_size() {
        assert_eq!(2, super::buffer_size(0));
        assert_eq!(14, super::buffer_size(1));
        assert_eq!(26, super::buffer_size(2));
        assert_eq!(38, super::buffer_size(3));
    }

    #[test]
    fn fill_with_color() {
        assert_eq!(
            &[
                0x00, // Blank, sometimes the first byte has weird timing
                0xee, 0xe8, 0xe8, 0xe8, // G
                0x8e, 0xee, 0xe8, 0xee, // R
                0x8e, 0x88, 0xee, 0x8e, // B
                0xee, 0xe8, 0xe8, 0xe8, // G
                0x8e, 0xee, 0xe8, 0xee, // R
                0x8e, 0x88, 0xee, 0x8e, // B
                0x00, // Blank, to make sure the state after the transmissin is 'low'
            ],
            super::fill_with_color(&mut [0xff; 26], (123, 234, 77)),
        );
    }
}
