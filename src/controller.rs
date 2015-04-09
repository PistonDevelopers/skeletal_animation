use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use collada::Skeleton;
use vecmath::{self, Matrix4};

use animation::{SQT, AnimationClip};
use blend_tree::{BlendTreeNode, BlendTreeNodeDef, ClipId};
use math;

const MAX_PARAMS: usize = 16;
const MAX_JOINTS: usize = 64;

pub struct AnimationController {

    ///
    /// The blend tree used to calculate pose
    ///
    /// TODO replace with some sort of state machine,
    /// where each state either uses a blend tree or a single animation
    ///
    blend_tree: BlendTreeNode,

    ///
    /// Parameters that will be referenced by blend tree nodes and animation states
    ///
    ///
    ///
    parameters: HashMap<String, f32>,

    ///
    /// Shared reference to the skeleton this controller is using
    ///
    skeleton: Rc<RefCell<Skeleton>>,

    // NOTE - consider keeping a local clock here rather than a global clock for all controller
}

impl AnimationController {

    pub fn new(skeleton: Rc<RefCell<Skeleton>>, blend_tree_def: BlendTreeNodeDef, animations: &HashMap<ClipId, Rc<RefCell<AnimationClip>>>) -> AnimationController {

        let mut parameters = HashMap::new();
        for param in blend_tree_def.get_parameters().iter() {
            parameters.insert(param.clone(), 0.0);
        }

        let blend_tree = BlendTreeNode::from_def(blend_tree_def, animations);

        AnimationController {
            blend_tree: blend_tree,
            parameters: parameters,
            skeleton: skeleton.clone(),
        }
    }

    ///
    /// Set the value for the given parameter
    ///
    pub fn set_param_value(&mut self, name: &str, value: f32) {
        self.parameters.insert(name.to_string(), value); // :(
    }

    ///
    /// Return the value for the given parameter
    ///
    pub fn get_param_value(&self, name: &str) -> f32 {
        self.parameters[name]
    }

    ///
    /// Return a read-only reference to the parameter map
    ///
    pub fn get_parameters(&self) -> &HashMap<String, f32> {
        &self.parameters
    }

    ///
    /// Calculate GLOBAL skeletal joint poses for the given time
    ///
    pub fn get_output_pose(&self, elapsed_time: f32, output_poses: &mut [Matrix4<f32>]) {

        let mut local_poses = [ SQT {
            translation: [0.0, 0.0, 0.0],
            scale: 0.0,
            rotation: (0.0, [0.0, 0.0, 0.0])
        }; MAX_JOINTS ];

        self.blend_tree.get_output_pose(elapsed_time, &self.parameters, &mut local_poses[..]);
        self.calculate_global_poses(&local_poses[..], output_poses);

    }

    ///
    /// Calculate global poses from the controller's skeleton and the given local poses
    ///
    fn calculate_global_poses(
        &self,
        local_poses: &[SQT],
        global_poses: &mut [Matrix4<f32>],
    ) {

        for (joint_index, joint) in self.skeleton.borrow().joints.iter().enumerate() {

            let parent_pose = if !joint.is_root() {
                global_poses[joint.parent_index as usize]
            } else {
                vecmath::mat4_id()
            };

            let local_pose_sqt = &local_poses[joint_index];

            let mut local_pose = math::quaternion_to_matrix(local_pose_sqt.rotation);

            local_pose[0][3] = local_pose_sqt.translation[0];
            local_pose[1][3] = local_pose_sqt.translation[1];
            local_pose[2][3] = local_pose_sqt.translation[2];

            global_poses[joint_index] = vecmath::row_mat4_mul(parent_pose, local_pose);
        }
    }

}
