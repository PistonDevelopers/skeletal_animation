use collada::Animation as ColladaAnim;
use collada::document::ColladaDocument;
use collada::Skeleton;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::num::Float;

use gfx::{Device};

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
