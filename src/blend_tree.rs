use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;

use interpolation;
use rustc_serialize::{self, Decodable, Decoder, json};

use animation::{AnimationClip, SQT};
use math::lerp_quaternion;

pub type ClipId = String;
pub type ParamId = String;

pub enum BlendTreeNode {
    LerpNode(Box<BlendTreeNode>, Box<BlendTreeNode>, ParamId),
    ClipNode(Rc<RefCell<AnimationClip>>),
}


#[derive(Clone)]
pub enum BlendTreeNodeDef {
    LerpNode(Box<BlendTreeNodeDef>, Box<BlendTreeNodeDef>, ParamId),
    ClipNode(ClipId),
}

impl BlendTreeNodeDef {
    pub fn from_path(path: &str) -> Result<BlendTreeNodeDef, &'static str> {
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

        Ok(json::decode(&json_string[..]).unwrap())
    }
}

impl BlendTreeNodeDef {

    pub fn get_parameters(&self) -> Vec<String> {
        let mut parameters = Vec::new();
        self.get_parameters_internal(&mut parameters);
        parameters
    }

    fn get_parameters_internal(&self, parameters: &mut Vec<String>) {
        match self {
            &BlendTreeNodeDef::LerpNode(ref input_1, ref input_2, ref param_name) => {

                if !parameters.iter().any(|p| p == param_name) {
                    parameters.push(param_name.clone());
                }
                input_1.get_parameters_internal(parameters);
                input_2.get_parameters_internal(parameters);
            },
            &BlendTreeNodeDef::ClipNode(_) => {}
        }
    }
}

impl Decodable for BlendTreeNodeDef {

    fn decode<D: Decoder>(decoder: &mut D) -> Result<BlendTreeNodeDef, D::Error> {
        decoder.read_struct("root", 0, |decoder| {

            let node_type = try!(decoder.read_struct_field("type", 0, |decoder| { Ok(try!(decoder.read_str())) }));

            match &node_type[..] {
                "LerpNode" => {

                    let (input_1, input_2) = try!(decoder.read_struct_field("inputs", 0, |decoder| {
                        decoder.read_seq(|decoder, len| {
                            Ok((
                                try!(decoder.read_seq_elt(0, Decodable::decode)),
                                try!(decoder.read_seq_elt(1, Decodable::decode))
                            ))
                        })
                    }));

                    let blend_param_name = try!(decoder.read_struct_field("param", 0, |decoder| { Ok(try!(decoder.read_str())) }));

                    Ok(BlendTreeNodeDef::LerpNode(Box::new(input_1), Box::new(input_2), blend_param_name))

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

impl BlendTreeNode {

    ///
    /// Initialize a new BlendTreeNode from a BlendTreeNodeDef and
    /// a mapping from animation names to AnimationClip
    ///
    pub fn from_def(
        def: BlendTreeNodeDef,
        animations: &HashMap<ClipId, Rc<RefCell<AnimationClip>>>
    ) -> BlendTreeNode {

        match def {

            BlendTreeNodeDef::LerpNode(input_1, input_2, param_id) => {
                BlendTreeNode::LerpNode(
                    Box::new(BlendTreeNode::from_def(*input_1, animations)),
                    Box::new(BlendTreeNode::from_def(*input_2, animations)),
                    param_id.clone()
                )
            }

            BlendTreeNodeDef::ClipNode(clip_id) => {
                let clip = &animations[&clip_id[..]];
                BlendTreeNode::ClipNode(clip.clone())
            }
        }
    }

    ///
    /// Get the output skeletal pose for this node and the given time and parameters
    ///
    pub fn get_output_pose(&self, elapsed_time: f32, params: &HashMap<String, f32>, output_poses: &mut [SQT]) {
        match self {
            &BlendTreeNode::LerpNode(ref input_1, ref input_2, ref param_name) => {

                let mut input_poses = [ SQT { translation: [0.0, 0.0, 0.0], scale: 0.0, rotation: (0.0, [0.0, 0.0, 0.0]) }; 64 ];

                let sample_count = output_poses.len();

                input_1.get_output_pose(elapsed_time, params, &mut input_poses[0 .. sample_count]);
                input_2.get_output_pose(elapsed_time, params, output_poses);

                let blend_parameter = params[&param_name[..]];

                for i in (0 .. output_poses.len()) {
                    let pose_1 = input_poses[i];
                    let pose_2 = &mut output_poses[i];
                    pose_2.scale = interpolation::lerp(&pose_1.scale, &pose_2.scale, &blend_parameter);
                    pose_2.translation = interpolation::lerp(&pose_1.translation, &pose_2.translation, &blend_parameter);
                    pose_2.rotation = lerp_quaternion(&pose_1.rotation, &pose_2.rotation, &blend_parameter);
                }

            }
            &BlendTreeNode::ClipNode(ref clip) => {
                clip.borrow().get_interpolated_poses_at_time(elapsed_time, output_poses);
            }
        }
    }
}
