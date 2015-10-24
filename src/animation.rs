use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use collada::document::ColladaDocument;
use collada;
use float::Radians;

use math::*;
use skeleton::Skeleton;
use transform::Transform;

/// A single skeletal pose
#[derive(Debug)]
pub struct AnimationSample<T: Transform>
{

    /// Local pose transforms for each joint in the targeted skeleton
    /// (relative to parent joint)
    pub local_poses: Vec<T>,

}

/// A sequence of skeletal pose samples at some sample rate
#[derive(Debug)]
pub struct AnimationClip<T: Transform> {

    /// The sequence of skeletal poses
    pub samples: Vec<AnimationSample<T>>,

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

impl<T: Transform> AnimationClip<T> {

    pub fn from_def(clip_def: &AnimationClipDef) -> AnimationClip<T> {

        // Wacky. Shouldn't it be an error if the struct field isn't present?
        // FIXME - use an Option
        let adjust = if !clip_def.rotate_z.is_nan() {
            mat4_rotate_z(clip_def.rotate_z.deg_to_rad())
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

    /// Return the duration of the clip in seconds
    pub fn get_duration(&self) -> f32 {
        self.samples.len() as f32 / self.samples_per_second
    }

    /// Obtains the interpolated skeletal pose at the given sampling time.
    ///
    /// # Arguments
    ///
    /// * `time` - The time to sample with, relative to the start of the animation
    /// * `blended_poses` - The output array slice of joint transforms that will be populated
    ///                     for each joint in the skeleton.
    pub fn get_pose_at_time(&self, elapsed_time: f32, blended_poses: &mut [T]) {

        let interpolated_index = elapsed_time * self.samples_per_second;

        let index_1 = interpolated_index.floor() as usize;
        let index_2 = interpolated_index.ceil() as usize;

        let blend_factor = interpolated_index - index_1 as f32;

        let index_1 = index_1 % self.samples.len();
        let index_2 = index_2 % self.samples.len();

        let sample_1 = &self.samples[index_1];
        let sample_2 = &self.samples[index_2];


        for i in 0 .. sample_1.local_poses.len() {

            let pose_1 = sample_1.local_poses[i];
            let pose_2 = sample_2.local_poses[i];

            let blended_pose = &mut blended_poses[i];
            *blended_pose = pose_1.lerp(pose_2, blend_factor);
        }

    }

    /// Create a difference clip from a source and reference clip for additive blending.
    pub fn as_difference_clip(source_clip: &AnimationClip<T>, reference_clip: &AnimationClip<T>) -> AnimationClip<T> {

        let samples = (0 .. source_clip.samples.len()).map(|sample_index| {

            let ref source_sample = source_clip.samples[sample_index];

            // Extrapolate reference clip by wrapping, if reference clip is shorter than source clip
            let ref reference_sample = reference_clip.samples[sample_index % reference_clip.samples.len()];

            let difference_poses = (0 .. source_sample.local_poses.len()).map(|joint_index| {
                let source_pose = source_sample.local_poses[joint_index];
                let reference_pose = reference_sample.local_poses[joint_index];
                reference_pose.inverse().concat(source_pose)
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
    pub fn from_collada(skeleton: &Skeleton, animations: &Vec<collada::Animation>, transform: &Matrix4<f32>) -> AnimationClip<T> {
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
            let local_poses: Vec<T> = local_poses.iter().map(|pose_matrix| {
                T::from_matrix(*pose_matrix)
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

/// An instance of an AnimationClip which tracks playback parameters
pub struct ClipInstance<T: Transform> {
    /// Shared clip reference
    pub clip: Rc<AnimationClip<T>>,

    /// Controller clock time at animation start
    pub start_time: f32,

    /// Playback rate modifier, where 1.0 is original speed
    pub playback_rate: f32,

    /// Used to account for changes in playback rate
    pub time_offset: f32,
}

impl<T: Transform> ClipInstance<T> {

    pub fn new(clip: Rc<AnimationClip<T>>) -> ClipInstance<T> {
        ClipInstance {
            clip: clip,
            start_time: 0.0,
            playback_rate: 1.0,
            time_offset: 0.0,
        }
    }

    /// Adjust the playback rate of the clip without affecting the
    /// value of get_local_time for a given global time.
    pub fn set_playback_rate(&mut self, global_time: f32, new_rate: f32) {
        if self.playback_rate != new_rate {
            let local_time = self.get_local_time(global_time);
            self.time_offset = local_time - (global_time - self.start_time) * new_rate;
            self.playback_rate = new_rate;
        }
    }

    pub fn get_pose_at_time(&self, global_time: f32, blended_poses: &mut [T]) {
        self.clip.get_pose_at_time(self.get_local_time(global_time), blended_poses);
    }

    pub fn get_duration(&self) -> f32 {
        self.clip.get_duration()
    }

    fn get_local_time(&self, global_time: f32) -> f32 {
        (global_time - self.start_time) * self.playback_rate + self.time_offset
    }
}
