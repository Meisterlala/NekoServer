use image::{ImageBuffer, Rgba};
use log::info;
use std::io::Write;

use crate::const_image;

#[derive(Debug, Clone)]
/// This struct holds the image in memory
pub struct CountImage {
    body: String,
}

impl CountImage {
    /// Returns a Clone of the image
    pub fn get_image(&self) -> warp::hyper::Body {
        self.body.clone().into()
    }

    /// Returns a new CountImage
    pub fn total_new() -> Self {
        let inital_data = match image::open("total.png") {
            Ok(data) => data.to_rgba8(),
            Err(_) => image::RgbaImage::new(128, 128),
        };
        CountImage {
            body: CountImage::img_to_string(&inital_data),
        }
    }

    /// Returns a new CountImage
    pub fn total_from_count(count: u128) -> Self {
        let data = CountImage::create_total_image(count);
        let body = CountImage::img_to_string(&data);
        CountImage { body }
    }

    /// Returns a new CountImage
    pub fn from_count(count: u128) -> Self {
        let data = CountImage::create_count_image(count);
        let body = CountImage::img_to_string(&data);
        CountImage { body }
    }

    /// Render the total image
    fn create_total_image(count: u128) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        // Generate number
        let number = CountImage::create_count_image(count);

        // Overlay the image
        let mut base = const_image::HEADER.clone();
        let pos = (560, ((base.height() - number.height()) / 2) as i64);
        image::imageops::overlay(&mut base, &number, pos.0, pos.1);

        base
    }

    fn create_count_image(count: u128) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        // Convert count to char[]
        let count_str = count.to_string();
        let count_len = count_str.len();
        let count_arr = count_str.chars();

        // Create a new image with the same dimensions as the template
        let number_width = const_image::NUMBERS[0].width();
        let number_height = const_image::NUMBERS[0].height();
        let mut overlay = image::RgbaImage::new(number_width * count_len as u32, number_height);

        // Write the Count onto the image
        for (i, digit) in count_arr.enumerate() {
            let digit = digit.to_digit(10).unwrap() as usize;
            let number = &const_image::NUMBERS[digit];
            image::imageops::overlay(&mut overlay, number, (i as u32 * number_width) as i64, 0);
        }
        overlay
    }

    /// Save to Image to the disk (not really needed but nice for debugging)
    /*
        pub fn save_image(&self) {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open("image.png")
                .expect("Cant open image to write to it");
            let mut bufferd = std::io::BufWriter::new(file);
            self.data
                .write_to(&mut bufferd, image::ImageOutputFormat::Png)
                .expect("Cant write to image");
            bufferd.flush().expect("Cant flush image");
        }
    */

    /// Convert the image to a String
    fn img_to_string(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> String {
        // Setup Bufferwriter with a Cursor so we have Seek
        let mut buffer = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));
        // Write Image to Buffer
        img.write_to(&mut buffer, image::ImageOutputFormat::Png)
            .unwrap();
        buffer.flush().unwrap();
        // Extract the written data
        let data = buffer.into_inner().unwrap().into_inner();
        // SAFETY: We dont care, that this is not Valid UTF-8. Its binary data
        unsafe { String::from_utf8_unchecked(data) }
    }
}

impl Default for CountImage {
    fn default() -> Self {
        Self::total_new()
    }
}

impl From<CountImage> for warp::hyper::Body {
    fn from(val: CountImage) -> Self {
        val.body.into()
    }
}
