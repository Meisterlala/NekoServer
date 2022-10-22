use chrono::prelude::*;
use image::{ImageBuffer, Rgba};
use log::{debug, error};

use crate::const_image;

struct SeasonalImage {
    name: &'static str,
    condition: fn(&Date<Utc>) -> bool,
    image: fn() -> ImageBuffer<Rgba<u8>, Vec<u8>>,
}

static TOTAL_IMAGES: [SeasonalImage; 2] = [
    SeasonalImage {
        name: "Halloween",
        condition: is_halloween,
        image: || const_image::HEADER_HALLOWEEN.clone(),
    },
    SeasonalImage {
        name: "Default",
        condition: |_| true,
        image: || const_image::HEADER.clone(),
    },
];

pub fn seasonal_count_total() -> image::RgbaImage {
    let date = Utc::today();
    for img in TOTAL_IMAGES.iter() {
        if (img.condition)(&date) {
            debug!("Using {} image", img.name);
            return (img.image)();
        }
    }
    error!("No seasonal image found");
    const_image::HEADER.clone()
}

pub fn seasonal_name() -> &'static str {
    let date = Utc::today();
    for img in TOTAL_IMAGES.iter() {
        if (img.condition)(&date) {
            return img.name;
        }
    }
    "Default"
}

fn is_halloween(date: &Date<Utc>) -> bool {
    (date.month() == 10 && date.day() >= 19) || (date.month() == 11 && date.day() <= 3)
}
