use std::default::Default;
use std::path::Path;

use collada;
use gfx;
use gfx::traits::*;
use gfx_texture;

use math::*;
use skeleton::Skeleton;
use transform::Transform;

const MAX_JOINTS: usize = 64;

pub struct SkinnedRenderBatch<R: gfx::Resources, T: Transform> {
    skinning_transforms_buffer: gfx::BufferHandle<R, T>,
    batch: gfx::batch::RefBatch<SkinnedShaderParams<R>>,
}

pub struct SkinnedRenderer<R: gfx::Resources, T: Transform> {
    skeleton: Skeleton, // TODO Should this be a ref? Should this just be the joints?
    render_batches: Vec<SkinnedRenderBatch<R, T>>,
    context: gfx::render::batch::Context<R>,
}

pub trait HasShaderSources<'a> {
    fn vertex_shader_source() -> gfx::ShaderSource<'a>;
    fn fragment_shader_source() -> gfx::ShaderSource<'a>;
}

impl<'a> HasShaderSources<'a> for Matrix4<f32> {
    fn vertex_shader_source() -> gfx::ShaderSource<'a> {
        gfx::ShaderSource {
            glsl_150: Some(include_bytes!("lbs_skinning_150.glslv")),
            .. gfx::ShaderSource::empty()
        }
    }
    fn fragment_shader_source() -> gfx::ShaderSource<'a> {
        gfx::ShaderSource {
            glsl_150: Some(include_bytes!("skinning_150.glslf")),
            .. gfx::ShaderSource::empty()
        }
    }
}

impl<'a> HasShaderSources<'a> for DualQuaternion<f32> {
    fn vertex_shader_source() -> gfx::ShaderSource<'a> {
        gfx::ShaderSource {
            glsl_150: Some(include_bytes!("dlb_skinning_150.glslv")),
            .. gfx::ShaderSource::empty()
        }
    }
    fn fragment_shader_source() -> gfx::ShaderSource<'a> {
        gfx::ShaderSource {
            glsl_150: Some(include_bytes!("skinning_150.glslf")),
            .. gfx::ShaderSource::empty()
        }
    }
}

impl<'a, R: gfx::Resources, T: Transform + HasShaderSources<'a>> SkinnedRenderer<R, T> {

    pub fn from_collada_with_canvas<
        C: gfx::CommandBuffer<R>,
        F: gfx::Factory<R>,
        O: gfx::render::target::Output<R>,
        D: Device<Resources = R, CommandBuffer = C>,
    >(
        canvas: &mut gfx::Canvas<O, D, F>,
        collada_document: collada::document::ColladaDocument,
        texture_paths: Vec<&str>,
    ) -> Result<SkinnedRenderer<R, T>, gfx::ProgramError> {
        SkinnedRenderer::from_collada(&mut canvas.factory, &canvas.device, collada_document, texture_paths)
    }

    pub fn from_collada<
        F: gfx::Factory<R>,
        D: gfx::Device,
    > (
        factory: &mut F,
        device: &D,
        collada_document: collada::document::ColladaDocument,
        texture_paths: Vec<&str>, // TODO - read from the COLLADA document (if available)
    ) -> Result<SkinnedRenderer<R, T>, gfx::ProgramError> {

        let program = {
            let vs = T::vertex_shader_source();
            let fs = T::fragment_shader_source();
            match factory.link_program_source(vs, fs, device.get_capabilities()) {
                Ok(program_handle) => program_handle,
                Err(e) => return Err(e),
            }
        };

        let obj_set = collada_document.get_obj_set().unwrap();

        let skeleton_set = collada_document.get_skeletons().unwrap();
        let skeleton = Skeleton::from_collada(&skeleton_set[0]);

        let mut render_batches = Vec::new();

        let mut context = gfx::render::batch::Context::new();

        for (i, object) in obj_set.objects.iter().enumerate().take(6) {

            let mut vertex_data: Vec<SkinnedVertex> = Vec::new();
            let mut index_data: Vec<u32> = Vec::new();

            get_vertex_index_data(&object, &mut vertex_data, &mut index_data);

            let mesh = factory.create_mesh(vertex_data.as_slice());

            let state = gfx::DrawState::new().depth(gfx::state::Comparison::LessEqual, true);

            let slice = factory
                .create_buffer_index::<u32>(index_data.as_slice())
                .to_slice(gfx::PrimitiveType::TriangleList);

            let skinning_transforms_buffer = factory.create_buffer::<T>(MAX_JOINTS, gfx::BufferUsage::Dynamic);

            let texture = gfx_texture::Texture::from_path(
                factory,
                &Path::new(&texture_paths[i]),
                &gfx_texture::Settings::new()
            ).unwrap();

            let sampler = factory.create_sampler(
                gfx::tex::SamplerInfo::new(
                    gfx::tex::FilterMethod::Trilinear,
                    gfx::tex::WrapMode::Clamp
                )
            );

            let shader_params = SkinnedShaderParams {
                u_model_view_proj: mat4_id(),
                u_model_view: mat4_id(),
                u_skinning_transforms: skinning_transforms_buffer.raw().clone(),
                u_texture: (texture.handle(), Some(sampler)),
            };

            let batch: gfx::batch::RefBatch<SkinnedShaderParams<R>> = context.make_batch(&program, shader_params, &mesh, slice, &state).unwrap();

            render_batches.push(SkinnedRenderBatch {
                batch: batch,
                skinning_transforms_buffer: skinning_transforms_buffer,
            });
        }


        Ok(SkinnedRenderer {
            render_batches: render_batches,
            skeleton: skeleton.clone(),
            context: context,
        })
    }

    pub fn render_canvas<
        C: gfx::CommandBuffer<R>,
        F: gfx::Factory<R>,
        O: gfx::render::target::Output<R>,
        D: Device<Resources = R, CommandBuffer = C>,
    >(
        &mut self,
        canvas: &mut gfx::Canvas<O, D, F>,
        view: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        joint_poses: &[T]
    ) {
        self.render(&mut canvas.renderer, &mut canvas.factory, &canvas.output, view, projection, joint_poses);
    }

    pub fn render<
        C: gfx::CommandBuffer<R>,
        F: gfx::Factory<R>,
        O: gfx::render::target::Output<R>,
    >(
        &mut self,
        renderer: &mut gfx::Renderer<R, C>,
        factory: &mut F,
        output: &O,
        view: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        joint_poses: &[T]
    ) {

        let skinning_transforms = self.calculate_skinning_transforms(&joint_poses);

        for material in self.render_batches.iter_mut() {
            material.batch.params.u_model_view = view;
            material.batch.params.u_model_view_proj = projection;

            // FIXME -- should all be able to share the same buffer
            factory.update_buffer(&material.skinning_transforms_buffer, &skinning_transforms[..], 0);

            renderer.draw(&(&material.batch, &self.context), output).unwrap();
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

#[shader_param]
struct SkinnedShaderParams<R: gfx::Resources> {
    u_model_view_proj: [[f32; 4]; 4],
    u_model_view: [[f32; 4]; 4],
    u_skinning_transforms: gfx::RawBufferHandle<R>,
    u_texture: gfx::shade::TextureParam<R>,
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
