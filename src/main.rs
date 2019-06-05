extern crate sdl2;
extern crate gl;
extern crate nalgebra;
extern crate encoding;
#[macro_use]
extern crate imgui;
extern crate imgui_sdl2;
extern crate imgui_opengl_renderer;

use std::io;
use std::io::prelude::*;
use std::fs::File;
use std::string::FromUtf8Error;
use std::path::Path;
use std::io::Cursor;
use crate::common::BinaryReader;
use crate::rsw::{Rsw, GroundData};
use crate::gnd::{Gnd, MeshVertex};
use crate::gat::Gat;
use std::ffi::{CString, CStr};

use imgui::{ImGuiCond, ImString, ImStr, ColorFormat, ColorPickerMode, ImTexture};
use nalgebra::{Vector3, Matrix4, Point3, Matrix, Matrix1x2, Matrix3, Unit, Rotation3};
use crate::opengl::{Shader, Program, VertexArray, VertexAttribDefinition, GlTexture};
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use crate::rsm::{Rsm, RsmNodeVertex};
use sdl2::keyboard::Keycode;
use crate::act::ActionFile;
use crate::spr::{SpriteFile, RenderableFrame};
use rand::Rng;

// guild_vs4.rsw

mod common;
mod opengl;
mod gat;
mod rsw;
mod gnd;
mod rsm;
mod act;
mod spr;


fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let gl_attr = video_subsystem.gl_attr();

    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(4, 5);
    let window = video_subsystem
        .window("Rustarok", 900, 700)
        .opengl()
        .allow_highdpi()
        .resizable()
        .build()
        .unwrap();

    let gl_context = window.gl_create_context().unwrap();
    let gl = gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);

    unsafe {
        gl::Viewport(0, 0, 900, 700); // set viewport
        gl::ClearColor(0.3, 0.3, 0.5, 1.0);
        gl::Enable(gl::DEPTH_TEST);
        gl::DepthFunc(gl::LEQUAL);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let ground_shader_program = Program::from_shaders(
        &[
            Shader::from_source(
                include_str!("shaders/ground.vert"),
                gl::VERTEX_SHADER,
            ).unwrap(),
            Shader::from_source(
                include_str!("shaders/ground.frag"),
                gl::FRAGMENT_SHADER,
            ).unwrap()
        ]
    ).unwrap();

    let model_shader_program = Program::from_shaders(
        &[
            Shader::from_source(
                include_str!("shaders/model.vert"),
                gl::VERTEX_SHADER,
            ).unwrap(),
            Shader::from_source(
                include_str!("shaders/model.frag"),
                gl::FRAGMENT_SHADER,
            ).unwrap()
        ]
    ).unwrap();

    let sprite_shader_program = Program::from_shaders(
        &[
            Shader::from_source(
                include_str!("shaders/sprite.vert"),
                gl::VERTEX_SHADER,
            ).unwrap(),
            Shader::from_source(
                include_str!("shaders/sprite.frag"),
                gl::FRAGMENT_SHADER,
            ).unwrap()
        ]
    ).unwrap();


    let mut map_render_data = load_map("prontera");

    let mut body_action = ActionFile::load(
        BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\sprite\\ÀÎ°£Á·\\¸Ó¸®Åë\\¿©\\1_¿©.act"))
    );

    let mut body_sprite = SpriteFile::load(
        BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\sprite\\ÀÎ°£Á·\\¸Ó¸®Åë\\¿©\\1_¿©.spr"))
    );
    let sprite_frames: Vec<RenderableFrame> = body_sprite.frames
        .into_iter()
        .map(|frame| RenderableFrame::from(frame))
        .collect();

    let mut imgui = imgui::ImGui::init();
    imgui.set_ini_filename(None);
    let video = sdl_context.video().unwrap();
    let mut imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui);

    let renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| video.gl_get_proc_address(s) as _);

    let mut event_pump = sdl_context.event_pump().unwrap();

    let my_str = ImString::new("shitaka");

    let mut camera_pos = Point3::<f32>::new(0.0, 0.0, 3.0);
    let mut camera_front = Vector3::<f32>::new(0.0, 0.0, -1.0);
    let world_up = Vector3::<f32>::new(0.0, 1.0, 0.0);
    let mut camera_up = world_up;
    let mut camera_right = camera_front.cross(&camera_up).normalize();

    let mut last_mouse_x = 400;
    let mut last_mouse_y = 300;
    let mut mouse_down = false;
    let mut yaw = -90f32;
    let mut pitch = 0f32;

    let mut map_name_filter = ImString::new("prontera");
    let all_map_names = std::fs::read_dir("d:\\Games\\TalonRO\\grf\\data").unwrap().map(|entry| {
        let dir_entry = entry.unwrap();
        if dir_entry.file_name().into_string().unwrap().ends_with("rsw") {
            let mut sstr = dir_entry.file_name().into_string().unwrap();
            let len = sstr.len();
            sstr.truncate(len - 4); // remove extension
            Some(sstr)
        } else { None }
    }).filter_map(|x| x).collect::<Vec<String>>();

    let proj = Matrix4::new_perspective(std::f32::consts::FRAC_PI_4, 900f32 / 700f32, 0.1f32, 1000.0f32);

    let mut use_tile_colors = true;
    let mut use_lightmaps = true;
    let mut use_lighting = true;
    let mut light_wheight = [0f32; 3];

    dbg!(map_render_data.texture_atlas.id());
    dbg!(map_render_data.tile_color_texture.id());
    dbg!(map_render_data.lightmap_texture.id());

    let mut keys: HashSet<Keycode> = HashSet::new();
    let mut camera_speed = 2f32;

    'running: loop {
        ///////////
        use sdl2::event::Event;
        use sdl2::keyboard::Keycode;
        for event in event_pump.poll_iter() {
            imgui_sdl2.handle_event(&mut imgui, &event);
            if imgui_sdl2.ignore_event(&event) { continue; }

            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                }
                Event::MouseButtonDown { .. } => {
                    mouse_down = true;
                }
                Event::MouseButtonUp { .. } => {
                    mouse_down = false;
                }
                Event::MouseMotion {
                    timestamp,
                    window_id,
                    which,
                    mousestate,
                    x,
                    y,
                    xrel,
                    yrel
                } => {
                    if mouse_down {
                        let x_offset = x - last_mouse_x;
                        let y_offset = last_mouse_y - y; // reversed since y-coordinates go from bottom to top
                        yaw += x_offset as f32;
                        pitch += y_offset as f32;
                        if pitch > 89.0 {
                            pitch = 89.0;
                        }
                        if pitch < -89.0 {
                            pitch = -89.0;
                        }
                        camera_front = Vector3::<f32>::new(
                            pitch.to_radians().cos() * yaw.to_radians().cos(),
                            pitch.to_radians().sin(),
                            pitch.to_radians().cos() * yaw.to_radians().sin(),
                        ).normalize();

                        camera_right = camera_front.cross(&world_up).normalize();
                        camera_up = camera_right.cross(&camera_front).normalize();
                    }
                    last_mouse_x = x;
                    last_mouse_y = y;
                }
                Event::KeyDown { keycode, .. } => {
                    if keycode.is_some() {
                        keys.insert(keycode.unwrap());
                    }
                }
                Event::KeyUp { keycode, .. } => {
                    if keycode.is_some() {
                        keys.remove(&keycode.unwrap());
                    }
                }
                _ => {}
            }
        }

        camera_speed = if keys.contains(&Keycode::LShift) { 6.0 } else { 2.0 };
        if keys.contains(&Keycode::W) {
            camera_pos += camera_speed * camera_front;
        } else if keys.contains(&Keycode::S) {
            camera_pos -= camera_speed * camera_front;
        }
        if keys.contains(&Keycode::A) {
            camera_pos -= camera_front.cross(&camera_up).normalize() * camera_speed;
        } else if keys.contains(&Keycode::D) {
            camera_pos += camera_front.cross(&camera_up).normalize() * camera_speed;
        }

        let ui = imgui_sdl2.frame(&window, &mut imgui, &event_pump.mouse_state());

        extern crate sublime_fuzzy;
        let map_name_filter_clone = map_name_filter.clone();
        let filtered_map_names: Vec<&String> = all_map_names.iter()
            .filter(|map_name| {
                let matc = sublime_fuzzy::best_match(map_name_filter_clone.to_str(), map_name);
                matc.is_some()
            }).collect();
        ui.window(im_str!("Maps: {},{},{}", camera_pos.x, camera_pos.y, camera_pos.z))
            .position((0.0, 200.0), ImGuiCond::FirstUseEver)
            .size((300.0, (100.0 + filtered_map_names.len() as f32 * 16.0).min(500.0)), ImGuiCond::Always)
            .build(|| {
                if ui.input_text(im_str!("Map name:"), &mut map_name_filter)
                    .enter_returns_true(true)
                    .build() {
                    if let Some(map_name) = filtered_map_names.get(0) {
                        map_render_data = load_map(map_name);
                    }
                }
                for map_name in filtered_map_names.iter() {
                    if ui.small_button(&ImString::new(map_name.as_str())) {
                        map_render_data = load_map(map_name);
                    }
                }
            });

        ui.window(im_str!("Graphic opsions"))
            .position((0.0, 0.0), ImGuiCond::FirstUseEver)
            .size((300.0, 200.0), ImGuiCond::FirstUseEver)
            .build(|| {
                ui.checkbox(im_str!("Use tile_colors"), &mut use_tile_colors);
                if ui.checkbox(im_str!("Use use_lighting"), &mut use_lighting) {
                    use_lightmaps = use_lighting && use_lightmaps;
                }
                if ui.checkbox(im_str!("Use lightmaps"), &mut use_lightmaps) {
                    use_lighting = use_lighting || use_lightmaps;
                }


                ui.drag_float3(im_str!("light_dir"), &mut map_render_data.rsw.light.direction)
                    .min(-1.0).max(1.0).speed(0.05).build();
                ui.color_edit(im_str!("light_ambient"), &mut map_render_data.rsw.light.ambient)
                    .inputs(false)
                    .format(ColorFormat::Float)
                    .build();
                ui.color_edit(im_str!("light_diffuse"), &mut map_render_data.rsw.light.diffuse)
                    .inputs(false)
                    .format(ColorFormat::Float)
                    .build();
                ui.drag_float(im_str!("light_opacity"), &mut map_render_data.rsw.light.opacity)
                    .min(0.0).max(1.0).speed(0.05).build();

                ui.image(ImTexture::from(map_render_data.texture_atlas.id() as usize), [200.0, 200.0]).build();
                let w = map_render_data.lightmap_texture.width as f32;
                let h = map_render_data.lightmap_texture.height as f32;
                let (posx, posy) = ui.get_cursor_screen_pos();
                ui.image(ImTexture::from(map_render_data.lightmap_texture.id() as usize), [w, h]).build();
                if (ui.is_item_hovered()) {
                    ui.tooltip(|| {
                        let focus_sz = 32.0f32;
                        let (mx, my) = ui.imgui().mouse_pos();
                        let mut focus_x = mx - posx - focus_sz * 0.5f32;
                        if focus_x < 0.0f32 {
                            focus_x = 0.0f32;
                        } else if focus_x > w - focus_sz {
                            focus_x = w - focus_sz
                        }
                        let mut focus_y = my - posy - focus_sz * 0.5f32;
                        if focus_y < 0.0f32 { focus_y = 0.0f32; } else if focus_y > h - focus_sz { focus_y = h - focus_sz; }
                        ui.text(format!("Min: {}, {}", focus_x, focus_y));
                        ui.text(format!("Max: {}, {}", focus_x + focus_sz, focus_y + focus_sz));
                        let uv0: [f32; 2] = [(focus_x) / w, (focus_y) / h];
                        let uv1: [f32; 2] = [(focus_x + focus_sz) / w, (focus_y + focus_sz) / h];
                        ui.image(ImTexture::from(map_render_data.lightmap_texture.id() as usize),
                                 [128.0, 128.0],
                        )
                            .uv0(uv0)
                            .uv1(uv1)
                            .build();
                    });
                }
            });

        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }


        let view = Matrix4::look_at_rh(&camera_pos, &(camera_pos + camera_front), &camera_up);

        let model = Matrix4::<f32>::identity();
        let model_view = view * model;
        let normal_matrix = {
            // toInverseMat3
            let inverted = model_view.try_inverse().unwrap();
            let m3x3 = inverted.fixed_slice::<nalgebra::base::U3, nalgebra::base::U3>(0, 0);
            m3x3.transpose()
        };

        ground_shader_program.gl_use();
        ground_shader_program.set_mat4("projection", &proj);
        ground_shader_program.set_mat4("model_view", &model_view);
        ground_shader_program.set_mat3("normal_matrix", &normal_matrix);

        ground_shader_program.set_vec3("light_dir", &map_render_data.rsw.light.direction);
        ground_shader_program.set_vec3("light_ambient", &map_render_data.rsw.light.ambient);
        ground_shader_program.set_vec3("light_diffuse", &map_render_data.rsw.light.diffuse);
        ground_shader_program.set_f32("light_opacity", map_render_data.rsw.light.opacity);

        ground_shader_program.set_vec3("in_lightWheight", &light_wheight);

        map_render_data.texture_atlas.bind(gl::TEXTURE0);
        ground_shader_program.set_int("gnd_texture_atlas", 0);

        map_render_data.tile_color_texture.bind(gl::TEXTURE1);
        ground_shader_program.set_int("tile_color_texture", 1);

        map_render_data.lightmap_texture.bind(gl::TEXTURE2);
        ground_shader_program.set_int("lightmap_texture", 2);

        ground_shader_program.set_int("use_tile_color", if use_tile_colors { 1 } else { 0 });
        ground_shader_program.set_int("use_lightmap", if use_lightmaps { 1 } else { 0 });
        ground_shader_program.set_int("use_lighting", if use_lighting { 1 } else { 0 });


        unsafe {
            map_render_data.ground_vertex_array_obj.bind();
            gl::DrawArrays(
                gl::TRIANGLES, // mode
                0, // starting index in the enabled arrays
                (map_render_data.gnd.mesh.len()) as i32, // number of indices to be rendered
            );
        }

        model_shader_program.gl_use();
        model_shader_program.set_mat4("projection", &proj);
        model_shader_program.set_mat4("view", &view);
        model_shader_program.set_mat3("normal_matrix", &normal_matrix);
        model_shader_program.set_int("model_texture", 0);

        model_shader_program.set_vec3("light_dir", &map_render_data.rsw.light.direction);
        model_shader_program.set_vec3("light_ambient", &map_render_data.rsw.light.ambient);
        model_shader_program.set_vec3("light_diffuse", &map_render_data.rsw.light.diffuse);
        model_shader_program.set_f32("light_opacity", map_render_data.rsw.light.opacity);

        model_shader_program.set_int("use_lighting", if use_lighting { 1 } else { 0 });

        unsafe {
            for (model_name, matrix) in &map_render_data.model_instances {
                model_shader_program.set_mat4("model", &matrix);
                let model_render_data = &map_render_data.models[&model_name];
                model_shader_program.set_f32("alpha", model_render_data.alpha);
                for node_render_data in &model_render_data.model {
                    for face_render_data in node_render_data {
                        face_render_data.texture.bind(gl::TEXTURE0);
                        face_render_data.vao.bind();
                        gl::DrawArrays(
                            gl::TRIANGLES, // mode
                            0, // starting index in the enabled arrays
                            face_render_data.vertex_count as i32, // number of indices to be rendered
                        );
                    }
                }
            }
        }

        sprite_shader_program.gl_use();
        sprite_shader_program.set_mat4("projection", &proj);
        sprite_shader_program.set_mat4("view", &view);
        sprite_shader_program.set_int("model_texture", 0);
        sprite_shader_program.set_f32("alpha", 1.0);
        sprite_frames[0].texture.bind(gl::TEXTURE0);
        for entity in &map_render_data.entities {
            let mut matrix = Matrix4::<f32>::identity();
            matrix.prepend_translation_mut(&entity.pos);
            sprite_shader_program.set_mat4("model", &matrix);
            map_render_data.sprite_vertex_array.bind();
            unsafe {
                gl::DrawArrays(
                    gl::TRIANGLE_STRIP, // mode
                    0, // starting index in the enabled arrays
                    4, // number of indices to be rendered
                );
            }
        }

        renderer.render(ui);

        window.gl_swap_window();
        std::thread::sleep(Duration::from_millis(30))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ModelName(String);

pub struct MapRenderData {
    pub gnd: Gnd,
    pub rsw: Rsw,
    pub ground_vertex_array_obj: VertexArray,
    pub sprite_vertex_array: VertexArray,
    pub texture_atlas: GlTexture,
    pub tile_color_texture: GlTexture,
    pub lightmap_texture: GlTexture,
    pub models: HashMap<ModelName, ModelRenderData>,
    pub model_instances: Vec<(ModelName, Matrix4<f32>)>,
    pub entities: Vec<EntityRenderData>,
}

pub struct EntityRenderData {
    pub pos: Vector3<f32>,
//    pub texture: GlTexture,
}

pub type DataForRenderingSingleNode = Vec<SameTextureNodeFaces>;

pub struct ModelRenderData {
    pub alpha: f32,
    pub model: Vec<DataForRenderingSingleNode>,
}

pub struct SameTextureNodeFaces {
    pub vao: VertexArray,
    pub vertex_count: usize,
    pub texture: GlTexture,
}

fn load_map(map_name: &str) -> MapRenderData {
    let world = Rsw::load(BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\{}.rsw", map_name)));
    let altitude = Gat::load(BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\{}.gat", map_name)));
    let mut ground = Gnd::load(BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\{}.gnd", map_name)),
                               world.water.level,
                               world.water.wave_height);
    let model_names: HashSet<_> = world.models.iter().map(|m| m.filename.clone()).collect();
    let models = Rsw::load_models(model_names);
    let model_render_datas: HashMap<ModelName, ModelRenderData> = models.iter().map(|(name, rsm)| {
        let textures = Rsm::load_textures(&rsm.texture_names);
        let data_for_rendering_full_model: Vec<DataForRenderingSingleNode> = Rsm::generate_meshes_by_texture_id(
            &rsm.bounding_box,
            rsm.shade_type,
            rsm.nodes.len() == 1,
            &rsm.nodes,
            &textures,
        );
        (name.clone(), ModelRenderData {
            alpha: rsm.alpha,
            model: data_for_rendering_full_model,
        })
    }).collect();

    let model_instances: Vec<(ModelName, Matrix4<f32>)> = world.models.iter().map(|model_instance| {
        let mut instance_matrix = Matrix4::<f32>::identity();
        instance_matrix.prepend_translation_mut(&(model_instance.pos + Vector3::new(ground.width as f32, 0f32, ground.height as f32)));

        // rot_z
        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::z()), model_instance.rot.z.to_radians()).to_homogeneous();
        instance_matrix = instance_matrix * rotation;
        // rot x
        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::x()), model_instance.rot.x.to_radians()).to_homogeneous();
        instance_matrix = instance_matrix * rotation;
        // rot y
        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::y()), model_instance.rot.y.to_radians()).to_homogeneous();
        instance_matrix = instance_matrix * rotation;

        instance_matrix.prepend_nonuniform_scaling_mut(&model_instance.scale);

        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::x()), 180f32.to_radians()).to_homogeneous();
        instance_matrix = rotation * instance_matrix;

        (model_instance.filename.clone(), instance_matrix)
    }).collect();

    let texture_atlas = Gnd::create_gl_texture_atlas(&ground.texture_names);
    let tile_color_texture = Gnd::create_tile_color_texture(
        &mut ground.tiles_color_image,
        ground.width, ground.height,
    );
    let lightmap_texture = Gnd::create_lightmap_texture(&ground.lightmap_image, ground.lightmaps.count);
    dbg!(ground.mesh.len());

    let s: Vec<[f32; 4]> = vec![
        [-0.5, 0.5, 0.0, 0.0],
        [0.5, 0.5, 1.0, 0.0],
        [-0.5, -0.5, 0.0, 1.0],
        [0.5, -0.5, 1.0, 1.0]
    ];
    let sprite_vertex_array = VertexArray::new(&s, &[
        VertexAttribDefinition {
            number_of_components: 2,
            offset_of_first_element: 0,
        }, VertexAttribDefinition { // uv
            number_of_components: 2,
            offset_of_first_element: 2,
        }
    ]);

    let vertex_array = VertexArray::new(&ground.mesh, &[
        VertexAttribDefinition {
            number_of_components: 3,
            offset_of_first_element: 0,
        }, VertexAttribDefinition { // normals
            number_of_components: 3,
            offset_of_first_element: 3,
        }, VertexAttribDefinition { // texcoords
            number_of_components: 2,
            offset_of_first_element: 6,
        }, VertexAttribDefinition { // lightmap_coord
            number_of_components: 2,
            offset_of_first_element: 8,
        }, VertexAttribDefinition { // tile color coordinate
            number_of_components: 2,
            offset_of_first_element: 10,
        }
    ]);
    let mut rng = rand::thread_rng();
    let entities = (0..10_000).map(|_i| {
        EntityRenderData {
            pos: Vector3::<f32>::new(2.0*ground.width as f32 * (rng.gen::<f32>()), 8.0, -(2.0*ground.height as f32 * (rng.gen::<f32>()))),
        }
    }).collect();
    MapRenderData {
        gnd: ground,
        rsw: world,
        ground_vertex_array_obj: vertex_array,
        models: model_render_datas,
        texture_atlas,
        tile_color_texture,
        lightmap_texture,
        model_instances,
        sprite_vertex_array,
        entities,
    }
}