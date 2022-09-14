lazy_static::lazy_static! {
    pub static ref NUMBERS: [image::RgbaImage; 10] = [
        image::load_from_memory(include_bytes!("../template/numbers/0.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/1.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/2.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/3.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/4.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/5.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/6.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/7.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/8.png"))
            .expect("Could not load number image")
            .to_rgba8(),
        image::load_from_memory(include_bytes!("../template/numbers/9.png"))
            .expect("Could not load number image")
            .to_rgba8(),
    ];
    pub static ref HEADER: image::RgbaImage = {
        image::load_from_memory(include_bytes!("../template/count.png"))
            .expect("Could not load header image")
            .to_rgba8()
    };
}
