use std::collections::HashMap;
use std::rc::Rc;

use rustc_serialize::{Decodable, Decoder};

use animation::{AnimationClip, ClipInstance};

use transform::Transform;

/// Identifier for an AnimationClip within a BlendTreeNodeDef
pub type ClipId = String;

/// Identifier for animation controller parameter, within a LerpNode
pub type ParamId = String;

/// Definition of a blend tree, used by AnimationController to construct an AnimBlendTree
#[derive(Debug, Clone)]
pub enum BlendTreeNodeDef {
    LerpNode(Box<BlendTreeNodeDef>, Box<BlendTreeNodeDef>, ParamId),
    AdditiveNode(Box<BlendTreeNodeDef>, Box<BlendTreeNodeDef>, ParamId),
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
    clip_nodes: Vec<ClipAnimNode<T>>,
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
        animations: &HashMap<ClipId, Rc<AnimationClip<T>>>
    ) -> AnimBlendTree<T> {

        let mut tree = AnimBlendTree {
            root_node: AnimNodeHandle::None,
            lerp_nodes: Vec::new(),
            additive_nodes: Vec::new(),
            clip_nodes: Vec::new(),
        };

        tree.root_node = tree.add_node(def, animations);
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
        animations: &HashMap<ClipId, Rc<AnimationClip<T>>>
    ) -> AnimNodeHandle {
        match def {
            BlendTreeNodeDef::LerpNode(input_1, input_2, param_id) => {
                let input_1_handle = self.add_node(*input_1, animations);
                let input_2_handle = self.add_node(*input_2, animations);
                self.lerp_nodes.push(LerpAnimNode {
                    input_1: input_1_handle,
                    input_2: input_2_handle,
                    blend_param: param_id.clone()
                });
                AnimNodeHandle::LerpAnimNodeHandle(self.lerp_nodes.len() - 1)
            }
            BlendTreeNodeDef::AdditiveNode(input_1, input_2, param_id) => {
                let input_1_handle = self.add_node(*input_1, animations);
                let input_2_handle = self.add_node(*input_2, animations);
                self.additive_nodes.push(AdditiveAnimNode {
                    base_input: input_1_handle,
                    additive_input: input_2_handle,
                    blend_param: param_id.clone()
                });
                AnimNodeHandle::AdditiveAnimNodeHandle(self.additive_nodes.len() - 1)
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

        for i in (0 .. output_poses.len()) {
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

        for i in (0 .. output_poses.len()) {
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

