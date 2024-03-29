use std::ptr;
use std::fs::{self, File};
use std::io::BufReader;
use std::ffi::{CStr, CString};
use std::sync::mpsc::Receiver;
use glfw::{Context, WindowHint, WindowEvent, WindowMode};
use glam::{Mat4, Vec3};
use image::imageops;
use obj::{Obj, TexturedVertex};
use log::{debug, trace, error};
use shader_prepper::{ResolvedInclude, ResolvedIncludePath};
use crate::ecs::World;
use crate::{Result, asset};

pub use gl;

pub struct Renderer {
  pub glfw: glfw::Glfw,
  pub window: glfw::Window,
  pub events: Receiver<(f64, WindowEvent)>,
  pub version: &'static str,
  pub renderer: &'static str,
}

impl Renderer {
  pub fn new() -> Result<Self> {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    let (mut window, events) = glfw
      .create_window(1400, 800, "phosphor", WindowMode::Windowed)
      .unwrap();
    window.make_current();
    window.set_all_polling(true);
    gl::load_with(|s| window.get_proc_address(s));
    unsafe {
      gl::Enable(gl::FRAMEBUFFER_SRGB);
      gl::Enable(gl::LINE_SMOOTH);
      gl::Enable(gl::DEPTH_TEST);
      gl::Enable(gl::SCISSOR_TEST);
      let version = CStr::from_ptr(gl::GetString(gl::VERSION) as _).to_str()?;
      let renderer = CStr::from_ptr(gl::GetString(gl::RENDERER) as _).to_str()?;
      debug!("Initialized OpenGL {} renderer on '{}'.", version, renderer);
      Ok(Self {
        glfw,
        window,
        events,
        version,
        renderer,
      })
    }
  }

  pub fn resize(&self, w: u32, h: u32) {
    unsafe {
      gl::Viewport(0, 0, w as _, h as _);
      gl::Scissor(0, 0, w as _, h as _);
    }
  }

  pub fn clear(&self, r: f32, g: f32, b: f32, a: f32) {
    unsafe {
      gl::ClearColor(r, g, b, a);
      gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
    }
  }
}

struct FileIncludeProvider;
impl shader_prepper::IncludeProvider for FileIncludeProvider {
  type IncludeContext = ();

  fn resolve_path(
    &self,
    path: &str,
    _: &Self::IncludeContext,
  ) -> Result<ResolvedInclude<Self::IncludeContext>> {
    Ok(ResolvedInclude {
      resolved_path: ResolvedIncludePath(format!("assets/shaders/{}", path)),
      context: (),
    })
  }

  fn get_include(&mut self, resolved: &ResolvedIncludePath) -> Result<String> {
    Ok(fs::read_to_string(&resolved.0)?)
  }
}

unsafe fn compile_shader(path: &str, ty: u32) -> Result<u32> {
  trace!("Compiling shader '{}'.", path);
  let shader = gl::CreateShader(ty);
  let src = shader_prepper::process_file(path, &mut FileIncludeProvider, ())?
    .into_iter()
    .map(|c| c.source)
    .collect::<Vec<String>>()
    .join("");
  gl::ShaderSource(
    shader,
    1,
    &(src.as_bytes().as_ptr().cast()),
    &(src.len().try_into().unwrap()),
  );
  gl::CompileShader(shader);
  let mut success = 0;
  gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
  if success == 0 {
    let err = CString::from_vec_unchecked(vec![0; 1024]);
    gl::GetShaderInfoLog(shader, 1024, ptr::null_mut(), err.as_ptr() as _);
    error!("Failed to compile '{}':\n{}", path, err.to_str()?);
  }
  Ok(shader)
}

#[derive(Copy, Clone)]
pub struct Shader(pub u32);

impl Shader {
  pub fn new(vert_path: &str, frag_path: &str) -> Result<Self> {
    unsafe {
      let vert = compile_shader(vert_path, gl::VERTEX_SHADER)?;
      let frag = compile_shader(frag_path, gl::FRAGMENT_SHADER)?;
      let program = gl::CreateProgram();
      gl::AttachShader(program, vert);
      gl::AttachShader(program, frag);
      gl::LinkProgram(program);
      gl::DeleteShader(vert);
      gl::DeleteShader(frag);
      Ok(Self(program))
    }
  }

  pub fn bind(&self) {
    unsafe { gl::UseProgram(self.0) }
  }

  fn get_loc(&self, name: &str) -> i32 {
    let c = CString::new(name).unwrap();
    unsafe { gl::GetUniformLocation(self.0, c.as_ptr() as _) }
  }

  pub fn set_mat4(&self, name: &str, val: &Mat4) {
    unsafe {
      gl::ProgramUniformMatrix4fv(
        self.0 as _,
        self.get_loc(name),
        1,
        gl::FALSE,
        val.to_cols_array().as_ptr(),
      )
    }
  }

  pub fn set_vec3(&self, name: &str, val: &Vec3) {
    unsafe { gl::ProgramUniform3fv(self.0 as _, self.get_loc(name), 1, val.to_array().as_ptr()) }
  }

  pub fn set_i32(&self, name: &str, val: &i32) {
    unsafe {
      gl::ProgramUniform1i(self.0 as _, self.get_loc(name), *val);
    }
  }

  pub fn set_f32(&self, name: &str, val: &f32) {
    unsafe {
      gl::ProgramUniform1f(self.0 as _, self.get_loc(name), *val);
    }
  }
}

#[repr(C)]
#[derive(Clone)]
pub struct Vertex {
  pub pos: [f32; 3],
  pub uv: [f32; 2],
  pub normal: [f32; 3],
}

#[asset(load_mesh)]
#[derive(Clone)]
pub struct Mesh {
  pub vert_arr: u32,
  pub vert_buf: u32,
  pub idx_buf: u32,
  pub vertices: Vec<Vertex>,
  pub indices: Vec<u32>,
}

fn load_mesh(_: &mut World, path: &str) -> Result<Mesh> {
  let obj: Obj<TexturedVertex, u32> = obj::load_obj(BufReader::new(File::open(path)?))?;
  Ok(Mesh::new(
    &obj
      .vertices
      .iter()
      .map(|v| Vertex {
        pos: v.position,
        uv: [v.texture[0], v.texture[1]],
        normal: v.normal,
      })
      .collect::<Vec<_>>(),
    &obj.indices,
  ))
}

impl Mesh {
  pub fn new(vertices: &[Vertex], indices: &[u32]) -> Self {
    unsafe {
      let mut vert_arr = 0;
      gl::GenVertexArrays(1, &mut vert_arr);
      gl::BindVertexArray(vert_arr);
      let mut vert_buf = 0;
      gl::GenBuffers(1, &mut vert_buf);
      gl::BindBuffer(gl::ARRAY_BUFFER, vert_buf);
      gl::BufferData(
        gl::ARRAY_BUFFER,
        (vertices.len() * 32) as _,
        vertices.as_ptr() as _,
        gl::STATIC_DRAW,
      );
      let mut idx_buf = 0;
      gl::GenBuffers(1, &mut idx_buf);
      gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, idx_buf);
      gl::BufferData(
        gl::ELEMENT_ARRAY_BUFFER,
        (indices.len() * 4) as _,
        indices.as_ptr() as _,
        gl::STATIC_DRAW,
      );
      gl::EnableVertexAttribArray(0);
      gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 32, 0 as _);
      gl::EnableVertexAttribArray(1);
      gl::VertexAttribPointer(1, 2, gl::FLOAT, gl::FALSE, 32, 12 as _);
      gl::EnableVertexAttribArray(2);
      gl::VertexAttribPointer(2, 3, gl::FLOAT, gl::FALSE, 32, 20 as _);
      Self {
        vert_arr,
        vert_buf,
        idx_buf,
        vertices: vertices.to_vec(),
        indices: indices.to_vec(),
      }
    }
  }

  pub fn draw(&self) {
    unsafe {
      gl::BindVertexArray(self.vert_arr);
      gl::DrawElements(
        gl::TRIANGLES,
        self.indices.len() as _,
        gl::UNSIGNED_INT,
        std::ptr::null(),
      );
    }
  }
}

#[derive(Copy, Clone)]
#[asset(load_tex)]
pub struct Texture {
  pub id: u32,
  pub width: u32,
  pub height: u32,
  pub iformat: u32,
  pub format: u32,
  pub typ: u32,
}

fn load_tex(_: &mut World, path: &str) -> Result<Texture> {
  let mut img = image::open(path)?.to_rgba8();
  imageops::flip_vertical_in_place(&mut img);
  Ok(Texture::new(
    img.as_ptr(),
    img.width(),
    img.height(),
    gl::SRGB_ALPHA,
    gl::RGBA,
    gl::UNSIGNED_BYTE,
  ))
}

impl Texture {
  pub fn new(
    data: *const u8,
    width: u32,
    height: u32,
    iformat: u32,
    format: u32,
    typ: u32,
  ) -> Self {
    unsafe {
      let mut tex = 0;
      gl::GenTextures(1, &mut tex);
      gl::BindTexture(gl::TEXTURE_2D, tex);
      gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
      gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
      gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
      gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);
      gl::TexImage2D(
        gl::TEXTURE_2D,
        0,
        iformat as _,
        width as _,
        height as _,
        0,
        format,
        typ,
        data as _,
      );

      Self {
        id: tex,
        width,
        height,
        iformat,
        format,
        typ,
      }
    }
  }

  pub fn empty() -> Self {
    Self::new(
      ptr::null(),
      0,
      0,
      gl::SRGB_ALPHA,
      gl::RGBA,
      gl::UNSIGNED_BYTE,
    )
  }

  pub fn bind(&self, unit: u32) {
    unsafe {
      gl::ActiveTexture(gl::TEXTURE0 + unit);
      gl::BindTexture(gl::TEXTURE_2D, self.id);
    }
  }

  pub fn resize(&mut self, width: u32, height: u32) {
    unsafe {
      self.bind(0);
      gl::TexImage2D(
        gl::TEXTURE_2D,
        0,
        self.iformat as _,
        width as _,
        height as _,
        0,
        self.format,
        self.typ,
        ptr::null(),
      );
      self.width = width;
      self.height = height;
    }
  }
}

#[derive(Copy, Clone)]
pub struct Framebuffer {
  pub fb: u32,
  pub rb: u32,
}

impl Framebuffer {
  pub const DEFAULT: Framebuffer = Self { fb: 0, rb: 0 };

  pub fn new() -> Self {
    unsafe {
      let mut s = Self::new_no_depth();
      gl::GenRenderbuffers(1, &mut s.rb);
      gl::BindRenderbuffer(gl::RENDERBUFFER, s.rb);
      gl::RenderbufferStorage(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, 0, 0);
      gl::FramebufferRenderbuffer(
        gl::FRAMEBUFFER,
        gl::DEPTH_STENCIL_ATTACHMENT,
        gl::RENDERBUFFER,
        s.rb,
      );
      s
    }
  }

  pub fn new_no_depth() -> Self {
    unsafe {
      let mut fb = 0;
      gl::GenFramebuffers(1, &mut fb);
      gl::BindFramebuffer(gl::FRAMEBUFFER, fb);
      Self { fb, rb: 0 }
    }
  }

  pub fn bind(&self) {
    unsafe {
      gl::BindFramebuffer(gl::FRAMEBUFFER, self.fb);
    }
  }

  pub fn bind_tex(&self, tex: &Texture, unit: u32) {
    unsafe {
      self.bind();
      gl::FramebufferTexture2D(
        gl::FRAMEBUFFER,
        gl::COLOR_ATTACHMENT0 + unit,
        gl::TEXTURE_2D,
        tex.id,
        0,
      );
    }
  }

  pub fn bind_depth(&self, tex: &Texture) {
    unsafe {
      self.bind();
      gl::FramebufferTexture2D(
        gl::FRAMEBUFFER,
        gl::DEPTH_ATTACHMENT,
        gl::TEXTURE_2D,
        tex.id,
        0,
      );
    }
  }

  pub fn resize(&self, width: u32, height: u32) {
    unsafe {
      gl::BindRenderbuffer(gl::RENDERBUFFER, self.rb);
      gl::RenderbufferStorage(
        gl::RENDERBUFFER,
        gl::DEPTH24_STENCIL8,
        width as _,
        height as _,
      );
    }
  }
}

pub struct Query(u32);

impl Query {
  pub fn new() -> Self {
    unsafe {
      let mut id = 0;
      gl::GenQueries(1, &mut id);
      Self(id)
    }
  }

  pub fn time<F: FnMut()>(&self, mut f: F) {
    unsafe {
      gl::BeginQuery(gl::TIME_ELAPSED, self.0);
      f();
      gl::EndQuery(gl::TIME_ELAPSED);
    }
  }

  pub fn get_blocking(&mut self) -> u64 {
    unsafe {
      let mut v = 0;
      gl::GetQueryObjectui64v(self.0, gl::QUERY_RESULT, &mut v);
      v
    }
  }

  pub fn get(&mut self) -> Option<u64> {
    unsafe {
      let mut avail = 0;
      gl::GetQueryObjectiv(self.0, gl::QUERY_RESULT_AVAILABLE, &mut avail);
      if avail > 0 {
        Some(self.get_blocking())
      } else {
        None
      }
    }
  }
}
