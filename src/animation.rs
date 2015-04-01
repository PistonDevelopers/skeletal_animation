use collada::Animation as ColladaAnim;
use collada::Skeleton;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::num::Float;
use vecmath::{Vector3, Matrix4, mat4_id, row_mat4_transform, row_mat4_mul, mat4_transposed};
use quaternion::id as quaternion_id;
use quaternion::Quaternion;

use gfx::{Device};
use gfx_debug_draw::DebugRenderer;

use math::{quaternion_to_matrix, matrix_to_quaternion};

#[derive(Debug)]
pub struct AnimationClip<D: Device> {
    pub samples: Vec<AnimationSample<D>>,

    ///
    /// Assumes constant sample rate for animation
    ///
    pub samples_per_second: f32,
}

impl<D: Device> AnimationClip<D> {

    pub fn sample_at_time(&self, elapsed_time: f32) -> &AnimationSample<D> {
        let sample_index = (elapsed_time * self.samples_per_second) as usize % self.samples.len();
        &self.samples[sample_index]
    }

    pub fn from_collada(skeleton: &Skeleton, animations: &Vec<ColladaAnim>) -> AnimationClip<D> {

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
                    Some(a) => a.sample_poses[sample_index], // convert col major to row major
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

            let global_poses = calculate_global_poses_sqt(&skeleton, &local_poses);
            let skinning_transforms = calculate_skinning_transforms(&skeleton, &global_poses);

            AnimationSample {
                local_poses: local_poses,
                global_poses: global_poses,
                skinning_transforms: skinning_transforms,
                _device_marker: PhantomData,
            }
        }).collect();

        AnimationClip {
            samples_per_second: samples_per_second,
            samples: samples,
        }
    }
}

fn calculate_skinning_transforms(
    skeleton: &Skeleton,
    global_poses: &Vec<Matrix4<f32>>,
) -> Vec<Matrix4<f32>> {

    use std::f32::consts::PI;
    use std::num::{Float};

    let rotate_on_x_inv =
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, (-PI/2.0).cos(), (-PI/2.0).sin(), 0.0],
        [0.0, (PI/2.0).sin(), (-PI/2.0).cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    skeleton.joints.iter().enumerate().map(|(i, joint)| {
        let inverse_bind_pose = row_mat4_mul(joint.inverse_bind_pose, mat4_id());
        row_mat4_mul(global_poses[i], inverse_bind_pose)
    }).collect()
}

fn calculate_global_poses(
    skeleton: &Skeleton,
    local_poses: &Vec<Matrix4<f32>>,
) -> Vec<Matrix4<f32>> {

    use std::f32::consts::PI;
    use std::num::{Float};

    let rotate_on_x =
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, (PI/2.0).cos(), (PI/2.0).sin(), 0.0],
        [0.0, (-PI/2.0).sin(), (PI/2.0).cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    let mut global_poses: Vec<Matrix4<f32>> = Vec::new();

    for (joint_index, joint) in skeleton.joints.iter().enumerate() {

        let parent_pose = if !joint.is_root() {
            global_poses[joint.parent_index as usize]
        } else {
            // COLLADA format treats y-axis as 'up', so
            // we need to do a PI/2 rotation around x axis
            // to adjust for that
            // TODO do this as step in collada importer
            rotate_on_x
        };

        let m = row_mat4_mul(parent_pose, local_poses[joint_index]);

        global_poses.push(row_mat4_mul(
            parent_pose,
            local_poses[joint_index]
        ));
    }

    global_poses
}

fn calculate_global_poses_sqt(
    skeleton: &Skeleton,
    local_poses: &Vec<SQT>,
) -> Vec<Matrix4<f32>> {

    use std::f32::consts::PI;
    use std::num::{Float};

    let rotate_on_x =
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, (PI/2.0).cos(), (PI/2.0).sin(), 0.0],
        [0.0, (-PI/2.0).sin(), (PI/2.0).cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    let mut global_poses: Vec<Matrix4<f32>> = Vec::new();

    for (joint_index, joint) in skeleton.joints.iter().enumerate() {

        let parent_pose = if !joint.is_root() {
            global_poses[joint.parent_index as usize]
        } else {
            rotate_on_x
        };

        let local_pose_sqt = &local_poses[joint_index];

        let mut local_pose = quaternion_to_matrix(local_pose_sqt.rotation);

        local_pose[0][3] = local_pose_sqt.translation[0];
        local_pose[1][3] = local_pose_sqt.translation[1];
        local_pose[2][3] = local_pose_sqt.translation[2];

        global_poses.push(row_mat4_mul(parent_pose, local_pose));
    }

    global_poses
}

///
/// SQT - (Scale, Quaternion, Translation)
/// Transformation represented by separate scaling, translation, and rotation factors
/// Necessary for rotational interpolation
///
#[derive(Debug)]
pub struct SQT
{
    ///
    /// 3D Translation
    ///
    translation: Vector3<f32>,
    ///
    /// Uniform scale factor.
    ///
    scale: f32,
    ///
    /// Rotation
    ///
    rotation: Quaternion<f32>
}

#[derive(Debug)]
pub struct AnimationSample<D: Device>
{
    ///
    /// Local pose transforms for each joint in the targeted skeleton
    /// (relative to parent joint)
    ///
    local_poses: Vec<SQT>,

    ///
    /// Global pose transforms for each joint in the targeted skeleton
    /// (relative to model)
    ///
    global_poses: Vec<Matrix4<f32>>,

    ///
    /// Skinning matrices that transform a bind-pose vertex in model-space
    /// to its new position in model-space according to this joint's current pose
    /// (relative to model)
    ///
    pub skinning_transforms: Vec<Matrix4<f32>>,

    _device_marker: PhantomData<D>,
}

impl<D: Device> AnimationSample<D> {

    pub fn debug_draw(&self, debug_renderer: &mut DebugRenderer<D>, skeleton: &Skeleton, draw_labels: bool) {
        for (joint_index, joint) in skeleton.joints.iter().enumerate() {

            let joint_position = row_mat4_transform(self.global_poses[joint_index], [0.0, 0.0, 0.0, 1.0]);

            let leaf_end = row_mat4_transform(
                self.global_poses[joint_index],
                [0.0, 1.0, 0.0, 1.0]
                );

            if !joint.is_root() {
                let parent_position = row_mat4_transform(self.global_poses[joint.parent_index as usize], [0.0, 0.0, 0.0, 1.0]);

                // Draw bone (between joint and parent joint)

                debug_renderer.draw_line(
                    [parent_position[0], parent_position[1], parent_position[2]],
                    [joint_position[0], joint_position[1], joint_position[2]],
                    [0.2, 0.2, 0.2, 1.0]
                    );

                if !skeleton.joints.iter().any(|j| j.parent_index as usize == joint_index) {

                    // Draw extension along joint's y-axis...
                    // TODO is y-axis 'forward' in joint-space? are there conventions for this?

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
                self.global_poses[joint_index],
                [1.0, 0.0, 0.0, 1.0]
            );

            let p_y_axis = row_mat4_transform(
                self.global_poses[joint_index],
                [0.0, 1.0, 0.0, 1.0]
            );

            let p_z_axis = row_mat4_transform(
                self.global_poses[joint_index],
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
