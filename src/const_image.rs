use std::path::Path;

macro_rules! const_image {
    ($path:literal) => {{
        let p: &'static str = $path;
        let file_name = Path::new(p).file_name().unwrap().to_str().unwrap();
        image::load_from_memory(include_bytes!($path))
            .expect(&format!("Could not load image from: {}", &file_name))
            .to_rgba8()
    }};
}

lazy_static::lazy_static! {
    pub static ref NUMBERS: [image::RgbaImage; 10] = [
        const_image!("../template/numbers/0.png"),
        const_image!("../template/numbers/1.png"),
        const_image!("../template/numbers/2.png"),
        const_image!("../template/numbers/3.png"),
        const_image!("../template/numbers/4.png"),
        const_image!("../template/numbers/5.png"),
        const_image!("../template/numbers/6.png"),
        const_image!("../template/numbers/7.png"),
        const_image!("../template/numbers/8.png"),
        const_image!("../template/numbers/9.png")
    ];

    pub static ref HEADER: image::RgbaImage = const_image!("../template/count.png");

    pub static ref HEADER_HALLOWEEN: image::RgbaImage = const_image!("../template/count_halloween.png");

    pub static ref HEADER_CHRISTMAS_DAYS: [image::RgbaImage; 22] = [
        const_image!("../template/padoru/1.png"),
        const_image!("../template/padoru/2.png"),
        const_image!("../template/padoru/3.png"),
        const_image!("../template/padoru/4.png"),
        const_image!("../template/padoru/5.png"),
        const_image!("../template/padoru/6.png"),
        const_image!("../template/padoru/7.png"),
        const_image!("../template/padoru/8.png"),
        const_image!("../template/padoru/9.png"),
        const_image!("../template/padoru/10.png"),
        const_image!("../template/padoru/11.png"),
        const_image!("../template/padoru/12.png"),
        const_image!("../template/padoru/13.png"),
        const_image!("../template/padoru/14.png"),
        const_image!("../template/padoru/15.png"),
        const_image!("../template/padoru/16.png"),
        const_image!("../template/padoru/17.png"),
        const_image!("../template/padoru/18.png"),
        const_image!("../template/padoru/19.png"),
        const_image!("../template/padoru/20.png"),
        const_image!("../template/padoru/21.png"),
        const_image!("../template/padoru/22.png")
    ];
    pub static ref HEADER_CHRISTMAS: image::RgbaImage = const_image!("../template/padoru/christmas.png");
}
