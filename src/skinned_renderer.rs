use collada::document::ColladaDocument;
use collada::{self, BindData, VertexWeight, Skeleton};
use gfx::batch::RefBatch;
use gfx::shade::TextureParam;
use gfx::state::Comparison;
use gfx::tex::{SamplerInfo, FilterMethod, WrapMode};
use gfx::traits::*;
use gfx::{ BufferHandle, BufferUsage, DrawState, Frame, Graphics, PrimitiveType, ProgramError, Resources, RawBufferHandle };
use gfx_debug_draw::DebugRenderer;
use gfx_device_gl::Device as GlDevice;
use gfx_device_gl::Resources as GlResources;
use gfx_device_gl::Factory as GlFactory;
use gfx_texture::{ self, Texture };
use quack::{ SetAt };
use std::default::Default;
use std::path::Path;
use vecmath::*;

use animation::{AnimationClip, calculate_global_poses, calculate_skinning_transforms, SQT};

const MAX_JOINTS: usize = 64;

pub struct SkinnedRenderBatch {
    skinning_transforms_buffer: BufferHandle<GlResources, [[f32; 4]; 4]>,
    batch: RefBatch<SkinnedShaderParams<GlResources>>,
}
pub struct SkinnedRenderer {
    animation_clip: AnimationClip,
    skeleton: Skeleton, // TODO Should this be a ref? Should this just be the joints?
    render_batches: Vec<SkinnedRenderBatch>,
}

///
/// TEMP - Rust nightly doesn't like geometry::Object::add_object ...
/// So we'll just do it ourselves here..
///

fn vtn_to_vertex(a: collada::VTNIndex, obj: &collada::Object) -> SkinnedVertex
{
    let mut vertex: SkinnedVertex = Default::default();
    let position = obj.vertices[a.0];

    vertex.pos = [position.x as f32, position.y as f32, position.z as f32];

    if obj.joint_weights.len() == obj.vertices.len() {
        let weights = obj.joint_weights[a.0];
        vertex.joint_weights = weights.weights;
        vertex.joint_indices = [
            weights.joints[0] as i32,
            weights.joints[1] as i32,
            weights.joints[2] as i32,
            weights.joints[3] as i32,
        ];
    }

    if let Some(uv) = a.1 {
        let uv = obj.tex_vertices[uv];
        vertex.uv = [uv.x as f32, uv.y as f32];
    }

    if let Some(normal) = a.2 {
        let normal = obj.normals[normal];
        vertex.normal = [normal.x as f32, normal.y as f32, normal.z as f32];
    }

    vertex
}

fn get_vertex_index_data(obj: &collada::Object, vertex_data: &mut Vec<SkinnedVertex>, index_data: &mut Vec<u32>) {
    for geom in obj.geometry.iter() {
        let start = index_data.len();
        let mut i = vertex_data.len() as u32;
        let mut uvs: u32 = 0;
        let mut normals: u32 = 0;
        {
            let mut add = |a: collada::VTNIndex| {
                if let Some(_) = a.1 { uvs += 1; }
                if let Some(_) = a.2 { normals += 1; }
                vertex_data.push(vtn_to_vertex(a, obj));
                index_data.push(i);
                i += 1;
            };
            for shape in geom.shapes.iter() {
                match shape {
                    &collada::Shape::Triangle(a, b, c) => {
                        add(a);
                        add(b);
                        add(c);
                    }
                    _ => {}
                }
            }
        }
    }
}


impl SkinnedRenderer {

    pub fn from_collada(
        graphics: &mut Graphics<GlDevice, GlFactory>,
        collada_document: ColladaDocument,
        texture_paths: Vec<&str>, // TODO - read from the COLLADA document (if available)
    ) -> Result<SkinnedRenderer, ProgramError> {

        let program = match graphics.factory.link_program(SKINNED_VERTEX_SHADER.clone(), SKINNED_FRAGMENT_SHADER.clone()) {
            Ok(program_handle) => program_handle,
            Err(e) => return Err(e),
        };

        let obj_set = collada_document.get_obj_set().unwrap();

        let mut skeleton_set = collada_document.get_skeletons().unwrap();
        let mut animations = collada_document.get_animations();
        let mut skeleton = &skeleton_set[0];

        let animation_clip = AnimationClip::from_collada(skeleton, &animations);

        let mut render_batches = Vec::new();

        for (i, object) in obj_set.objects.iter().enumerate().take(6) {

            let mut vertex_data: Vec<SkinnedVertex> = Vec::new();
            let mut index_data: Vec<u32> = Vec::new();

            get_vertex_index_data(&object, &mut vertex_data, &mut index_data);

            let mesh = graphics.factory.create_mesh(vertex_data.as_slice());

            let state = DrawState::new().depth(Comparison::LessEqual, true);

            let slice = graphics.factory
                .create_buffer_index::<u32>(index_data.as_slice())
                .to_slice(PrimitiveType::TriangleList);

            let skinning_transforms_buffer = graphics.factory.create_buffer::<[[f32; 4]; 4]>(MAX_JOINTS, BufferUsage::Dynamic);

            let texture = Texture::from_path(
                &mut graphics.factory,
                &Path::new(&texture_paths[i]),
                &gfx_texture::Settings::new()
            ).unwrap();

            let sampler = graphics.factory.create_sampler(
                SamplerInfo::new(
                    FilterMethod::Trilinear,
                    WrapMode::Clamp
                    )
                );

            let shader_params = SkinnedShaderParams {
                u_model_view_proj: mat4_id(),
                u_model_view: mat4_id(),
                u_skinning_transforms: skinning_transforms_buffer.raw().clone(),
                u_texture: (texture.handle, Some(sampler)),
            };

            let batch: RefBatch<SkinnedShaderParams<GlResources>> = graphics.make_batch(&program, shader_params, &mesh, slice, &state).unwrap();

            render_batches.push(SkinnedRenderBatch {
                batch: batch,
                skinning_transforms_buffer: skinning_transforms_buffer,
            });
        }


        Ok(SkinnedRenderer {
            animation_clip: animation_clip,
            render_batches: render_batches,
            skeleton: skeleton.clone(),
        })
    }

    pub fn render(
        &mut self,
        graphics: &mut Graphics<GlDevice, GlFactory>,
        frame: &Frame<GlResources>,
        view: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        elapsed_time: f32,
    ) {
        for material in self.render_batches.iter_mut() {
            material.batch.params.u_model_view = view;
            material.batch.params.u_model_view_proj = projection;

            let mut local_poses = [ SQT { translation: [0.0, 0.0, 0.0], scale: 0.0, rotation: (0.0, [0.0, 0.0, 0.0]) }; MAX_JOINTS ];

            self.animation_clip.get_interpolated_poses_at_time(elapsed_time, &mut local_poses[0 .. self.skeleton.joints.len()]);

            // uses skeletal hierarchy to calculate global poses from the given local pose
            let global_poses = calculate_global_poses(&self.skeleton, &local_poses);

            // multiplies joint global poses with their inverse-bind-pose
            let skinning_transforms = calculate_skinning_transforms(&self.skeleton, &global_poses);

            graphics.factory.update_buffer(&material.skinning_transforms_buffer, &skinning_transforms[..], 0);
            graphics.draw(&material.batch, frame).unwrap();
        }
    }

    pub fn render_skeleton(&self, debug_renderer: &mut DebugRenderer<GlDevice, GlFactory>, elapsed_time: f32, draw_labels: bool) {

        let mut local_poses = [ SQT { translation: [0.0, 0.0, 0.0], scale: 0.0, rotation: (0.0, [0.0, 0.0, 0.0]) }; MAX_JOINTS ];
        self.animation_clip.get_interpolated_poses_at_time(elapsed_time, &mut local_poses[0 .. self.skeleton.joints.len()]);
        let global_poses = calculate_global_poses(&self.skeleton, &local_poses);

        for (joint_index, joint) in self.skeleton.joints.iter().enumerate() {

            let joint_position = row_mat4_transform(global_poses[joint_index], [0.0, 0.0, 0.0, 1.0]);

            let leaf_end = row_mat4_transform(
                global_poses[joint_index],
                [0.0, 1.0, 0.0, 1.0]
                );

            if !joint.is_root() {
                let parent_position = row_mat4_transform(global_poses[joint.parent_index as usize], [0.0, 0.0, 0.0, 1.0]);

                // Draw bone (between joint and parent joint)

                debug_renderer.draw_line(
                    [parent_position[0], parent_position[1], parent_position[2]],
                    [joint_position[0], joint_position[1], joint_position[2]],
                    [0.2, 0.2, 0.2, 1.0]
                    );

                if !self.skeleton.joints.iter().any(|j| j.parent_index as usize == joint_index) {

                    // Draw extension along joint's y-axis...
                    debug_renderer.draw_line(
                        [joint_position[0], joint_position[1], joint_position[2]],
                        [leaf_end[0], leaf_end[1], leaf_end[2]],
                        [0.2, 0.2, 0.2, 1.0]
                        );
                }
            }

            if draw_labels {
                // Label joint
                debug_renderer.draw_text_at_position(
                    &joint.name[..],
                    [leaf_end[0], leaf_end[1], leaf_end[2]],
                    [1.0, 1.0, 1.0, 1.0]);
            }

            // Draw joint-relative axes
            let p_x_axis = row_mat4_transform(
                global_poses[joint_index],
                [1.0, 0.0, 0.0, 1.0]
            );

            let p_y_axis = row_mat4_transform(
                global_poses[joint_index],
                [0.0, 1.0, 0.0, 1.0]
            );

            let p_z_axis = row_mat4_transform(
                global_poses[joint_index],
                [0.0, 0.0, 1.0, 1.0]
            );

            debug_renderer.draw_line(
                [joint_position[0], joint_position[1], joint_position[2]],
                [p_x_axis[0], p_x_axis[1], p_x_axis[2]],
                [1.0, 0.2, 0.2, 1.0]
            );

            debug_renderer.draw_line(
                [joint_position[0], joint_position[1], joint_position[2]],
                [p_y_axis[0], p_y_axis[1], p_y_axis[2]],
                [0.2, 1.0, 0.2, 1.0]
            );

            debug_renderer.draw_line(
                [joint_position[0], joint_position[1], joint_position[2]],
                [p_z_axis[0], p_z_axis[1], p_z_axis[2]],
                [0.2, 0.2, 1.0, 1.0]
            );
        }
    }
}

#[shader_param]
struct SkinnedShaderParams<R: Resources> {
    u_model_view_proj: [[f32; 4]; 4],
    u_model_view: [[f32; 4]; 4],
    u_skinning_transforms: RawBufferHandle<R>,
    u_texture: TextureParam<R>,
}

#[vertex_format]
#[derive(Copy)]
#[derive(Clone)]
#[derive(Debug)]
struct SkinnedVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    joint_indices: [i32; 4],
    joint_weights: [f32; 4], // TODO last weight is redundant
}

impl Default for SkinnedVertex {
    fn default() -> SkinnedVertex {
        SkinnedVertex {
            pos: [0.0; 3],
            normal: [0.0; 3],
            uv: [0.0; 2],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        }
    }
}

static SKINNED_VERTEX_SHADER: &'static [u8] = b"
    #version 150 core

    uniform mat4 u_model_view_proj;
    uniform mat4 u_model_view;

    const int MAX_JOINTS = 64;

    uniform u_skinning_transforms {
        mat4 skinning_transforms[MAX_JOINTS];
    };

    in vec3 pos, normal;
    in vec2 uv;

    in ivec4 joint_indices;
    in vec4 joint_weights;

    out vec3 v_normal;
    out vec2 v_TexCoord;

    void main() {
        v_TexCoord = vec2(uv.x, 1 - uv.y); // this feels like a bug with gfx?

        vec4 adjustedVertex;
        vec4 adjustedNormal;

        vec4 bindPoseVertex = vec4(pos, 1.0);
        vec4 bindPoseNormal = vec4(normal, 0.0);

        adjustedVertex = bindPoseVertex * skinning_transforms[joint_indices.x] * joint_weights.x;
        adjustedNormal = bindPoseNormal * skinning_transforms[joint_indices.x] * joint_weights.x;

        adjustedVertex = adjustedVertex + bindPoseVertex * skinning_transforms[joint_indices.y] * joint_weights.y;
        adjustedNormal = adjustedNormal + bindPoseNormal * skinning_transforms[joint_indices.y] * joint_weights.y;

        adjustedVertex = adjustedVertex + bindPoseVertex * skinning_transforms[joint_indices.z] * joint_weights.z;
        adjustedNormal = adjustedNormal + bindPoseNormal * skinning_transforms[joint_indices.z] * joint_weights.z;

        // TODO just use remainder for this weight?
        adjustedVertex = adjustedVertex + bindPoseVertex * skinning_transforms[joint_indices.a] * joint_weights.a;
        adjustedNormal = adjustedNormal + bindPoseNormal * skinning_transforms[joint_indices.a] * joint_weights.a;

        gl_Position = u_model_view_proj * adjustedVertex;
        v_normal = normalize(u_model_view * adjustedNormal).xyz;
    }
";

static SKINNED_FRAGMENT_SHADER: &'static [u8] = b"
    #version 150

    uniform sampler2D u_texture;

    in vec3 v_normal;
    out vec4 out_color;

    in vec2 v_TexCoord;

    void main() {
        vec4 texColor = texture(u_texture, v_TexCoord);

        // unidirectional light in direction as camera
        vec3 light = vec3(0.0, 0.0, 1.0);
        light = normalize(light);
        float intensity = max(dot(v_normal, light), 0.0);

        out_color = vec4(intensity, intensity, intensity, 1.0) * texColor;
    }
";
