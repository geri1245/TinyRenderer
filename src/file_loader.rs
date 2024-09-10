use async_std::{fs, path::PathBuf, task::block_on};
use image::RgbaImage;

pub struct ImageLoader {}

impl ImageLoader {
    pub fn try_load_image(path: PathBuf) -> anyhow::Result<RgbaImage> {
        let data = block_on(fs::read(path))?;
        let img = image::load_from_memory(&data)?;
        Ok(img.to_rgba8())
    }
}
