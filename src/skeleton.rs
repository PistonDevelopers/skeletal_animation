use gfx;
use gfx_debug_draw;

use collada;
use math::*;
use transform::Transform;

pub type JointIndex = u8;
pub const ROOT_JOINT_PARENT_INDEX: JointIndex  = 255u8;

#[derive(Debug, Clone)]
pub struct Skeleton {
    ///
    /// All joints in the skeleton
    ///
    pub joints: Vec<Joint>,
}

impl Skeleton {

    ///
    /// Build a skeleton fromm a Collada skeleton
    ///
    pub fn from_collada(skeleton: &collada::Skeleton) -> Skeleton {
        Skeleton {
            joints: skeleton.joints.iter().map(|j| {
                Joint {
                    name: j.name.clone(),
                    parent_index: j.parent_index,
                    inverse_bind_pose: j.inverse_bind_pose,
                }
            }).collect()
        }
    }

    pub fn draw<R: gfx::Resources, F: gfx::Factory<R>, T: Transform> (
        &self,
        global_poses: &[T],
        debug_renderer: &mut gfx_debug_draw::DebugRenderer<R, F>,
        draw_labels: bool)
    {

        for (joint_index, joint) in self.joints.iter().enumerate() {

            let joint_position = global_poses[joint_index].transform_vector([0.0, 0.0, 0.0]);
            let leaf_end = global_poses[joint_index].transform_vector([0.0, 1.0, 0.0]);

            if !joint.is_root() {

                let parent_position = global_poses[joint.parent_index as usize].transform_vector([0.0, 0.0, 0.0]);

                // Draw bone (between joint and parent joint)

                debug_renderer.draw_line(
                    [parent_position[0], parent_position[1], parent_position[2]],
                    [joint_position[0], joint_position[1], joint_position[2]],
                    [0.2, 0.2, 0.2, 1.0]
                    );

                if !self.joints.iter().any(|j| j.parent_index as usize == joint_index) {

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
            let p_x_axis = global_poses[joint_index].transform_vector([1.0, 0.0, 0.0]);
            let p_y_axis = global_poses[joint_index].transform_vector([0.0, 1.0, 0.0]);
            let p_z_axis = global_poses[joint_index].transform_vector([0.0, 0.0, 1.0]);

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

#[derive(Debug, Clone)]
pub struct Joint {
    ///
    /// Name of joint
    ///
    pub name: String,

    ///
    /// Index of parent joint in Skeleton's 'joints' vector
    ///
    pub parent_index: JointIndex,

    ///
    /// Matrix transforming vertex coordinates from model-space to joint-space
    /// Column-major.
    ///
    pub inverse_bind_pose: Matrix4<f32>,
}

impl Joint {
    pub fn is_root(&self) -> bool {
        self.parent_index == ROOT_JOINT_PARENT_INDEX
    }
}
