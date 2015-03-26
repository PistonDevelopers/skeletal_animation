use collada::document::ColladaDocument;
use collada::{BindData, VertexWeight, Skeleton};
use geometry::Object as GeometryObject;
use geometry::Model as GeometryModel;
use geometry::{ Position, TextureCoords, Normal, SkinningWeights, Geometry };
use gfx::batch::RefBatch;
use gfx::shade::TextureParam;
use gfx::state::Comparison;
use gfx::tex::{SamplerInfo, FilterMethod, WrapMode};
use gfx::traits::*;
use gfx::{ BufferHandle, BufferUsage, DrawState, Frame, Graphics, PrimitiveType, ProgramError, Resources, RawBufferHandle };
use gfx_device_gl::{ GlDevice, GlResources };
use gfx_texture::{ Texture };
use quack::{ SetAt };
use std::default::Default;
use std::old_path::*;
use vecmath::*;
use wavefront_obj as wobj;
use wavefront_obj::obj::Object;

use animation::AnimationClip;

static MAX_JOINTS: usize = 64;

pub struct SkinnedRenderBatch {
    skinning_transforms_buffer: BufferHandle<GlResources, [[f32; 4]; 4]>,
    batch: RefBatch<SkinnedShaderParams<GlResources>>,
}
pub struct SkinnedRenderer {
    animation_clip: AnimationClip<GlDevice>,
    render_batches: Vec<SkinnedRenderBatch>,
}

impl SkinnedRenderer {

    pub fn from_collada(
        graphics: &mut Graphics<GlDevice>,
        collada_document: ColladaDocument,
        texture_paths: Vec<&str>, // TODO - read from the COLLADA document (if available)
    ) -> Result<SkinnedRenderer, ProgramError> {

        let program = match graphics.device.link_program(SKINNED_VERTEX_SHADER.clone(), SKINNED_FRAGMENT_SHADER.clone()) {
            Ok(program_handle) => program_handle,
            Err(e) => return Err(e),
        };

        let obj_set = collada_document.get_obj_set().unwrap();
        let skeleton_set = collada_document.get_skeletons().unwrap();
        let animations = collada_document.get_animations();
        let skeleton = &skeleton_set[0];

        let animation_clip = AnimationClip::from_collada(skeleton, &animations);

        let mut render_batches = Vec::new();

        for (i, object) in obj_set.objects.iter().enumerate().take(6) {

            let mut vertex_data: Vec<SkinnedVertex> = Vec::new();
            let mut index_data: Vec<u32> = Vec::new();
            let mut geometry_data: Geometry = Geometry::new();
            let mut objects = GeometryObject::new();

            GeometryObject::add_object(&object, &mut vertex_data, &mut index_data, &mut geometry_data);

            let mesh = graphics.device.create_mesh(vertex_data.as_slice());

            let state = DrawState::new().depth(Comparison::LessEqual, true);

            let slice = graphics.device
                .create_buffer_index::<u32>(index_data.as_slice())
                .to_slice(PrimitiveType::TriangleList);

            let skinning_transforms_buffer = graphics.device.create_buffer::<[[f32; 4]; 4]>(MAX_JOINTS, BufferUsage::Dynamic);

            let texture = Texture::from_path(&mut graphics.device, &Path::new(&texture_paths[i])).unwrap();
            let sampler = graphics.device.create_sampler(
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
        })
    }

    pub fn render(
        &mut self,
        graphics: &mut Graphics<GlDevice>,
        frame: &Frame<GlResources>,
        view: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        elapsed_time: f32,
    ) {
        for material in self.render_batches.iter_mut() {
            material.batch.params.u_model_view = view;
            material.batch.params.u_model_view_proj = projection;

            let sample = self.animation_clip.sample_at_time(elapsed_time);
            graphics.device.update_buffer(&material.skinning_transforms_buffer, &sample.skinning_transforms[..], 0);
            graphics.draw(&material.batch, frame).unwrap();
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

impl SetAt for (Position, SkinnedVertex) {
    fn set_at(Position(pos): Position, vertex: &mut SkinnedVertex) {
        vertex.pos = pos;
    }
}

impl SetAt for (Normal, SkinnedVertex) {
    fn set_at(Normal(normal): Normal, vertex: &mut SkinnedVertex) {
        vertex.normal = normal;
    }
}

impl SetAt for (TextureCoords, SkinnedVertex) {
    fn set_at(TextureCoords(coords): TextureCoords, vertex: &mut SkinnedVertex) {
        vertex.uv = coords;
    }
}

impl SetAt for (SkinningWeights, SkinnedVertex) {
    fn set_at(SkinningWeights(joints, weights): SkinningWeights, vertex: &mut SkinnedVertex) {
        vertex.joint_indices = [
            joints[0] as i32,
            joints[1] as i32,
            joints[2] as i32,
            joints[3] as i32,
        ];
        vertex.joint_weights = weights;
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