use collada::Animation as ColladaAnim;
use collada::document::ColladaDocument;
use collada::Skeleton;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::num::Float;
use vecmath::{Vector3, Matrix4, mat4_id, row_mat4_transform, row_mat4_mul, mat4_transposed, mat4_sub};
use quaternion::Quaternion;

use gfx::{Device};
use gfx_debug_draw::DebugRenderer;

use gfx_device_gl::Resources as GlResources;
use gfx_device_gl::Device as GlDevice;
use gfx_device_gl::Factory as GlFactory;

use math::*;

use interpolation::{Spatial, lerp};

use std::rc::Rc;
use std::cell::RefCell;

use rustc_serialize::{self, Decodable, Decoder, json};

use std::fs::File;
use std::io::Read;
use std::path::Path;


///
/// SQT - (Scale, Quaternion, Translation)
/// Transformation represented by separate scaling, translation, and rotation factors
/// Necessary for rotational interpolation
///
#[derive(Debug, Copy, Clone)]
pub struct SQT
{
    ///
    /// 3D Translation
    ///
    pub translation: Vector3<f32>,
    ///
    /// Uniform scale factor.
    ///
    pub scale: f32,
    ///
    /// Rotation
    ///
    pub rotation: Quaternion<f32>
}

#[derive(Debug)]
pub struct AnimationSample
{
    ///
    /// Local pose transforms for each joint in the targeted skeleton
    /// (relative to parent joint)
    ///
    pub local_poses: Vec<SQT>,
}


#[derive(Debug)]
pub struct AnimationClip {
    pub samples: Vec<AnimationSample>,

    ///
    /// Assumes constant sample rate for animation
    ///
    pub samples_per_second: f32,
}

/// rotation matrix for `a` radians about z
/// TODO move
pub fn mat4_rotate_z(a: f32) -> Matrix4<f32> {
    [
        [a.cos(), -a.sin(), 0.0, 0.0],
        [a.sin(), a.cos(), 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

impl AnimationClip {

    pub fn sample_at_time(&self, elapsed_time: f32) -> &AnimationSample {
        let sample_index = (elapsed_time * self.samples_per_second) as usize % self.samples.len();
        &self.samples[sample_index]
    }

    ///
    /// Sets sample_per_second such that the animation will have the given
    /// duration
    ///
    pub fn set_duration(&mut self, duration: f32) {
        self.samples_per_second = self.samples.len() as f32 / duration;
    }

    pub fn get_interpolated_poses_at_time(&self, elapsed_time: f32, blended_poses: &mut [SQT]) {

        let interpolated_index = elapsed_time * self.samples_per_second;

        let index_1 = interpolated_index.floor() as usize;
        let index_2 = interpolated_index.ceil() as usize;

        let blend_factor = interpolated_index - index_1 as f32;

        let index_1 = index_1 % self.samples.len();
        let index_2 = index_2 % self.samples.len();

        let sample_1 = &self.samples[index_1];
        let sample_2 = &self.samples[index_2];


        for i in (0 .. sample_1.local_poses.len()) {

            let pose_1 = &sample_1.local_poses[i];
            let pose_2 = &sample_2.local_poses[i];

            let blended_pose = &mut blended_poses[i];
            blended_pose.scale = lerp(&pose_1.scale, &pose_2.scale, &blend_factor);
            blended_pose.translation = lerp(&pose_1.translation, &pose_2.translation, &blend_factor);
            blended_pose.rotation = lerp_quaternion(&pose_1.rotation, &pose_2.rotation, &blend_factor);

        }

    }

    pub fn from_collada(skeleton: &Skeleton, animations: &Vec<ColladaAnim>, transform: &Matrix4<f32>) -> AnimationClip {
        use std::f32::consts::PI;

        // Z-axis is 'up' in COLLADA, so need to rotate root pose about x-axis so y-axis is 'up'
        let rotate_on_x =
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, (PI/2.0).cos(), (PI/2.0).sin(), 0.0],
                [0.0, (-PI/2.0).sin(), (PI/2.0).cos(), 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];

        let transform = row_mat4_mul(rotate_on_x, *transform);

        // Build an index of joint names to anims
        let mut joint_animations = HashMap::new();
        for anim in animations.iter() {
            let joint_name = anim.target.split('/').next().unwrap();
            joint_animations.insert(joint_name, anim);
        }

        // Assuming all ColladaAnims have the same number of samples..
        let sample_count = animations[0].sample_times.len();

        // Assuming all ColladaAnims have the same duration..
        let duration = *animations[0].sample_times.last().unwrap();

        // Assuming constant sample rate
        let samples_per_second = sample_count as f32 / duration;

        let samples = (0 .. sample_count).map(|sample_index| {

            // Grab local poses for each joint from COLLADA animation if available,
            // falling back to identity matrix
            let local_poses: Vec<Matrix4<f32>> = skeleton.joints.iter().map(|joint| {
                match joint_animations.get(&joint.name[..]) {
                    Some(a) if joint.is_root() => row_mat4_mul(transform, a.sample_poses[sample_index]),
                    Some(a) => a.sample_poses[sample_index],
                    None => mat4_id(),
                }
            }).collect();

            // Convert local poses to SQT (for interpolation)
            let local_poses: Vec<SQT> = local_poses.iter().map(|pose_matrix| {
                SQT {
                    translation: [
                        pose_matrix[0][3],
                        pose_matrix[1][3],
                        pose_matrix[2][3],
                    ],
                    scale: 1.0, // TODO don't assume?
                    rotation: matrix_to_quaternion(pose_matrix),
                }
            }).collect();

            AnimationSample {
                local_poses: local_poses,
            }
        }).collect();

        AnimationClip {
            samples_per_second: samples_per_second,
            samples: samples,
        }
    }
}

// TODO - have a crate-local skeleton struct (not from collada!), move this to it's impl
pub fn draw_skeleton(skeleton: Rc<RefCell<Skeleton>>, global_poses: &[Matrix4<f32>], debug_renderer: &mut DebugRenderer<GlResources>, draw_labels: bool) {
    for (joint_index, joint) in skeleton.borrow().joints.iter().enumerate() {

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

            if !skeleton.borrow().joints.iter().any(|j| j.parent_index as usize == joint_index) {

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
                [1.0, 1.0, 1.0, 1.0]
            );
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

pub fn load_animations(path: &str) ->  Result<HashMap<String, Rc<RefCell<AnimationClip>>>, &'static str> {

    let file_result = File::open(path);

    let mut file = match file_result {
        Ok(file) => file,
        Err(_) => return Err("Failed to open definition file at path.")
    };

    let mut json_string = String::new();
    match file.read_to_string(&mut json_string) {
        Ok(_) => {},
        Err(_) => return Err("Failed to read definition file.")
    };

    let json = match json::Json::from_str(&json_string[..]) {
        Ok(x) => x,
        Err(e) => return Err("invalid json!?")
    };

    let mut clips = HashMap::new();

    let mut decoder = json::Decoder::new(json);

    decoder.read_seq(|decoder, len| {
        for i in (0 .. len) {
            decoder.read_struct("root", 0, |decoder| {

                let name = try!(decoder.read_struct_field("name", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                let source = try!(decoder.read_struct_field("source", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                let looping = try!(decoder.read_struct_field("looping", 0, |decoder| { Ok(try!(decoder.read_bool())) }));
                let duration = try!(decoder.read_struct_field("duration", 0, |decoder| { Ok(try!(decoder.read_f32())) }));
                let rotate_z_angle = try!(decoder.read_struct_field("rotate-z", 0, |decoder| { Ok(try!(decoder.read_f32())) }));

                // Wacky. Shouldn't it be an error if the struct field isn't present?
                let adjust = if !rotate_z_angle.is_nan() {
                    mat4_rotate_z(rotate_z_angle.to_radians())
                } else {
                    mat4_id()
                };

                let collada_document = ColladaDocument::from_path(&Path::new(&source[..])).unwrap();
                let animations = collada_document.get_animations();
                let mut skeleton_set = collada_document.get_skeletons().unwrap();
                let skeleton = &skeleton_set[0];

                let mut clip = AnimationClip::from_collada(skeleton, &animations, &adjust);
                clip.set_duration(duration);

                clips.insert(name, Rc::new(RefCell::new(clip)));

                Ok(0)
            });
        }

        Ok(0)
    });

    Ok(clips)
}
