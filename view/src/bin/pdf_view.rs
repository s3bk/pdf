// pathfinder/examples/canvas_text/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use font_kit::handle::Handle;
use pathfinder_canvas::{CanvasFontContext, CanvasRenderingContext2D, TextAlign};
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use pathfinder_content::color::ColorF;
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_gpu::resources::{FilesystemResourceLoader, ResourceLoader};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::iter;
use std::sync::Arc;
use std::env;

use env_logger;
use pdf::file::File as PdfFile;
use pdf::object::*;
use pdf::error::PdfError;
use view::Cache;

fn main() -> Result<(), PdfError> {
    env_logger::init();
    
    let path = env::args().nth(1).expect("no file given");
    println!("read: {}", path);
    let file = PdfFile::<Vec<u8>>::open(&path)?;
    
    let pages: Vec<_> = file.pages().filter_map(|p| p.ok()).collect();
    let num_pages = pages.len();
    let mut current_page = 0;
    let mut cache = Cache::new();
    // Render the canvas to screen.
    let scene = cache.render_page(&file, &pages[current_page])?;
    let size = scene.view_box().size();
    
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context. Pathfinder requires this.
    let gl_attributes = video.gl_attr();
    gl_attributes.set_context_profile(GLProfile::Core);
    gl_attributes.set_context_version(3, 3);

    let scale = Vector2F::splat(1.0);
    // Open a window.
    let window_size = (size * scale).to_i32();
    let mut window = video.window("Probably Distorted File", window_size.x() as u32, window_size.y() as u32)
                      .opengl()
                      .build()
                      .unwrap();

    // Create the GL context, and make it current.
    let gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).unwrap();

    // Create a Pathfinder renderer.
    let resource_loader = FilesystemResourceLoader::locate();
    let mut renderer = Renderer::new(GLDevice::new(GLVersion::GL3, 0),
                                     &resource_loader,
                                     DestFramebuffer::full_window(window_size),
                                     RendererOptions { background_color: Some(ColorF::white()) });

    let proxy = SceneProxy::from_scene(scene, RayonExecutor);
    proxy.build_and_render(&mut renderer, BuildOptions::default());
    window.gl_swap_window();

    // Wait for a keypress.
    let mut event_pump = sdl_context.event_pump().unwrap();
    loop {
        let mut needs_update = false;
        match event_pump.wait_event() {
            Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break,
            Event::KeyDown { keycode: Some(keycode), .. } => {
                match keycode {
                    Keycode::Left => {
                        current_page = (0).max(current_page - 1);
                        needs_update = true;
                    }
                    Keycode::Right => {
                        current_page = (num_pages - 1).min(current_page + 1);
                        needs_update = true;
                    }
                    _ => {}
                }
            }
            Event::KeyUp { .. } => {}
            Event::Window { win_event: Exposed, .. } => {
                proxy.build_and_render(&mut renderer, BuildOptions::default());
                window.gl_swap_window();
            }
            e => println!("{:?}", e)
        }
        if needs_update {
            println!("showing page {}", current_page);
            let scene = cache.render_page(&file, &pages[current_page])?;
            proxy.replace_scene(scene);
            proxy.build_and_render(&mut renderer, BuildOptions::default());
            window.gl_swap_window();
        }
    }
    
    Ok(())
}
