use std::default::Default;
use std::path::Path;

use collada;
use gfx;
use gfx::memory::Typed;
use gfx::traits::*;
use gfx_texture;

use math::*;
use skeleton::Skeleton;
use transform::Transform;

const MAX_JOINTS: usize = 64;

pub struct SkinnedRenderBatch<R: gfx::Resources, T: Transform> {
    skinning_transforms_buffer: gfx::handle::Buffer<R, T>,
    slice: gfx::Slice<R>,
    vertex_buffer: gfx::handle::Buffer<R, SkinnedVertex>,
    texture: (gfx::handle::ShaderResourceView<R, [f32; 4]>, gfx::handle::Sampler<R>),
}

pub struct SkinnedRenderer<R: gfx::Resources, T: Transform> {
    pso: gfx::PipelineState<R, pipe::Meta>,
    skeleton: Skeleton, // TODO Should this be a ref? Should this just be the joints?
    render_batches: Vec<SkinnedRenderBatch<R, T>>,
}

pub trait HasShaderSources<'a> {
    fn vertex_shader_source() -> &'a [u8];
    fn fragment_shader_source() -> &'a [u8];
}

impl<'a> HasShaderSources<'a> for Matrix4<f32> {
    fn vertex_shader_source() -> &'a [u8] {
        include_bytes!("lbs_skinning_150.glslv")
    }
    fn fragment_shader_source() -> &'a [u8] {
        include_bytes!("skinning_150.glslf")
    }
}

impl<'a> HasShaderSources<'a> for DualQuaternion<f32> {
    fn vertex_shader_source() -> &'a [u8] {
        include_bytes!("dlb_skinning_150.glslv")
    }
    fn fragment_shader_source() -> &'a [u8] {
        include_bytes!("skinning_150.glslf")
    }
}

impl<'a, R: gfx::Resources, T: Transform + HasShaderSources<'a>> SkinnedRenderer<R, T> {

    pub fn from_collada<F: gfx::Factory<R>>(
        factory: &mut F,
        collada_document: collada::document::ColladaDocument,
        texture_paths: Vec<&str>, // TODO - read from the COLLADA document (if available)
    ) -> Result<Self, gfx::shade::ProgramError> {
        use gfx::format::Formatted;

        let program = {
            let vs = T::vertex_shader_source();
            let fs = T::fragment_shader_source();
            match factory.link_program(vs, fs) {
                Ok(program_handle) => program_handle,
                Err(e) => return Err(e),
            }
        };

        // TODO: Pass in format as parameter.
        let format = gfx::format::Srgba8::get_format();
        let init = pipe::Init {
            vertex: (),
            u_model_view_proj: "u_model_view_proj",
            u_model_view: "u_model_view",
            u_skinning_transforms: "u_skinning_transforms",
            u_texture: "u_texture",
            out_color: ("out_color", format, gfx::state::ColorMask::all(), None),
            out_depth: gfx::preset::depth::LESS_EQUAL_WRITE,
        };
        let pso = factory.create_pipeline_from_program(
            &program,
            gfx::Primitive::TriangleList,
            gfx::state::Rasterizer::new_fill(),
            init
        ).unwrap();

        let sampler = factory.create_sampler(
            gfx::texture::SamplerInfo::new(
                gfx::texture::FilterMethod::Trilinear,
                gfx::texture::WrapMode::Clamp
            )
        );

        let obj_set = collada_document.get_obj_set().unwrap();

        let skeleton_set = collada_document.get_skeletons().unwrap();
        let skeleton = Skeleton::from_collada(&skeleton_set[0]);

        let mut render_batches = Vec::new();

        for (i, object) in obj_set.objects.iter().enumerate().take(6) {

            let mut vertex_data: Vec<SkinnedVertex> = Vec::new();
            let mut index_data: Vec<u32> = Vec::new();

            get_vertex_index_data(&object, &mut vertex_data, &mut index_data);

            let (vbuf, slice) = factory.create_vertex_buffer_with_slice
                (&vertex_data, &index_data[..]);

            let skinning_transforms_buffer = factory.create_buffer::<T>(
                MAX_JOINTS,
                gfx::buffer::Role::Constant,
                gfx::memory::Usage::Dynamic,
                gfx::memory::Bind::empty()
            ).unwrap();

            let texture = gfx_texture::Texture::from_path(
                factory,
                &Path::new(&texture_paths[i]),
                gfx_texture::Flip::None,
                &gfx_texture::TextureSettings::new()
            ).unwrap();

            render_batches.push(SkinnedRenderBatch {
                slice: slice,
                vertex_buffer: vbuf,
                skinning_transforms_buffer: skinning_transforms_buffer,
                texture: (texture.view.clone(), sampler.clone()),
            });
        }


        Ok(Self {
            pso: pso,
            render_batches: render_batches,
            skeleton: skeleton.clone(),
        })
    }

    pub fn render<C: gfx::CommandBuffer<R>, Rf: gfx::format::RenderFormat> (
        &mut self,
        encoder: &mut gfx::Encoder<R, C>,
        out_color: &gfx::handle::RenderTargetView<R, Rf>,
        out_depth: &gfx::handle::DepthStencilView<R, gfx::format::DepthStencil>,
        view: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        joint_poses: &[T]
    )
        where T: gfx::traits::Pod
    {

        let skinning_transforms = self.calculate_skinning_transforms(&joint_poses);

        for material in self.render_batches.iter_mut() {
            // FIXME -- should all be able to share the same buffer
            encoder.update_buffer(&material.skinning_transforms_buffer, &skinning_transforms[..], 0).unwrap();

            let data = pipe::Data {
                vertex: material.vertex_buffer.clone(),
                u_model_view_proj: projection,
                u_model_view: view,
                u_skinning_transforms: material.skinning_transforms_buffer.raw().clone(),
                u_texture: material.texture.clone(),
                out_color: out_color.raw().clone(),
                out_depth: out_depth.clone(),
            };

            encoder.draw(&material.slice, &self.pso, &data);
        }
    }

    ///
    /// TODO - don't allocate a new vector
    ///
    pub fn calculate_skinning_transforms(&self, global_poses: &[T]) -> Vec<T> {
        self.skeleton.joints.iter().enumerate().map(|(i, joint)| {
            // TODO avoid conversion...
            global_poses[i].concat(T::from_matrix(joint.inverse_bind_pose))
        }).collect()
    }
}

gfx_pipeline_base!( pipe {
    vertex: gfx::VertexBuffer<SkinnedVertex>,
    u_model_view_proj: gfx::Global<[[f32; 4]; 4]>,
    u_model_view: gfx::Global<[[f32; 4]; 4]>,
    u_skinning_transforms: gfx::RawConstantBuffer,
    u_texture: gfx::TextureSampler<[f32; 4]>,
    out_color: gfx::RawRenderTarget,
    out_depth: gfx::DepthTarget<gfx::format::DepthStencil>,
});

/*
gfx_pipeline!( pipe {
    u_model_view_proj: gfx::Global<[[f32; 4]; 4]> = "u_model_view_proj",
    u_model_view: gfx::Global<[[f32; 4]; 4]> = "u_model_view",
    u_skinning_transforms: gfx::RawVertexBuffer = &[],
    u_texture: gfx::TextureSampler<[f32; 4]> = "u_texture",
    // out_color: gfx::RenderTarget<gfx::format::Srgba8> = "o_Color",
    out_color: gfx::RawRenderTarget = "o_Color",
    out_depth: gfx::DepthTarget<gfx::format::DepthStencil> =
        gfx::preset::depth::LESS_EQUAL_WRITE,
});
*/

gfx_vertex_struct!(SkinnedVertex {
    pos: [f32; 3] = "pos",
    normal: [f32; 3] = "normal",
    uv: [f32; 2] = "uv",
    joint_indices: [i32; 4] = "joint_indices",
    joint_weights: [f32; 4] = "joint_weights", // TODO last weight is redundant
});

impl Default for SkinnedVertex {
    fn default() -> Self {
        Self {
            pos: [0.0; 3],
            normal: [0.0; 3],
            uv: [0.0; 2],
            joint_indices: [0; 4],
            joint_weights: [0.0; 4],
        }
    }
}

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
            for mesh in geom.mesh.iter() {
                match mesh {
                    &collada::PrimitiveElement::Triangles(ref triangles) => {
                        for &(a, b, c) in &triangles.vertices {
                            add(a);
                            add(b);
                            add(c);
                        }
                    }
                    &collada::PrimitiveElement::Polylist(ref polylist) => {
                        for shape in &polylist.shapes {
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
        }
    }
}
