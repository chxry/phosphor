use phosphor::Result;
use phosphor::ecs::{World, Name};
use phosphor::gfx::{Texture, Mesh, gl};
use phosphor::math::Vec3;
use phosphor_ui::Textures;
use phosphor_ui::imgui::{Ui, Image, TextureId, WindowFlags};
use phosphor_3d::{Camera, Transform, SceneRendererOptions};
use crate::SelectedEntity;

pub struct Panel {
  pub title: &'static str,
  pub flags: WindowFlags,
  pub open: bool,
  pub render: &'static dyn Fn(&mut World, &Ui),
}

pub fn setup_panels(world: &mut World) -> Result<()> {
  let scene = scene_init(world)?;
  let outline = outline_init();
  let inspector = inspector_init();
  world.add_resource(vec![scene, outline, inspector]);
  Ok(())
}

struct SceneState {
  fb: u32,
  tex: TextureId,
}

fn scene_init(world: &mut World) -> Result<Panel> {
  let textures = world.get_resource::<Textures>().unwrap();
  world
    .spawn("cam")
    .insert(
      Transform::new()
        .pos(Vec3::new(0.0, 1.0, -10.0))
        .rot_euler(Vec3::new(0.0, 0.0, 1.5)),
    )
    .insert(Camera::new(0.8, 0.1..100.0));
  world
    .spawn("teapot")
    .insert(Transform::new())
    .insert(Mesh::load("res/teapot.obj")?);
  unsafe {
    let mut fb = 0;
    gl::GenFramebuffers(1, &mut fb);
    gl::BindFramebuffer(gl::FRAMEBUFFER, fb);
    let tex = Texture::new(&[], 0, 0)?;
    gl::FramebufferTexture2D(
      gl::FRAMEBUFFER,
      gl::COLOR_ATTACHMENT0,
      gl::TEXTURE_2D,
      tex.0,
      0,
    );
    let tex = textures.insert(tex);
    world.add_resource(SceneState { fb, tex });
    Ok(Panel {
      title: "Scene",
      flags: WindowFlags::NO_SCROLLBAR | WindowFlags::NO_SCROLL_WITH_MOUSE,
      open: true,
      render: &scene_render,
    })
  }
}

fn scene_render(world: &mut World, ui: &Ui) {
  let s = world.get_resource::<SceneState>().unwrap();
  let size = ui.window_size();
  Image::new(s.tex, size)
    .uv0([0.0, 1.0])
    .uv1([1.0, 0.0])
    .build(&ui);

  let tex = world
    .get_resource::<Textures>()
    .unwrap()
    .get(s.tex)
    .unwrap();
  tex.resize(size[0] as _, size[1] as _);
  world.add_resource(SceneRendererOptions { fb: s.fb, size });
}

fn outline_init() -> Panel {
  Panel {
    title: "Outline",
    flags: WindowFlags::empty(),
    open: true,
    render: &outline_render,
  }
}

fn outline_render(world: &mut World, ui: &Ui) {
  let [w, _] = ui.window_size();
  let selected = world.get_resource::<SelectedEntity>().unwrap();
  for (e, n) in world.query::<Name>() {
    if ui
      .selectable_config(n.0.clone())
      .selected(e.id == selected.0.unwrap_or_default())
      .build()
    {
      *selected = SelectedEntity(Some(e.id));
    }
  }
  ui.button_with_size("Add Entity", [w, 0.0]);
}

fn inspector_init() -> Panel {
  Panel {
    title: "Inspector",
    flags: WindowFlags::empty(),
    open: true,
    render: &inspector_render,
  }
}

fn inspector_render(world: &mut World, ui: &Ui) {
  match world.get_resource::<SelectedEntity>().unwrap().0 {
    Some(e) => {
      let size = ui.content_region_avail();
      let (e, n) = world.get_id::<Name>(e).unwrap();
      let mut buf = n.0.clone();
      ui.set_next_item_width(size[0]);
      if ui
        .input_text("##", &mut buf)
        .enter_returns_true(true)
        .build()
        && !buf.is_empty()
      {
        *n = Name(buf);
      }
    }
    None => ui.text("no entity selected."),
  }
}
