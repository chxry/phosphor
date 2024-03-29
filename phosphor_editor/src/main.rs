#![feature(is_some_and)]
#![allow(clippy::redundant_pattern_matching)]
mod panels;

use std::fs;
use phosphor::{Engine, Result};
use phosphor::ecs::{World, Entity, stage};
use phosphor::scene::Scene;
use phosphor::log::{LevelFilter, error};
use phosphor::glfw::{WindowEvent, Key, Modifiers};
use phosphor_imgui::{imgui_plugin, UiRendererOptions};
use phosphor_imgui::imgui::{Ui, StyleStackToken, Context};
use phosphor_fmod::{FmodOptions, fmod_plugin};
use rfd::FileDialog;
use crate::panels::{Panel, setup_panels};

pub struct SelectedEntity(Option<Entity>);
pub struct SceneName(String);
struct Layout(String);

const VER: &str = concat!(
  "\u{f5d3} ",
  env!("CARGO_PKG_NAME"),
  " ",
  env!("CARGO_PKG_VERSION")
);

fn main() -> Result {
  ezlogger::init(LevelFilter::Debug)?;
  Engine::new()
    .add_resource(UiRendererOptions {
      docking: true,
      fonts: &[
        &[
          ("assets/fonts/roboto.ttf", 16.0, None),
          (
            "assets/fonts/fontawesome.ttf",
            14.0,
            Some(&[0xe005, 0xf8ff, 0]),
          ),
        ],
        &[
          ("assets/fonts/shingo.otf", 48.0, None),
          (
            "assets/fonts/fontawesome.ttf",
            48.0,
            Some(&[0xe005, 0xf8ff, 0]),
          ),
        ],
      ],
    })
    .add_resource(FmodOptions {
      play_on_start: false,
    })
    .add_resource(SelectedEntity(None))
    .add_resource(SceneName("".to_string()))
    .add_resource(Layout("Default.ini".to_string()))
    .add_system(stage::INIT, imgui_plugin)
    .add_system(stage::INIT, fmod_plugin)
    .add_system(stage::INIT, setup_panels)
    .add_system(stage::DRAW, draw_ui)
    .add_system(stage::POST_DRAW, layout_change)
    .add_system(stage::EVENT, shortcut_handler)
    .run()
}

fn layout_change(world: &mut World) -> Result {
  if let Some(layout) = world.take_resource::<Layout>() {
    world
      .get_resource::<Context>()
      .unwrap()
      .load_ini_settings(&fs::read_to_string(
        "phosphor_editor/layouts/".to_string() + &layout.0,
      )?);
  }
  Ok(())
}

fn draw_ui(world: &mut World) -> Result {
  let ui = world.get_resource::<Ui>().unwrap();
  let panels = world.get_resource::<Vec<Panel>>().unwrap();
  let scene_name = world.get_resource::<SceneName>().unwrap().0.clone();
  ui.main_menu_bar(|| {
    ui.menu("File", || {
      if ui.menu_item_config("Save").shortcut(shortcut("S")).build() {
        save(mutate(world));
      }
      if ui.menu_item_config("Open").shortcut(shortcut("O")).build() {
        load(mutate(world));
      }
    });
    ui.menu("Windows", || {
      for panel in panels.iter_mut() {
        ui.menu_item_config(panel.title)
          .build_with_ref(&mut panel.open);
      }
    });
    ui.menu("Layout", || {
      for p in fs::read_dir("phosphor_editor/layouts").unwrap() {
        let name = p.unwrap().file_name().into_string().unwrap();
        if ui.menu_item(name.clone()) {
          world.add_resource(Layout(name));
        }
      }
      // if ui.button("save") {
      //   let mut s = String::new();
      //   world.get_resource::<Context>().unwrap().save_ini_settings(&mut s);
      //   fs::write("layout", s).unwrap();
      // }
    });
    let [w, _] = ui.window_size();
    let [tx, _] = ui.calc_text_size(scene_name.clone());
    ui.same_line_with_pos((w - tx) / 2.0);
    ui.text_disabled(scene_name);
    let [tx, _] = ui.calc_text_size(VER);
    ui.same_line_with_pos(w - tx - 16.0);
    ui.text_disabled(VER);
  });
  for panel in panels {
    if panel.open {
      let tokens: Vec<StyleStackToken> = panel.vars.iter().map(|v| ui.push_style_var(*v)).collect();
      ui.window(panel.title)
        .flags(panel.flags)
        .opened(&mut panel.open)
        .build(|| {
          (panel.render)(mutate(world), ui);
        });
      for token in tokens {
        token.pop();
      }
    }
  }
  Ok(())
}

fn shortcut_handler(world: &mut World) -> Result {
  const M: Modifiers = if cfg!(target_os = "macos") {
    Modifiers::Super
  } else {
    Modifiers::Control
  };
  match world.get_resource::<WindowEvent>().unwrap() {
    WindowEvent::Key(Key::S, _, _, M) => {
      save(world);
    }
    WindowEvent::Key(Key::O, _, _, M) => {
      load(world);
    }
    _ => {}
  }
  Ok(())
}

fn save(world: &mut World) {
  if let Some(p) = FileDialog::new().set_file_name("test.scene").save_file() {
    Scene::save(world, p).unwrap();
  }
}

fn load(world: &mut World) {
  if let Some(p) = FileDialog::new().pick_file() {
    world.add_resource(SceneName(p.display().to_string()));
    world.add_resource(SelectedEntity(None));
    if let Err(e) = Scene::load(world, p.clone()) {
      error!("Couldnt load '{}'. {}", p.display(), e);
    }
  };
}

fn shortcut(s: &str) -> String {
  if cfg!(target_os = "macos") {
    "\u{e14f} "
  } else {
    "Ctrl "
  }
  .to_string()
    + s
}

// not very safe, use refcell or something
pub fn mutate<T>(t: &T) -> &mut T {
  unsafe { &mut *(t as *const T as *mut T) }
}
