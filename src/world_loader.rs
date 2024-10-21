use std::{
    env,
    fs::{self, File},
    io::Write,
    path::Path,
};

use serde_json::json;

use crate::{camera::Camera, lights::Light, model::WorldObject, world::World};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LevelFileContent {
    objects: Vec<WorldObject>,
    lights: Vec<Light>,
    camera: Camera,
}

pub struct WorldLoader {}

impl WorldLoader {
    pub fn load_level(world: &mut World, level_file_path: &Path) -> anyhow::Result<()> {
        let file_contents = fs::read_to_string(level_file_path)?;
        let mut level_contents = serde_json::from_str::<LevelFileContent>(&file_contents)?;
        for object in level_contents.objects.drain(..) {
            world.add_object(object);
        }
        for light in level_contents.lights.drain(..) {
            world.add_light(light);
        }
        world.set_camera(&level_contents.camera);

        Ok(())
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

        let mut file = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(target_file)?;

        let json =
            json!({"objects": meshes, "lights": lights, "camera": world.camera_controller.camera});
        let contents = json.to_string();
        file.write(contents.as_bytes())?;

        Ok(true)
    }
}
