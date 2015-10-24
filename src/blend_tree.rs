use std::collections::HashMap;
use std::rc::Rc;

use rustc_serialize::{Decodable, Decoder};

use animation::{AnimationClip, ClipInstance};
use skeleton::{Skeleton, JointIndex};

use transform::Transform;
use math::*;

/// Identifier for an AnimationClip within a BlendTreeNodeDef
pub type ClipId = String;

/// Identifier for animation controller parameter, within a LerpNode
pub type ParamId = String;

/// Definition of a blend tree, used by AnimationController to construct an AnimBlendTree
#[derive(Debug, Clone)]
pub enum BlendTreeNodeDef {
    LerpNode(Box<BlendTreeNodeDef>, Box<BlendTreeNodeDef>, ParamId),
    AdditiveNode(Box<BlendTreeNodeDef>, Box<BlendTreeNodeDef>, ParamId),
    IKNode(Box<BlendTreeNodeDef>, String, ParamId, ParamId, ParamId, ParamId, ParamId, ParamId, ParamId),
    ClipNode(ClipId),
}

impl Decodable for BlendTreeNodeDef {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<BlendTreeNodeDef, D::Error> {
        decoder.read_struct("root", 0, |decoder| {

            let node_type = try!(decoder.read_struct_field("type", 0, |decoder| { Ok(try!(decoder.read_str())) }));

            match &node_type[..] {
                "LerpNode" => {

                    let (input_1, input_2) = try!(decoder.read_struct_field("inputs", 0, |decoder| {
                        decoder.read_seq(|decoder, _len| {
                            Ok((
                                try!(decoder.read_seq_elt(0, Decodable::decode)),
                                try!(decoder.read_seq_elt(1, Decodable::decode))
                            ))
                        })
                    }));

                    let blend_param_name = try!(decoder.read_struct_field("param", 0, |decoder| { Ok(try!(decoder.read_str())) }));

                    Ok(BlendTreeNodeDef::LerpNode(Box::new(input_1), Box::new(input_2), blend_param_name))

                },
                "AdditiveNode" => {

                    let (input_1, input_2) = try!(decoder.read_struct_field("inputs", 0, |decoder| {
                        decoder.read_seq(|decoder, _len| {
                            Ok((
                                try!(decoder.read_seq_elt(0, Decodable::decode)),
                                try!(decoder.read_seq_elt(1, Decodable::decode))
                            ))
                        })
                    }));

                    let blend_param_name = try!(decoder.read_struct_field("param", 0, |decoder| { Ok(try!(decoder.read_str())) }));

                    Ok(BlendTreeNodeDef::AdditiveNode(Box::new(input_1), Box::new(input_2), blend_param_name))

                },
                "IKNode" => {

                    let input = try!(decoder.read_struct_field("input", 0, Decodable::decode));

                    let effector_name = try!(decoder.read_struct_field("effector", 0, |decoder| { Ok(try!(decoder.read_str())) }));

                    let blend_param_name = try!(decoder.read_struct_field("blend_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));

                    let target_x_name = try!(decoder.read_struct_field("target_x_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    let target_y_name = try!(decoder.read_struct_field("target_y_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    let target_z_name = try!(decoder.read_struct_field("target_z_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));

                    let bend_x_name = try!(decoder.read_struct_field("bend_x_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    let bend_y_name = try!(decoder.read_struct_field("bend_y_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    let bend_z_name = try!(decoder.read_struct_field("bend_z_param", 0, |decoder| { Ok(try!(decoder.read_str())) }));


                    Ok(BlendTreeNodeDef::IKNode(Box::new(input),
                                                effector_name,
                                                blend_param_name,
                                                target_x_name,
                                                target_y_name,
                                                target_z_name,
                                                bend_x_name,
                                                bend_y_name,
                                                bend_z_name))

                },
                "ClipNode" => {
                    let clip_source = try!(decoder.read_struct_field("clip_source", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    Ok(BlendTreeNodeDef::ClipNode(clip_source))
                }
                _ => panic!("Unexpected blend node type")
            }
        })
    }
}

/// A tree of AnimNodes
pub struct AnimBlendTree<T: Transform> {
    root_node: AnimNodeHandle,
    lerp_nodes: Vec<LerpAnimNode>,
    additive_nodes: Vec<AdditiveAnimNode>,
    ik_nodes: Vec<IKNode>,
    clip_nodes: Vec<ClipAnimNode<T>>,
    skeleton: Rc<Skeleton>,
}

impl<T: Transform> AnimBlendTree<T> {

    /// Initialize a new AnimBlendTree from the root BlendTreeNodeDef and
    /// a mapping from animation names to AnimationClip
    ///
    /// # Arguments
    ///
    /// * `def` - The root BlendTreeNodeDef
    /// * `animations` - A mapping from ClipIds to shared AnimationClip instances
    pub fn from_def(
        def: BlendTreeNodeDef,
        animations: &HashMap<ClipId, Rc<AnimationClip<T>>>,
        skeleton: Rc<Skeleton>,
    ) -> AnimBlendTree<T> {

        let mut tree = AnimBlendTree {
            root_node: AnimNodeHandle::None,
            lerp_nodes: Vec::new(),
            additive_nodes: Vec::new(),
            ik_nodes: Vec::new(),
            clip_nodes: Vec::new(),
            skeleton: skeleton.clone()
        };

        tree.root_node = tree.add_node(def, animations, &skeleton);
        tree
    }

    /// Get the output skeletal pose from the blend tree for the given time and parameters
    ///
    /// # Arguments
    ///
    /// * `time` - The time to sample from any AnimationClips
    /// * `params` - A mapping from ParamIds to their current parameter values
    /// * `output_poses` - The output array slice of joint transforms that will be populated
    ///                    according to the defined output for this BlendTreeNode
    pub fn get_output_pose(&self, time: f32, params: &HashMap<String, f32>, output_poses: &mut [T]) {
        if let Some(ref node) = self.get_node(self.root_node.clone()) {
            node.get_output_pose(self, time, params, output_poses);
        }
    }

    /// For each LerpNode with two animation clips, synchronize their playback rates according to the blend parameter
    ///
    /// # Arguments
    ///
    /// * `global_time` - The current global clock time from the controller
    /// * `params` - A mapping from ParamIds to their current parameter values
    pub fn synchronize(&mut self, global_time: f32, params: &HashMap<String, f32>) {
        for lerp_node in self.lerp_nodes.iter() {
            if let (AnimNodeHandle::ClipAnimNodeHandle(clip_1), AnimNodeHandle::ClipAnimNodeHandle(clip_2)) = (lerp_node.input_1.clone(), lerp_node.input_2.clone()) {
                let blend_parameter = params[&lerp_node.blend_param[..]];

                let target_length = {
                    let clip_1 = &self.clip_nodes[clip_1].clip;
                    let clip_2 = &self.clip_nodes[clip_2].clip;

                    let length_1 = clip_1.get_duration();
                    let length_2 = clip_2.get_duration();

                    (1.0 - blend_parameter) * length_1 + blend_parameter * length_2
                };

                {
                    let clip_1 = &mut self.clip_nodes[clip_1].clip;
                    let length = clip_1.get_duration();
                    clip_1.set_playback_rate(global_time, length / target_length);
                }

                {
                    let clip_2 = &mut self.clip_nodes[clip_2].clip;
                    let length = clip_2.get_duration();
                    clip_2.set_playback_rate(global_time, length / target_length);
                }
            }
        }
    }

    fn add_node(
        &mut self,
        def: BlendTreeNodeDef,
        animations: &HashMap<ClipId, Rc<AnimationClip<T>>>,
        skeleton: &Skeleton
    ) -> AnimNodeHandle {
        match def {
            BlendTreeNodeDef::LerpNode(input_1, input_2, param_id) => {
                let input_1_handle = self.add_node(*input_1, animations, skeleton);
                let input_2_handle = self.add_node(*input_2, animations, skeleton);
                self.lerp_nodes.push(LerpAnimNode {
                    input_1: input_1_handle,
                    input_2: input_2_handle,
                    blend_param: param_id.clone()
                });
                AnimNodeHandle::LerpAnimNodeHandle(self.lerp_nodes.len() - 1)
            }
            BlendTreeNodeDef::AdditiveNode(input_1, input_2, param_id) => {
                let input_1_handle = self.add_node(*input_1, animations, skeleton);
                let input_2_handle = self.add_node(*input_2, animations, skeleton);
                self.additive_nodes.push(AdditiveAnimNode {
                    base_input: input_1_handle,
                    additive_input: input_2_handle,
                    blend_param: param_id.clone()
                });
                AnimNodeHandle::AdditiveAnimNodeHandle(self.additive_nodes.len() - 1)
            }
            BlendTreeNodeDef::IKNode(input, effector_name, blend_param, target_x_param, target_y_param, target_z_param, bend_x_param, bend_y_param, bend_z_param) => {
                let input_handle = self.add_node(*input, animations, skeleton);
                self.ik_nodes.push(IKNode {
                    input: input_handle,
                    blend_param: blend_param.clone(),
                    target_x_param: target_x_param.clone(),
                    target_y_param: target_y_param.clone(),
                    target_z_param: target_z_param.clone(),
                    bend_x_param: bend_x_param.clone(),
                    bend_y_param: bend_y_param.clone(),
                    bend_z_param: bend_z_param.clone(),
                    effector_bone_index: skeleton.get_joint_index(&effector_name).unwrap(),

                });
                AnimNodeHandle::IKAnimNodeHandle(self.ik_nodes.len() - 1)
            }
            BlendTreeNodeDef::ClipNode(clip_id) => {
                let clip = animations.get(&clip_id[..]).expect(&format!("Missing animation clip: {}", clip_id)[..]);
                self.clip_nodes.push(ClipAnimNode {
                    clip: ClipInstance::new(clip.clone())
                });
                AnimNodeHandle::ClipAnimNodeHandle(self.clip_nodes.len() - 1)
            }
        }
    }

    fn get_node(&self, handle: AnimNodeHandle) -> Option<&AnimNode<T>> {
        match handle {
            AnimNodeHandle::LerpAnimNodeHandle(i) => Some(&self.lerp_nodes[i]),
            AnimNodeHandle::AdditiveAnimNodeHandle(i) => Some(&self.additive_nodes[i]),
            AnimNodeHandle::ClipAnimNodeHandle(i) => Some(&self.clip_nodes[i]),
            AnimNodeHandle::IKAnimNodeHandle(i) => Some(&self.ik_nodes[i]),
            AnimNodeHandle::None => None,
        }
    }
}

pub trait AnimNode<T: Transform> {
    fn get_output_pose(&self, tree: &AnimBlendTree<T>, time: f32, params: &HashMap<String, f32>, output_poses: &mut [T]);
}

#[derive(Clone)]
pub enum AnimNodeHandle {
    None,
    LerpAnimNodeHandle(usize),
    AdditiveAnimNodeHandle(usize),
    ClipAnimNodeHandle(usize),
    IKAnimNodeHandle(usize),
}

/// An AnimNode where pose output is linear blend between the output of the two input AnimNodes,
/// with blend factor according the blend_param value
pub struct LerpAnimNode {
    input_1: AnimNodeHandle,
    input_2: AnimNodeHandle,
    blend_param: ParamId
}

impl<T: Transform> AnimNode<T> for LerpAnimNode {
    fn get_output_pose(&self, tree: &AnimBlendTree<T>, time: f32, params: &HashMap<String, f32>, output_poses: &mut [T]) {

        let mut input_poses = [ T::identity(); 64 ];
        let sample_count = output_poses.len();

        let blend_parameter = params[&self.blend_param[..]];

        if let Some(ref node) = tree.get_node(self.input_1.clone()) {
            node.get_output_pose(tree, time, params, &mut input_poses[0 .. sample_count]);
        }

        if let Some(ref node) = tree.get_node(self.input_2.clone()) {
            node.get_output_pose(tree, time, params, output_poses);
        }

        for i in 0 .. output_poses.len() {
            let pose_1 = input_poses[i];
            let pose_2 = &mut output_poses[i];
            (*pose_2) = pose_1.lerp(pose_2.clone(), blend_parameter);
        }
    }
}

/// An AnimNode where pose output is additive blend with output of additive_input
/// added to base_input,  with blend factor according to value of blend_param
pub struct AdditiveAnimNode {
    base_input: AnimNodeHandle,
    additive_input: AnimNodeHandle,
    blend_param: ParamId
}

impl<T: Transform> AnimNode<T> for AdditiveAnimNode {
    fn get_output_pose(&self, tree: &AnimBlendTree<T>, time: f32, params: &HashMap<String, f32>, output_poses: &mut [T]) {

        let mut input_poses = [ T::identity(); 64 ];
        let sample_count = output_poses.len();

        let blend_parameter = params[&self.blend_param[..]];

        if let Some(ref node) = tree.get_node(self.base_input.clone()) {
            node.get_output_pose(tree, time, params, &mut input_poses[0 .. sample_count]);
        }

        if let Some(ref node) = tree.get_node(self.additive_input.clone()) {
            node.get_output_pose(tree, time, params, output_poses);
        }

        for i in 0 .. output_poses.len() {
            let pose_1 = input_poses[i];
            let pose_2 = &mut output_poses[i];
            let additive_pose = T::identity().lerp(pose_2.clone(), blend_parameter);
            (*pose_2) = pose_1.concat(additive_pose);
        }
    }
}

/// An AnimNode where pose output is from an animation ClipInstance
pub struct ClipAnimNode<T: Transform> {
    clip: ClipInstance<T>
}

impl<T: Transform> AnimNode<T> for ClipAnimNode<T> {
    fn get_output_pose(&self, _tree: &AnimBlendTree<T>, time: f32, _params: &HashMap<String, f32>, output_poses: &mut [T]) {
        self.clip.get_pose_at_time(time, output_poses);
    }
}

pub struct IKNode {
    input: AnimNodeHandle,
    blend_param: ParamId,
    target_x_param: ParamId,
    target_y_param: ParamId,
    target_z_param: ParamId,
    bend_x_param: ParamId,
    bend_y_param: ParamId,
    bend_z_param: ParamId,
    effector_bone_index: JointIndex,
}

impl<T: Transform> AnimNode<T> for IKNode {
    fn get_output_pose(&self, tree: &AnimBlendTree<T>, time: f32, params: &HashMap<String, f32>, output_poses: &mut [T]) {

        // Get input pose
        if let Some(ref node) = tree.get_node(self.input.clone()) {
            node.get_output_pose(tree, time, params, output_poses);
        }

        // Target position should be in model-space
        let effector_target_position = [params[&self.target_x_param[..]],
                                        params[&self.target_y_param[..]],
                                        params[&self.target_z_param[..]]];


        let effector_bone_index = self.effector_bone_index;
        let middle_bone_index = tree.skeleton.joints[effector_bone_index as usize].parent_index;
        let root_bone_index = tree.skeleton.joints[middle_bone_index as usize].parent_index;
        let root_bone_parent_index = tree.skeleton.joints[root_bone_index as usize].parent_index;

        // Get bone positions in model-space by calculating global poses
        let mut global_poses = [ Matrix4::<f32>::identity(); 64 ];
        tree.skeleton.calculate_global_poses(output_poses, &mut global_poses);

        let root_bone_position = global_poses[root_bone_index as usize].transform_vector([0.0, 0.0, 0.0]);
        let middle_bone_position = global_poses[middle_bone_index as usize].transform_vector([0.0, 0.0, 0.0]);
        let effector_bone_position = global_poses[effector_bone_index as usize].transform_vector([0.0, 0.0, 0.0]);

        let length_1 = vec3_len(vec3_sub(root_bone_position, middle_bone_position));
        let length_2 = vec3_len(vec3_sub(middle_bone_position, effector_bone_position));

        // get effector target position on a 2D bend plane,
        // with coordinates relative to root bone position

        // x axis of bend plane
        let root_to_effector = vec3_normalized(vec3_sub(effector_target_position, root_bone_position));

        // z axis of bend plane
        let plane_normal = {
            let bend_direction = [params[&self.bend_x_param[..]],
                                  params[&self.bend_y_param[..]],
                                  params[&self.bend_z_param[..]]];
            if vec3_len(bend_direction) == 0.0 {
                // Choose a somewhat arbitary bend normal:
                vec3_normalized(vec3_cross(vec3_sub(middle_bone_position, root_bone_position),
                                           root_to_effector))
            } else {
                // Use desired bend direction:
                let desired_bend_direction = vec3_normalized(bend_direction);
                vec3_normalized(vec3_cross(desired_bend_direction,
                                           root_to_effector))
            }
        };

        // y axis of bend plane
        let plane_y_direction = vec3_normalized(vec3_cross(root_to_effector, plane_normal));

        let plane_rotation = [
            root_to_effector,
            plane_y_direction,
            plane_normal
        ];

        // Convert to 2D coords on the plane, where parent root is at (0,0) and target is at (x,y)
        let plane_target = row_mat3_transform(plane_rotation,
                                              vec3_sub(effector_target_position,
                                                       root_bone_position));

        if let Some(elbow_target) = solve_ik_2d(length_1, length_2, [plane_target[0], plane_target[1]]) {

            // Copy input poses into IK target poses
            let mut target_poses = [ T::identity(); 64 ];
            for i in 0 .. 64 {
                target_poses[i] = output_poses[i];
            }

            let middle_bone_plane = [elbow_target[0], elbow_target[1], 0.0];

            let middle_bone_target = vec3_add(row_mat3_transform(mat3_inv(plane_rotation),
                                                                 middle_bone_plane),
                                              root_bone_position);

            // calculate root bone pose
            {
                let original_direction = vec3_normalized(vec3_sub(middle_bone_position, root_bone_position));
                let target_direction = vec3_normalized(vec3_sub(middle_bone_target, root_bone_position));
                let rotation_change = quaternion::rotation_from_to(target_direction, original_direction);
                let original_rotation = global_poses[root_bone_index as usize].get_rotation();
                let new_rotation = quaternion::mul(original_rotation, rotation_change);

                global_poses[root_bone_index as usize].set_rotation(new_rotation);

                let local_pose = row_mat4_mul(mat4_inv(global_poses[root_bone_parent_index as usize]), global_poses[root_bone_index as usize]);
                target_poses[root_bone_index as usize] = T::from_matrix(local_pose);
            }

            // calculate middle bone pose
            {
                let original_direction = vec3_normalized(vec3_sub(effector_bone_position, middle_bone_position));
                let target_direction = vec3_normalized(vec3_sub(effector_target_position, middle_bone_target));
                let rotation_change = quaternion::rotation_from_to(target_direction, original_direction);
                let original_rotation = global_poses[middle_bone_index as usize].get_rotation();
                let new_rotation = quaternion::mul(original_rotation, rotation_change);

                global_poses[middle_bone_index as usize].set_rotation(new_rotation);
                global_poses[middle_bone_index as usize].set_translation(middle_bone_target);

                let local_pose = row_mat4_mul(mat4_inv(global_poses[root_bone_index as usize]), global_poses[middle_bone_index as usize]);
                target_poses[middle_bone_index as usize] = T::from_matrix(local_pose);
            }

            // Blend between input and IK target poses

            let blend_parameter = params[&self.blend_param[..]];
            for i in 0 .. output_poses.len() {
                let ik_pose = target_poses[i];
                let output_pose = &mut output_poses[i];
                (*output_pose) = output_pose.lerp(ik_pose.clone(), blend_parameter);
            }
        }
    }
}
