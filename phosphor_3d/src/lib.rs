use std::ops::Range;
use phosphor::Result;
use phosphor::gfx::{Renderer, Shader, Texture, Mesh};
use phosphor::ecs::{World, Stage};
use phosphor::math::{Vec3, Quat, EulerRot, Mat4};
use phosphor::log::warn;

pub struct Transform {
  pub position: Vec3,
  pub rotation: Quat,
  pub scale: Vec3,
}

impl Transform {
  pub fn new() -> Self {
    Self {
      position: Vec3::ZERO,
      rotation: Quat::IDENTITY,
      scale: Vec3::ONE,
    }
  }

  pub fn pos(mut self, position: Vec3) -> Self {
    self.position = position;
    self
  }

  pub fn rot_quat(mut self, rotation: Quat) -> Self {
    self.rotation = rotation;
    self
  }

  pub fn rot_euler(mut self, rotation: Vec3) -> Self {
    self.rotation = Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z);
    self
  }

  pub fn scale(mut self, scale: Vec3) -> Self {
    self.scale = scale;
    self
  }

  pub fn as_mat4(&self) -> Mat4 {
    Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
  }
}

pub struct Camera {
  pub fov: f32,
  pub clip: Range<f32>,
}

impl Camera {
  pub fn new(fov: f32, clip: Range<f32>) -> Self {
    Self { fov, clip }
  }
}

pub enum Material {
  Textured(Texture),
  Color(Vec3),
}

struct SceneRenderer {
  texture_shader: Shader,
  color_shader: Shader,
}

pub fn scenerenderer(world: &mut World) -> Result<()> {
  let renderer = world.get_resource::<Renderer>().unwrap();
  world.add_resource(SceneRenderer {
    texture_shader: Shader::new(renderer, "res/base.vert", "res/texture.frag")?,
    color_shader: Shader::new(renderer, "res/base.vert", "res/color.frag")?,
  });
  world.add_system(Stage::Draw, &scenerenderer_draw);
  Ok(())
}

fn scenerenderer_draw(world: &mut World) -> Result<()> {
  match world.query::<Camera>().get(0) {
    Some((e, cam)) => match e.get::<Transform>() {
      Some(cam_t) => {
        let renderer = world.get_resource::<Renderer>().unwrap();
        let size = renderer.context.window().inner_size();
        let r = world.get_resource::<SceneRenderer>().unwrap();

        let view = Mat4::look_to_rh(cam_t.position, cam_t.rotation.to_scaled_axis(), Vec3::Y);
        let projection = Mat4::perspective_rh(
          cam.fov,
          size.width as f32 / size.height as f32,
          cam.clip.start,
          cam.clip.end,
        );

        for (e, mesh) in world.query::<Mesh>() {
          match e.get::<Transform>() {
            Some(mesh_t) => {
              let shader = match e
                .get::<Material>()
                .unwrap_or(&mut Material::Color(Vec3::splat(0.75)))
              {
                Material::Textured(tex) => {
                  r.texture_shader.bind(renderer);
                  tex.bind(renderer);
                  &r.texture_shader
                }
                Material::Color(col) => {
                  r.color_shader.bind(renderer);
                  r.color_shader.set_vec3(renderer, 3, col);
                  &r.color_shader
                }
              };

              shader.set_mat4(renderer, 0, &mesh_t.as_mat4());
              shader.set_mat4(renderer, 1, &view);
              shader.set_mat4(renderer, 2, &projection);
              mesh.draw(renderer);
            }
            None => warn!(
              "Mesh on entity {} won't be rendered (Missing Transform).",
              e.id
            ),
          }
        }
      }
      None => warn!("Scene will not be rendered (Missing camera transform)."),
    },
    None => warn!("Scene will not be rendered (Missing camera)."),
  };
  Ok(())
}