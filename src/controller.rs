use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rustc_serialize::{self, Decodable, Decoder, json};
use interpolation;

use animation::{SQT, AnimationClip};
use blend_tree::{BlendTreeNode, BlendTreeNodeDef, ClipId};
use math::*;
use skeleton::Skeleton;

const MAX_PARAMS: usize = 16;
const MAX_JOINTS: usize = 64;

pub struct AnimationState {
    blend_tree: BlendTreeNode,
    transitions: Vec<AnimationTransition>,
}

#[derive(Debug, Clone, RustcDecodable)]
pub struct AnimationTransition {
    target_state: String,
    condition: TransitionCondition,
    duration: f32,
}

#[derive(Debug, Clone, RustcDecodable)]
pub struct TransitionCondition {
    parameter: String,
    operator: Operator,
    value: f32,
}

impl TransitionCondition {
    ///
    /// Returns true if the condition is satisfied
    ///
    pub fn is_true(&self, parameters: &HashMap<String, f32>) -> bool {
        match self.operator {
            Operator::LessThan => parameters[&self.parameter[..]] < self.value,
            Operator::GreaterThan => parameters[&self.parameter[..]] > self.value,
            Operator::LessThanEqual => parameters[&self.parameter[..]] <= self.value,
            Operator::GreaterThanEqual => parameters[&self.parameter[..]] >= self.value,
            Operator::Equal => parameters[&self.parameter[..]] == self.value,
            Operator::NotEqual => parameters[&self.parameter[..]] != self.value,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Operator {
    LessThan,
    LessThanEqual,
    GreaterThan,
    GreaterThanEqual,
    Equal,
    NotEqual,
}

impl Decodable for Operator {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Operator, D::Error> {

        match &try!(decoder.read_str())[..] {
            "<" => Ok(Operator::LessThan),
            ">" => Ok(Operator::GreaterThan),
            "<=" => Ok(Operator::LessThanEqual),
            ">=" => Ok(Operator::GreaterThanEqual),
            "=" => Ok(Operator::Equal),
            "!=" => Ok(Operator::NotEqual),
            _ => Ok(Operator::Equal), // FIXME -- figure out how to throw a D::Error...
        }
    }
}


pub struct AnimationControllerDef {
    parameters: Vec<String>,
    states: Vec<AnimationStateDef>,
    initial_state: String,
}

impl Decodable for AnimationControllerDef {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<AnimationControllerDef, D::Error> {
        decoder.read_struct("root", 0, |decoder| {

            let params = try!(decoder.read_struct_field("parameters", 0, |decoder| {
                decoder.read_seq(|decoder, len| {
                    let mut params = Vec::new();
                    for i in (0 .. len) {
                        params.push(try!(decoder.read_seq_elt(i, Decodable::decode)));
                    }
                    Ok(params)
                })
            }));

            let states = try!(decoder.read_struct_field("states", 0, |decoder| {
                decoder.read_seq(|decoder, len| {
                    let mut states = Vec::new();
                    for i in (0 .. len) {
                        states.push(try!(decoder.read_seq_elt(i, Decodable::decode)));
                    }
                    Ok(states)
                })
            }));

            let initial_state = try!(decoder.read_struct_field("initial_state", 0, |decoder| {
                Ok(try!(decoder.read_str()))
            }));

            Ok(AnimationControllerDef{
                parameters: params,
                states: states,
                initial_state: initial_state,
            })
        })
    }
}

pub struct AnimationStateDef {
    name: String,
    blend_tree: BlendTreeNodeDef,
    transitions: Vec<AnimationTransition>,
}

impl Decodable for AnimationStateDef {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<AnimationStateDef, D::Error> {
        decoder.read_struct("root", 0, |decoder| {

            let name = try!(decoder.read_struct_field("name", 0, |decoder| {
                Ok(try!(decoder.read_str()))
            }));

            let blend_tree = try!(decoder.read_struct_field("blend_tree", 0, Decodable::decode));

            let transitions = try!(decoder.read_struct_field("transitions", 0, |decoder| {
                decoder.read_seq(|decoder, len| {
                    let mut transitions = Vec::new();
                    for i in (0 .. len) {
                        transitions.push(try!(decoder.read_seq_elt(i, Decodable::decode)));
                    }
                    Ok(transitions)
                })
            }));

            Ok(AnimationStateDef {
                name: name,
                blend_tree: blend_tree,
                transitions: transitions,
            })
        })
    }
}


pub struct AnimationController {

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

    ///
    /// Tracks seconds since controller started running
    ///
    local_clock: f64,

    ///
    /// Playback speed multiplier.
    ///
    playback_speed: f64,


    states: HashMap<String, AnimationState>,
    current_state: String,
    transition: Option<(f64, AnimationTransition)>, // Transition with local clock time of transition start

}



impl AnimationController {

    pub fn new(controller_def: AnimationControllerDef, skeleton: Rc<RefCell<Skeleton>>, animations: &HashMap<ClipId, Rc<RefCell<AnimationClip>>>) -> AnimationController {

        let mut parameters = HashMap::new();

        for parameter in controller_def.parameters.iter() {
            parameters.insert(parameter.clone(), 0.0);
        };

        let mut states = HashMap::new();
        for state_def in controller_def.states.iter() {
            states.insert(state_def.name.clone(), AnimationState {
                blend_tree: BlendTreeNode::from_def(state_def.blend_tree.clone(), animations),
                transitions: state_def.transitions.clone()
            });
        }

        AnimationController {
            parameters: parameters,
            skeleton: skeleton.clone(),
            local_clock: 0.0,
            playback_speed: 1.0,
            states: states,
            current_state: controller_def.initial_state,
            transition: None,
        }
    }

    pub fn update(&mut self, delta_time: f64) {
        self.local_clock += delta_time * self.playback_speed;
    }

    pub fn update_state(&mut self, ext_dt: f64) {

        match self.transition.clone() {

            Some((ref start_time, ref transition)) => {

                // If transition is finished, switch state to new transition
                if self.local_clock + ext_dt >= start_time + transition.duration as f64{
                    self.current_state = transition.target_state.clone();
                    self.transition = None;
                }

            },
            None => {

                // Check for any transitions with passing conditions
                let current_state = &self.states[&self.current_state[..]];
                for transition in current_state.transitions.iter() {

                    if transition.condition.is_true(&self.parameters) {
                        self.transition = Some((self.local_clock + ext_dt, transition.clone()));
                        break;
                    }

                }

            }
        }

    }

    ///
    /// Set the value for the given parameter
    ///
    pub fn set_param_value(&mut self, name: &str, value: f32) {
        self.parameters.insert(name.to_string(), value); // :(
    }

    pub fn set_playback_speed(&mut self, speed: f64) {
        self.playback_speed = speed;
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
    /// Calculate GLOBAL skeletal joint poses for the given time since last update
    ///
    pub fn get_output_pose(&mut self, ext_dt: f64, output_poses: &mut [Matrix4<f32>]) {

        self.update_state(ext_dt);

        let elapsed_time = self.local_clock + ext_dt * self.playback_speed;

        let current_state = &self.states[&self.current_state[..]];

        let mut local_poses = [ SQT {
            translation: [0.0, 0.0, 0.0],
            scale: 0.0,
            rotation: (0.0, [0.0, 0.0, 0.0])
        }; MAX_JOINTS ];

        current_state.blend_tree.get_output_pose(elapsed_time as f32, &self.parameters, &mut local_poses[..]);

        // TODO - would be kinda cool if you could just use a lerp node that pointed to the two
        // blend trees, but then we'd need RC pointers?

        if let Some((transition_start_time, ref transition)) = self.transition {

            // Blend with the target state ...

            let mut target_poses = [ SQT {
                translation: [0.0, 0.0, 0.0],
                scale: 0.0,
                rotation: (0.0, [0.0, 0.0, 0.0])
            }; MAX_JOINTS ];

            let target_state = &self.states[&transition.target_state[..]];

            target_state.blend_tree.get_output_pose(elapsed_time as f32, &self.parameters, &mut target_poses[..]);

            let blend_parameter = ((self.local_clock + ext_dt - transition_start_time) / transition.duration as f64) as f32;

            for i in (0 .. output_poses.len()) {
                let pose_1 = &mut local_poses[i];
                let pose_2 = target_poses[i];
                pose_1.scale = interpolation::lerp(&pose_1.scale, &pose_2.scale, &blend_parameter);
                pose_1.translation = interpolation::lerp(&pose_1.translation, &pose_2.translation, &blend_parameter);
                pose_1.rotation = lerp_quaternion(&pose_1.rotation, &pose_2.rotation, &blend_parameter);
            }

        }

        self.calculate_global_poses(&local_poses[..], output_poses);

    }

    ///
    /// Calculate global poses from the controller's skeleton and the given local poses
    ///
    pub fn calculate_global_poses(
        &self,
        local_poses: &[SQT],
        global_poses: &mut [Matrix4<f32>],
    ) {

        for (joint_index, joint) in self.skeleton.borrow().joints.iter().enumerate() {

            let parent_pose = if !joint.is_root() {
                global_poses[joint.parent_index as usize]
            } else {
                mat4_id()
            };

            let local_pose_sqt = &local_poses[joint_index];

            let mut local_pose = quaternion_to_matrix(local_pose_sqt.rotation);

            local_pose[0][3] = local_pose_sqt.translation[0];
            local_pose[1][3] = local_pose_sqt.translation[1];
            local_pose[2][3] = local_pose_sqt.translation[2];

            global_poses[joint_index] = row_mat4_mul(parent_pose, local_pose);
        }
    }

}
