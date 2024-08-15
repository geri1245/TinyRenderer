use std::{
    borrow::BorrowMut,
    env,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use serde_json::to_string;

use crate::world::World;

pub enum SaveResult {
    Ok,
    FailedToCheckPathExists,
    PathAlreadyExists,
    FailedToSerializeData,
}

pub struct WorldLoader {}

impl WorldLoader {
    pub fn load_level(&self, world: &mut World) {}

    pub fn save_level(world: &World, level_file_name: &str) -> anyhow::Result<bool> {
        let lights = world.get_lights();
        let meshes = world.get_meshes();
        let mut target_folder = env::current_dir()?;
        target_folder.push("levels");
        let target_file = target_folder.join(level_file_name);
        if !target_folder.try_exists()? {
            std::fs::create_dir(target_folder)?;
        }

        log::warn!("Trying to save into {:?}", target_file);

        let serialized_world = to_string(lights)?;
        let does_file_exist = target_file.try_exists()?;
        if !does_file_exist {
            let mut file = File::create_new(target_file)?;
            file.write(serialized_world.as_bytes())?;
        }
        Ok(true)
    }

    pub fn update(&mut self) {}
}
