use std::{
    env,
    fs::{self, File},
    io::Write,
    path::Path,
};

use serde_json::json;

use crate::{
    camera::Camera,
    world::World,
    world_object::{OmnipresentObject, WorldObject},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LevelFileContent {
    world_objects: Vec<WorldObject>,
    omnipresent_objects: Vec<OmnipresentObject>,
    camera: Camera,
}

pub fn load_level(world: &mut World, level_file_path: &Path) -> anyhow::Result<()> {
    let file_contents = fs::read_to_string(level_file_path)?;
    let mut level_contents = serde_json::from_str::<LevelFileContent>(&file_contents)?;
    for object in level_contents.world_objects.drain(..) {
        world.add_world_object(object);
    }

    for omnipresent_object in level_contents.omnipresent_objects.drain(..) {
        world.add_omnipresent_object(omnipresent_object);
    }

    world.set_camera(&level_contents.camera);

    Ok(())
}

pub fn save_level(world: &World, level_file_name: &str) -> anyhow::Result<()> {
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

    let omnipresent_objects = world.get_omnipresent_objects();
    let meshes = world.get_world_objects();

    let mut meshes_to_save = vec![];
    for mut world_object in meshes.into_iter().cloned().into_iter() {
        let non_transient_components = world_object
            .components
            .into_iter()
            .filter(|component| !component.is_transient())
            .collect::<Vec<_>>();

        if !non_transient_components.is_empty() {
            world_object.components = non_transient_components;
            meshes_to_save.push(world_object.clone());
        }
    }

    let json = json!({"world_objects": meshes_to_save, "omnipresent_objects": omnipresent_objects, "camera": world.camera_controller.camera});
    let contents = serde_json::to_string_pretty(&json)?;
    file.write(contents.as_bytes())?;

    Ok(())
}
