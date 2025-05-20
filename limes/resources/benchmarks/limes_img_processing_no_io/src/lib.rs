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
use image::ImageReader;
use std::path::Path;

struct Component;
impl Guest for Component {
    #[allow(unused)]
    fn run(args: String) -> String {
        // Open photo & proc the image
        let image_path = Path::new("./img-1.jpg");
        let image_mod_path = Path::new("./img-mod-1.jpg");
        let img = ImageReader::open(image_path)
            .expect("Can't open the image file")
            .decode()
            .expect("Can't decode the image")
            .blur(3.0)
            .brighten(2)
            .adjust_contrast(1.5)
            .grayscale();
        // img.save(image_mod_path);
        args
    }
}

export!(Component);
