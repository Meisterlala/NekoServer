use chrono::prelude::*;
use image::{ImageBuffer, Rgba};
use log::debug;

use crate::const_image;

pub fn seasonal_count_total() -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let date = Utc::today();
    if is_halloween(&date) {
        debug!("The season is Halloween");
        const_image::HEADER_HALLOWEEN.clone()
    } else {
        debug!("No active season");
        const_image::HEADER.clone()
    }
}

fn is_halloween(date: &Date<Utc>) -> bool {
    (date.month() == 10 && date.day() >= 19) || (date.month() == 11 && date.day() <= 3)
}
