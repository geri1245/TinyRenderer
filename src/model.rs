use std::{ops::Range, rc::Rc};

use crate::texture;

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Rc<Material>>,
}

pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub material: Option<Rc<Material>>,
}

pub trait Drawable<'a, 'b> {
    fn draw(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    );

    fn draw_instanced(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        instances: Range<u32>,
    );
}

impl<'a, 'b> Drawable<'a, 'b> for Mesh
where
    'a: 'b,
{
    fn draw_instanced(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        instances: Range<u32>,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if self.material.is_some() {
            render_pass.set_bind_group(0, &self.material.as_ref().unwrap().bind_group, &[]);
        }
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.set_bind_group(2, light_bind_group, &[]);
        render_pass.draw_indexed(0..self.index_count, 0, instances);
    }

    fn draw(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_instanced(render_pass, camera_bind_group, light_bind_group, 0..1);
    }
}

impl<'a, 'b> Drawable<'a, 'b> for Model
where
    'a: 'b,
{
    fn draw_instanced(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
        instances: Range<u32>,
    ) {
        for mesh in &self.meshes {
            mesh.draw_instanced(
                render_pass,
                camera_bind_group,
                light_bind_group,
                instances.clone(),
            );
        }
    }

    fn draw(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_instanced(render_pass, camera_bind_group, light_bind_group, 0..1);
    }
}

pub trait DrawLight<'a> {
    fn draw_light_mesh(
        &mut self,
        mesh: &'a Mesh,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_light_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_light_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawLight<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_light_mesh(
        &mut self,
        mesh: &'b Mesh,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_mesh_instanced(mesh, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_light_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.draw_indexed(0..mesh.index_count, 0, instances);
    }

    fn draw_light_model(
        &mut self,
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_light_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }
    fn draw_light_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            self.draw_light_mesh_instanced(
                mesh,
                instances.clone(),
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}
