use std::{
    env,
    fs::{self, File},
    io::Write,
    path::Path,
};

use serde_json::{json, to_value};

use crate::{instance::SceneComponent, lights::Light, world::World};

pub enum SaveResult {
    Ok,
    FailedToCheckPathExists,
    PathAlreadyExists,
    FailedToSerializeData,
}

struct LoadedObjects {
    instances: Vec<SceneComponent>,
}

struct LevelFileContent {
    objects: Vec<LoadedObjects>,
    lights: Vec<Light>,
}

pub struct WorldLoader {}

impl WorldLoader {
    pub fn load_level(&self, world: &mut World, level_file_path: &Path) {
        let file_contents = fs::read_to_string(level_file_path);
    }

    pub fn save_level(world: &World, level_file_name: &str) -> anyhow::Result<bool> {
        let lights = world.get_lights();
        let meshes = world.get_meshes();
        let mut target_folder = env::current_dir()?;
        target_folder.push("levels");
        let target_file = target_folder.join(level_file_name);
        if !target_folder.try_exists()? {
            std::fs::create_dir(target_folder)?;
        }

        log::info!("Saving into {:?}", target_file);

        let serialized_lights = to_value(lights)?;
        let serialized_objects = to_value(meshes)?;
        // let does_file_exist = target_file.try_exists()?;

        // if !does_file_exist
        {
            // let mut file = File::create_new(target_file)?;
            let mut file = File::options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(target_file)?;
            let json = json!({"objects": serialized_objects, "lights": serialized_lights});
            file.write(json.to_string().as_bytes())?;
        }
        Ok(true)
    }
}
