use std::collections::HashMap;
use std::rc::Rc;

use rustc_serialize::{Decodable, Decoder};

use animation::{AnimationClip, Transform};

/// Identifier for an AnimationClip within a BlendTreeNodeDef
pub type ClipId = String;

/// Identifier for animation controller parameter, within a LerpNode
pub type ParamId = String;

/// Definition of a blend tree, to be converted to BlendTreeNode when used by an
/// AnimationController
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

/// Runtime representation of a blend tree.
pub enum BlendTreeNode {

    /// Pose output is linear blend between the output of
    /// two child BlendTreeNodes, with blend factor according
    /// the paramater value for name ParamId
    LerpNode(Box<BlendTreeNode>, Box<BlendTreeNode>, ParamId),

    /// Pose output is additive blend between the output of
    /// two child BlendTreeNodes, with blend factor according
    /// the paramater value for name ParamId
    AdditiveNode(Box<BlendTreeNode>, Box<BlendTreeNode>, ParamId),

    /// Pose output is from an AnimationClip
    ClipNode(Rc<AnimationClip>),
}

impl BlendTreeNode {

    /// Initialize a new BlendTreeNode from a BlendTreeNodeDef and
    /// a mapping from animation names to AnimationClip
    ///
    /// # Arguments
    ///
    /// * `def` - The root BlendTreeNodeDef
    /// * `animations` - A mapping from ClipIds to shared AnimationClip instances
    pub fn from_def(
        def: BlendTreeNodeDef,
        animations: &HashMap<ClipId, Rc<AnimationClip>>
    ) -> BlendTreeNode {

        match def {

            BlendTreeNodeDef::LerpNode(input_1, input_2, param_id) => {
                BlendTreeNode::LerpNode(
                    Box::new(BlendTreeNode::from_def(*input_1, animations)),
                    Box::new(BlendTreeNode::from_def(*input_2, animations)),
                    param_id.clone()
                )
            }

            BlendTreeNodeDef::AdditiveNode(input_1, input_2, param_id) => {
                BlendTreeNode::AdditiveNode(
                    Box::new(BlendTreeNode::from_def(*input_1, animations)),
                    Box::new(BlendTreeNode::from_def(*input_2, animations)),
                    param_id.clone()
                )
            }

            BlendTreeNodeDef::ClipNode(clip_id) => {
                let clip = animations.get(&clip_id[..]).expect(&format!("Missing animation clip: {}", clip_id)[..]);
                BlendTreeNode::ClipNode(clip.clone())
            }
        }
    }

    /// Get the output skeletal pose for this node and the given time and parameters
    ///
    /// # Arguments
    ///
    /// * `time` - The time to sample from any AnimationClips
    /// * `params` - A mapping from ParamIds to their current parameter values
    /// * `output_poses` - The output array slice of joint transforms that will be populated
    ///                    according to the defined output for this BlendTreeNode
    pub fn get_output_pose(&self, time: f32, params: &HashMap<String, f32>, output_poses: &mut [Transform]) {
        match self {
            &BlendTreeNode::LerpNode(ref input_1, ref input_2, ref param_name) => {

                let mut input_poses = [ Transform { translation: [0.0, 0.0, 0.0], scale: 0.0, rotation: (0.0, [0.0, 0.0, 0.0]) }; 64 ];

                let sample_count = output_poses.len();

                input_1.get_output_pose(time, params, &mut input_poses[0 .. sample_count]);
                input_2.get_output_pose(time, params, output_poses);

                let blend_parameter = params[&param_name[..]];

                for i in (0 .. output_poses.len()) {
                    let pose_1 = input_poses[i];
                    let pose_2 = &mut output_poses[i];
                    (*pose_2) = pose_1.lerp(pose_2.clone(), blend_parameter);
                }

            }
            &BlendTreeNode::AdditiveNode(ref input_1, ref input_2, ref param_name) => {

                let mut input_poses = [ Transform { translation: [0.0, 0.0, 0.0], scale: 0.0, rotation: (0.0, [0.0, 0.0, 0.0]) }; 64 ];

                let sample_count = output_poses.len();

                input_1.get_output_pose(time, params, &mut input_poses[0 .. sample_count]);
                input_2.get_output_pose(time, params, output_poses);

                let blend_parameter = params[&param_name[..]];

                for i in (0 .. output_poses.len()) {
                    let pose_1 = input_poses[i];
                    let pose_2 = &mut output_poses[i];
                    let additive_pose = Transform::identity().lerp(pose_2.clone(), blend_parameter);
                    (*pose_2) = pose_1.add(additive_pose);
                }

            }
            &BlendTreeNode::ClipNode(ref clip) => {
                clip.get_pose_at_time(time, output_poses);
            }
        }
    }
}
