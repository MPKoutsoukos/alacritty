//! Background image drawn behind the terminal grid (`window.background_image`).

use std::fs::File;
use std::io::BufReader;
use std::mem::{size_of, size_of_val};
use std::path::{Path, PathBuf};
use std::ptr;

use log::{error, info};

use crate::config::window::BackgroundImage;
use crate::display::SizeInfo;
use crate::gl;
use crate::gl::types::*;
use crate::renderer::shader::{ShaderError, ShaderProgram, ShaderVersion};

static IMAGE_SHADER_V: &str = include_str!("../../res/image.v.glsl");
static IMAGE_SHADER_F: &str = include_str!("../../res/image.f.glsl");

/// An uploaded image.
#[derive(Debug)]
struct Texture {
    id: GLuint,
    width: u32,
    height: u32,
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.id) };
    }
}

#[derive(Debug)]
pub struct ImageRenderer {
    program: ShaderProgram,
    vao: GLuint,
    vbo: GLuint,
    u_opacity: GLint,
    u_image: GLint,
    texture: Option<Texture>,

    /// The file the current texture (or the failed attempt to load it) came from, so it is only
    /// read again when the configured path changes.
    source: Option<PathBuf>,
}

impl ImageRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, ShaderError> {
        let program = ShaderProgram::new(shader_version, None, IMAGE_SHADER_V, IMAGE_SHADER_F)?;
        let u_opacity = program.get_uniform_location(c"opacity")?;
        let u_image = program.get_uniform_location(c"image")?;

        // Look the attributes up instead of assuming 0 and 1: GLES2 shaders can't pin them.
        let a_pos = unsafe { gl::GetAttribLocation(program.id(), c"aPos".as_ptr()) } as GLuint;
        let a_tex = unsafe { gl::GetAttribLocation(program.id(), c"aTexCoords".as_ptr()) } as GLuint;

        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (16 * size_of::<f32>()) as isize,
                ptr::null(),
                gl::DYNAMIC_DRAW,
            );

            let stride = (4 * size_of::<f32>()) as GLsizei;
            gl::VertexAttribPointer(a_pos, 2, gl::FLOAT, gl::FALSE, stride, ptr::null());
            gl::EnableVertexAttribArray(a_pos);
            gl::VertexAttribPointer(
                a_tex,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * size_of::<f32>()) as *const _,
            );
            gl::EnableVertexAttribArray(a_tex);

            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }

        Ok(Self { program, vao, vbo, u_opacity, u_image, texture: None, source: None })
    }

    /// Draw the configured image over the cleared window, under the grid.
    ///
    /// The caller sets the viewport and blending up and restores them afterwards.
    pub fn draw(&mut self, size_info: &SizeInfo, config: &BackgroundImage) {
        let Some(path) = config.path.as_deref() else {
            self.texture = None;
            self.source = None;
            return;
        };

        if self.source.as_deref() != Some(path) {
            self.source = Some(path.to_owned());
            self.texture = load(path);
        }

        let Some(texture) = &self.texture else { return };

        let (win_w, win_h) = (size_info.width(), size_info.height());
        if win_w <= 0. || win_h <= 0. {
            return;
        }

        // Native size, shrunk to fit if the window is smaller than the image.
        let (mut w, mut h) = (texture.width as f32, texture.height as f32);
        let fit = (win_w / w).min(win_h / h).min(1.);
        w *= fit;
        h *= fit;

        let (fx, fy) = config.position.anchor();
        let x = ((win_w - w) * fx).round();
        let y = ((win_h - h) * fy).round();

        // Pixels (top-left origin) to normalized device coordinates.
        let left = x / win_w * 2. - 1.;
        let right = (x + w) / win_w * 2. - 1.;
        let top = 1. - y / win_h * 2.;
        let bottom = 1. - (y + h) / win_h * 2.;

        #[rustfmt::skip]
        let vertices: [f32; 16] = [
            left,  top,    0., 0.,
            right, top,    1., 0.,
            left,  bottom, 0., 1.,
            right, bottom, 1., 1.,
        ];

        unsafe {
            gl::UseProgram(self.program.id());
            gl::Uniform1f(self.u_opacity, config.opacity.as_f32());
            gl::Uniform1i(self.u_image, 0);

            // The text renderer caches which glyph atlas is bound and only rebinds on change, so
            // whatever is bound now has to be bound again afterwards.
            gl::ActiveTexture(gl::TEXTURE0);
            let mut previous: GLint = 0;
            gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut previous);
            gl::BindTexture(gl::TEXTURE_2D, texture.id);

            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                size_of_val(&vertices) as isize,
                vertices.as_ptr() as *const _,
            );
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);

            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
            gl::BindTexture(gl::TEXTURE_2D, previous as GLuint);
            gl::UseProgram(0);
        }
    }
}

impl Drop for ImageRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

/// Read a PNG into a premultiplied RGBA texture.
fn load(path: &Path) -> Option<Texture> {
    let (width, height, mut rgba) = match decode(path) {
        Ok(image) => image,
        Err(err) => {
            error!("Unable to load background image {path:?}: {err}");
            return None;
        },
    };

    // Premultiply, to match the rest of Alacritty's blending.
    for px in rgba.chunks_exact_mut(4) {
        let a = u16::from(px[3]);
        for c in &mut px[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }

    let mut id: GLuint = 0;
    let mut previous: GLint = 0;
    unsafe {
        gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut previous);
        gl::GenTextures(1, &mut id);
        gl::BindTexture(gl::TEXTURE_2D, id);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as GLint,
            width as GLsizei,
            height as GLsizei,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            rgba.as_ptr() as *const _,
        );
        gl::BindTexture(gl::TEXTURE_2D, previous as GLuint);
    }

    info!("Loaded background image {path:?} ({width}x{height})");
    Some(Texture { id, width, height })
}

/// Decode any 8/16-bit PNG to straight RGBA8.
fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let file = File::open(path).map_err(|err| err.to_string())?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|err| err.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).map_err(|err| err.to_string())?;
    buf.truncate(frame.buffer_size());

    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()
        },
        png::ColorType::GrayscaleAlpha => {
            buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        },
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("unexpected indexed PNG after expansion".into()),
    };

    Ok((frame.width, frame.height, rgba))
}
