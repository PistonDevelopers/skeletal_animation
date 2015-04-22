use std::collections::HashMap;
use std::path::Path;

use collada::document::ColladaDocument;
use collada;
use interpolation::{self, Spatial};

use math::*;
use skeleton::Skeleton;
use transform::Transform;

/// A single skeletal pose
#[derive(Debug)]
pub struct AnimationSample
{

    /// Local pose transforms for each joint in the targeted skeleton
    /// (relative to parent joint)
    pub local_poses: Vec<Transform>,

}

/// A sequence of skeletal pose samples at some sample rate
#[derive(Debug)]
pub struct AnimationClip {

    /// The sequence of skeletal poses
    pub samples: Vec<AnimationSample>,

    /// Sample rate for the clip. Assumes a constant sample rate.
    pub samples_per_second: f32,

}

#[derive(Debug, RustcDecodable)]
pub struct AnimationClipDef {
    pub name: String,
    pub source: String,
    pub duration: f32,
    pub rotate_z: f32,
}

#[derive(Debug, RustcDecodable)]
pub struct DifferenceClipDef {
    pub name: String,
    pub source_clip: String,
    pub reference_clip: String,
}

impl AnimationClip {

    pub fn from_def(clip_def: &AnimationClipDef) -> AnimationClip {

        // Wacky. Shouldn't it be an error if the struct field isn't present?
        // FIXME - use an Option
        let adjust = if !clip_def.rotate_z.is_nan() {
            mat4_rotate_z(clip_def.rotate_z.to_radians())
        } else {
            mat4_id()
        };

        // FIXME - load skeleton separately?
        let collada_document = ColladaDocument::from_path(&Path::new(&clip_def.source[..])).unwrap();
        let animations = collada_document.get_animations();
        let skeleton_set = collada_document.get_skeletons().unwrap();
        let skeleton = Skeleton::from_collada(&skeleton_set[0]);

        let mut clip = AnimationClip::from_collada(&skeleton, &animations, &adjust);

        if !clip_def.duration.is_nan() {
            clip.set_duration(clip_def.duration);
        }
        clip

    }

    /// Overrides the sampling rate of the clip to give the given duration (in seconds).
    pub fn set_duration(&mut self, duration: f32) {
        self.samples_per_second = self.samples.len() as f32 / duration;
    }

    /// Obtains the interpolated skeletal pose at the given sampling time.
    ///
    /// # Arguments
    ///
    /// * `time` - The time to sample with, relative to the start of the animation
    /// * `blended_poses` - The output array slice of joint transforms that will be populated
    ///                     for each joint in the skeleton.
    pub fn get_pose_at_time(&self, elapsed_time: f32, blended_poses: &mut [Transform]) {

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
            blended_pose.scale = interpolation::lerp(&pose_1.scale, &pose_2.scale, &blend_factor);
            blended_pose.translation = interpolation::lerp(&pose_1.translation, &pose_2.translation, &blend_factor);
            blended_pose.rotation = lerp_quaternion(&pose_1.rotation, &pose_2.rotation, &blend_factor);

        }

    }

    /// Create a difference clip from a source and reference clip for additive blending.
    pub fn as_difference_clip(source_clip: &AnimationClip, reference_clip: &AnimationClip) -> AnimationClip {

        let samples = (0 .. source_clip.samples.len()).map(|sample_index| {

            let ref source_sample = source_clip.samples[sample_index];

            // Extrapolate reference clip by wrapping, if reference clip is shorter than source clip
            let ref reference_sample = reference_clip.samples[sample_index % reference_clip.samples.len()];

            let difference_poses = (0 .. source_sample.local_poses.len()).map(|joint_index| {
                let ref source_pose = source_sample.local_poses[joint_index];
                let ref reference_pose = reference_sample.local_poses[joint_index];
                source_pose.subtract(reference_pose.clone())
            }).collect();

            AnimationSample {
                local_poses: difference_poses,
            }

        }).collect();

        AnimationClip {
            samples_per_second: source_clip.samples_per_second,
            samples: samples,
        }
    }

    /// Creates an `AnimationClip` from a collection of `collada::Animation`.
    ///
    /// # Arguments
    ///
    /// * `skeleton` - The `Skeleton` that the `AnimationClip` will be created for.
    /// * `animations` - The collection of `collada::Animation`s that will be converted into an
    ///                  `AnimationClip`, using the given `Skeleton`.
    /// * `transform` - An offset transform to apply to the root pose of each animation sample,
    ///                 useful for applying rotation, translation, or scaling when loading an
    ///                 animation.
    pub fn from_collada(skeleton: &Skeleton, animations: &Vec<collada::Animation>, transform: &Matrix4<f32>) -> AnimationClip {
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

            // Convert local poses to Transforms (for interpolation)
            let local_poses: Vec<Transform> = local_poses.iter().map(|pose_matrix| {
                Transform {
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
