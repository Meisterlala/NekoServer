use chrono::prelude::*;
use log::{debug, error};

use crate::const_image;

struct SeasonalImage {
    name: &'static str,
    condition: fn(&Date<Utc>) -> bool,
    image: fn(&Date<Utc>) -> image::RgbaImage,
}

static TOTAL_IMAGES: [SeasonalImage; 4] = [
    SeasonalImage {
        name: "Halloween",
        condition: is_halloween,
        image: |_| const_image::HEADER_HALLOWEEN.clone(),
    },
    SeasonalImage {
        name: "Christmas Advent",
        condition: |date| date.month() == 12 && date.day() <= 22,
        image: |date| const_image::HEADER_CHRISTMAS_DAYS[(date.day() - 1) as usize].clone(),
    },
    SeasonalImage {
        name: "Christmas Holliday",
        condition: |date| date.month() == 12 && date.day() > 22 && date.day() < 28,
        image: |_| const_image::HEADER_CHRISTMAS.clone(),
    },
    SeasonalImage {
        name: "Default",
        condition: |_| true,
        image: |_| const_image::HEADER.clone(),
    },
];

pub fn seasonal_count_total() -> image::RgbaImage {
    let date = Utc::today();
    for img in TOTAL_IMAGES.iter() {
        if (img.condition)(&date) {
            debug!("Using {} image", img.name);
            return (img.image)(&date);
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
