wit_bindgen::generate!({
    inline: r"
        package component:run;
        interface run {
            run: func(args: string) -> string;
        }

        world runnable {
            export run;
        }
    "
});

use crate::exports::component::run::run::Guest;
use image;
use num_complex;

struct Component;
impl Guest for Component {
    #[allow(unused)]
    fn run(args: String) -> String {
        // Params
        let max_iterations = 256u16;
        let img_side = 800u32;
        let cxmin = -2f32;
        let cxmax = 1f32;
        let cymin = -1.5f32;
        let cymax = 1.5f32;
        let scalex = (cxmax - cxmin) / img_side as f32;
        let scaley = (cymax - cymin) / img_side as f32;

        // Create img buffer
        let mut imgbuf = image::ImageBuffer::new(img_side, img_side);

        // Calculate MandelBrot Set
        for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
            let cx = cxmin + x as f32 * scalex;
            let cy = cymin + y as f32 * scaley;

            let c = num_complex::Complex::new(cx, cy);
            let mut z = num_complex::Complex::new(0f32, 0f32);

            let mut i = 0;
            for t in 0..max_iterations {
                if z.norm() > 2.0 {
                    break;
                }
                z = z * z + c;
                i = t;
            }

            *pixel = image::Luma([i as u8]);
        }
        args
    }
}

export!(Component);
