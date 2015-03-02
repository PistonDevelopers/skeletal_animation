use collada::document::ColladaDocument;
use collada::{BindData, VertexWeight, Skeleton};
use geometry::Object as GeometryObject;
use geometry::{ Position, TextureCoords, Normal, Geometry };
use gfx::batch::RefBatch;
use gfx::state::Comparison;
use gfx::{ BufferHandle, BufferUsage, Device, DeviceExt, DrawState, Frame, Graphics, PrimitiveType, ProgramError, Resources, ToSlice, RawBufferHandle };
use gfx_device_gl::{ GlDevice, GlResources };
use quack::{ SetAt };
use std::default::Default;
use vecmath::*;
use wavefront_obj as wobj;
use wavefront_obj::obj::Object;

use animation::AnimationClip;

static MAX_JOINTS: usize = 64;

pub struct SkinnedRenderer {
    animation_clip: AnimationClip,
    skinning_transforms_buffer: BufferHandle<GlResources, [[f32; 4]; 4]>,
    shader_params: SkinnedShaderParams<GlResources>,
    batch: RefBatch<SkinnedShaderParams<GlResources>>,
}

impl SkinnedRenderer {

    pub fn from_collada(
        graphics: &mut Graphics<GlDevice>,
        collada_document: ColladaDocument
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

        let mut vertex_data: Vec<SkinnedVertex> = Vec::new();
        let mut index_data: Vec<u32> = Vec::new();
        let mut geometry_data: Geometry = Geometry::new();
        GeometryObject::add_object(&obj_set.objects[0], &mut vertex_data, &mut index_data, &mut geometry_data);

        let bind_data_set = collada_document.get_bind_data_set().unwrap();
        bind_vertices(&mut vertex_data, &bind_data_set.bind_data[0], &skeleton, &obj_set.objects[0]);
        let mesh = graphics.device.create_mesh(vertex_data.as_slice());

        let state = DrawState::new().depth(Comparison::LessEqual, true);

        let slice = graphics.device
            .create_buffer_static::<u32>(index_data.as_slice())
            .to_slice(PrimitiveType::TriangleList);

        let batch: RefBatch<SkinnedShaderParams<GlResources>> = graphics.make_batch(&program, &mesh, slice, &state).unwrap();

        let skinning_transforms_buffer = graphics.device.create_buffer::<[[f32; 4]; 4]>(MAX_JOINTS, BufferUsage::Dynamic);

        Ok(SkinnedRenderer {
            animation_clip: animation_clip,
            skinning_transforms_buffer: skinning_transforms_buffer,
            batch: batch,
            shader_params: SkinnedShaderParams {
                u_model_view_proj: mat4_id(),
                u_model_view: mat4_id(),
                u_skinning_transforms: skinning_transforms_buffer.raw(),
            },
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
        self.shader_params.u_model_view = view;
        self.shader_params.u_model_view_proj = projection;

        let sample = self.animation_clip.sample_at_time(elapsed_time);
        graphics.device.update_buffer(self.skinning_transforms_buffer.clone(), &sample.skinning_transforms[..], 0);
        graphics.draw(&self.batch, &self.shader_params, frame);
    }
}

#[shader_param]
struct SkinnedShaderParams<R: Resources> {
    u_model_view_proj: [[f32; 4]; 4],
    u_model_view: [[f32; 4]; 4],
    u_skinning_transforms: RawBufferHandle<R>,
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
        v_TexCoord = uv;

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

    in vec3 v_normal;
    out vec4 out_color;

    in vec2 v_TexCoord;

    void main() {
        // unidirectional light in direction as camera
        vec3 light = vec3(0.0, 0.0, 1.0);
        light = normalize(light);
        float intensity = max(dot(v_normal, light), 0.0);
        out_color = vec4(intensity, intensity, intensity, 1.0);
    }
";

fn bind_vertex(vertex: &mut SkinnedVertex, vtn_index: wobj::obj::VTNIndex, bind_data: &BindData, skeleton: &Skeleton) {

    let vertex_weights: Vec<&VertexWeight> = bind_data.vertex_weights.iter()
        .filter(|vw| vw.vertex == vtn_index.0)
        .collect();

    for (i, vertex_weight) in vertex_weights.iter().take(4).enumerate() {
        let joint_name = &bind_data.joint_names[vertex_weight.joint as usize];
        if let Some((joint_index, _)) = skeleton.joints.iter().enumerate()
            .find(|&(_, j)| &j.name == joint_name)
        {
            vertex.joint_indices[i] = joint_index as i32;
            vertex.joint_weights[i] = bind_data.weights[vertex_weight.weight];
        }
    }
}

///
/// For each vertex, set joint_indices and joint_weights according to BindData
/// TODO currently this is SUPER SLOW - probably need a combination of:
///     - writing this less dumb
///     - importing COLLADA file in a more convenient format
///     - some kind of rudimentary asset pipeline with caching so we don't have to
///       rebuild on every run
///
fn bind_vertices(vertices: &mut Vec<SkinnedVertex>, bind_data: &BindData, skeleton: &Skeleton, object: &Object) {

    let mut vertex_index = 0;
    for shape in object.geometry[0].shapes.iter() {
        match shape {
            &wobj::obj::Shape::Triangle(a, b, c) => {
                bind_vertex(&mut vertices[vertex_index], a, bind_data, skeleton);
                bind_vertex(&mut vertices[vertex_index + 1], b, bind_data, skeleton);
                bind_vertex(&mut vertices[vertex_index + 2], c, bind_data, skeleton);
                vertex_index += 3;
            }
            _ => {}
        }
    }
}
