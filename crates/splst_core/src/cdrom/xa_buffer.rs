/// Xa Circular resample buffer.
#[derive(Default)]
pub struct XaBuffer {
    /// Buffer with both left and right sample. The actual size is 25 but 32 allows for faster modulo.
    data: [(i16, i16); Self::SIZE],
    head: u8,
}

impl XaBuffer {
    const SIZE: usize = 32;

    pub fn push(&mut self, val: (i16, i16)) {
        self.head = self.head.wrapping_add(1);
        self.data[self.head as usize] = val;
    }

    pub fn resample(&self, phase: u8) -> (i16, i16) {
        let coeffs = &FIR_FILTER_COEFFS[phase as usize];

        let (left, right): (i32, i32) = coeffs
            .iter()
            .map(|c| *c as i32)
            .enumerate()
            .fold((0, 0), |(l, r), (i, c)| {
                let (ls, rs) = self.data[(self.head as usize + i) % Self::SIZE];
                (l + ls as i32 * c, r + rs as i32 * c)
            });

        let (left, right) = (left >> 15, right >> 15);

        let clamp = |val: i32| -> i16 {
            val.clamp(i16::MIN.into(), i16::MAX.into()) as i16 
        };

        (clamp(left), clamp(right))
    }
}

/// Finite impulse reponse filter coefficients, taken from mednafen.
const FIR_FILTER_COEFFS: [[i16; 25]; 7] = [
    [
        0, -5, 17, -35, 70, -23, -68, 347, -839, 2062, -4681, 15367,
        21472, -5882,  2810, -1352, 635, -235, 26, 43, -35, 16, -8, 2, 0,
    ],
    [
        0, -2, 10, -34, 65, -84, 52, 9, -266, 1024, -2680, 9036, 26516,
        -6016,  3021, -1571, 848, -365, 107, 10, -16, 17, -8, 3, -1,
    ],
    [
        -2, 0, 3, -19, 60, -75, 162, -227, 306, -67, -615, 3229, 29883,
        -4532, 2488, -1471, 882, -424, 166, -27, 5, 6, -8, 3, -1
    ],
    [
        -1, 3, -2, -5, 31, -74, 179, -402, 689, -926, 1272, -1446, 31033,
        -1446,  1272, -926, 689, -402, 179, -74, 31, -5, -2, 3, -1,
    ],
    [
        -1, 3, -8, 6, 5, -27, 166, -424, 882, -1471,  2488, -4532, 29883,
        3229, -615, -67, 306, -227, 162, -75, 60, -19, 3, 0, -2,
    ],
    [
        -1, 3, -8, 17, -16, 10, 107, -365, 848, -1571, 3021, -6016, 26516,
        9036, -2680,  1024, -266, 9, 52, -84, 65, -34, 10, -2, 0,
    ],
    [
        0, 2, -8, 16, -35, 43, 26, -235, 635, -1352, 2810, -5882, 21472,
        15367, -4681,  2062, -839, 347, -68, -23, 70, -35, 17, -5, 0,
    ],
];
