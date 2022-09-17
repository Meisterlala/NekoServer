use log::{info, warn};
use std::{collections::HashMap, time::Duration};
use tokio::sync::Mutex;

use crate::CountImage;

pub const UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 5);
pub const MAX_CACHE_SIZE: usize = 13000; // About 350MB

#[derive(Debug)]
pub struct ImageCache {
    // Image in memory
    count_total_image: Mutex<CountImage>,

    // List of count images
    count_images: Mutex<HashMap<u128, CountImage>>,
}

impl ImageCache {
    pub fn new() -> Self {
        ImageCache {
            count_total_image: Mutex::new(CountImage::total_new()),
            count_images: Mutex::new(HashMap::new()),
        }
    }

    pub async fn update_total_image(&self, count: u128) {
        info!("Updating total image, count: {}", count);
        let mut new_img = CountImage::total_from_count(count);
        let mut img = self.count_total_image.lock().await;
        std::mem::swap(&mut *img, &mut new_img);
    }

    pub async fn get_total(&self) -> CountImage {
        (*self.count_total_image.lock().await).clone()
    }

    pub async fn get_count(&self, count: u128) -> CountImage {
        {
            let map = self.count_images.lock().await;
            if let Some(img) = map.get(&count) {
                return img.clone();
            }
        }

        // Release Lock while generating image
        let img = CountImage::from_count(count);
        info!("Generated image for {}", count);

        let mut map = self.count_images.lock().await;
        if map.len() >= MAX_CACHE_SIZE {
            // Cleare the cache
            warn!("Clearing cache");
            map.clear();
        }
        map.insert(count, img.clone());
        img
    }
}
