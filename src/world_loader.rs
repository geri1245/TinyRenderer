use crate::world::World;

pub struct WorldLoader {}

impl WorldLoader {
    pub fn load_level(&self, world: &mut World) {}

    pub fn save_level(&self, world: &World, level_file_name: &str) {
        let lights = world.get_lights();
        let meshes = world.get_meshes();
    }

    pub fn update(&mut self) {}
}
