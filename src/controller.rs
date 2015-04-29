use std::collections::HashMap;
use std::rc::Rc;

use rustc_serialize::{Decodable, Decoder};

use animation::AnimationClip;
use transform::{Transform, FromTransform};
use blend_tree::{BlendTreeNode, BlendTreeNodeDef, ClipId};
use skeleton::Skeleton;

const MAX_JOINTS: usize = 64;

/// A state that an AnimationController can be in, consisting
/// of a blend tree and a collection of transitions to other states
pub struct AnimationState<T: Transform> {

    /// The blend tree used to determine the final blended pose
    /// for this state
    pub blend_tree: BlendTreeNode<T>,

    /// Transitions from this state to other AnimationStates
    pub transitions: Vec<AnimationTransition>,
}

/// Representation of a state transition to a target state, with a condition and a duration
#[derive(Debug, Clone, RustcDecodable)]
pub struct AnimationTransition {
    /// The name of the target state to transition to
    pub target_state: String,

    /// The condition that will be checked in order to determine
    /// if the controller should transition to the target state
    pub condition: TransitionCondition,

    /// The duration of the transition, during which a linear blend
    /// transition between the current and target states should occur
    pub duration: f32,
}

/// Representation of a condition to check for an AnimationTransition
#[derive(Debug, Clone, RustcDecodable)]
pub struct TransitionCondition {

    /// The name of the controller parameter to compare with
    pub parameter: String,

    /// The comparision operator to use
    pub operator: Operator,

    /// The constant value to compare with the controller parameter value
    pub value: f32,
}

impl TransitionCondition {
    /// Returns true if the condition is satisfied
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

/// Definition struct for an AnimationController, which can be deserialized from JSON
/// and converted to an AnimationController instance at runtime
#[derive(Clone, Debug, RustcDecodable)]
pub struct AnimationControllerDef {

    /// Identifying name for the controller definition
    pub name: String,

    /// Declaration list of all parameters that are used by the AnimationController,
    /// including state transition conditions and blend tree parameters
    pub parameters: Vec<String>,

    /// List of animation state definitions
    pub states: Vec<AnimationStateDef>,

    /// The name of the state that the AnimationController should start in
    pub initial_state: String,
}

/// Definition struct for an AnimationState, which can be deserialized from JSON
/// and converted to an AnimationState instance at runtime
#[derive(Clone, Debug)]
pub struct AnimationStateDef {

    /// The identifying name for the state
    pub name: String,

    /// The blend tree definition for this state
    pub blend_tree: BlendTreeNodeDef,

    /// The transitions to other states that can occur from this state
    pub transitions: Vec<AnimationTransition>,
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


/// A runtime representation of an Animation State Machine, consisting of one or more
/// AnimationStates connected by AnimationTransitions, where the output animation
/// pose depends on the current state or any active transitions between states.
pub struct AnimationController<T: Transform> {

    /// Parameters that will be referenced by blend tree nodes and animation states
    parameters: HashMap<String, f32>,

    /// Shared reference to the skeleton this controller is using
    skeleton: Rc<Skeleton>,

    /// Tracks seconds since controller started running
    local_clock: f64,

    /// Playback speed multiplier.
    playback_speed: f64,

    /// Mapping of all animation state names to their instances
    states: HashMap<String, AnimationState<T>>,

    /// The name of the current active AnimationState
    current_state: String,

    /// The current active AnimationTransition and its start time, if any
    transition: Option<(f64, AnimationTransition)>,
}



impl<T: Transform> AnimationController<T> {

    /// Create an AnimationController instance from its definition, the desired skeleton, and a
    /// collection of currently loaded animation clips.
    pub fn new(controller_def: AnimationControllerDef, skeleton: Rc<Skeleton>, animations: &HashMap<ClipId, Rc<AnimationClip<T>>>) -> AnimationController<T> {

        let mut parameters = HashMap::new();

        for parameter in controller_def.parameters.iter() {
            parameters.insert(parameter.clone(), 0.0);
        };

        let mut states = HashMap::new();
        for state_def in controller_def.states.iter() {

            let mut blend_tree = BlendTreeNode::from_def(state_def.blend_tree.clone(), animations);
            blend_tree.synchronize_subtree(0.0, &parameters);

            states.insert(state_def.name.clone(), AnimationState {
                blend_tree: blend_tree,
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

    /// Update the controller's local clock with the given time delta
    pub fn update(&mut self, delta_time: f64) {
        self.local_clock += delta_time * self.playback_speed;
    }

    /// Checks if controller should transition to a different state, or if currently
    /// in a transition, checks if the transition is complete
    fn update_state(&mut self, ext_dt: f64) {
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

    /// Set the playback speed for the controller
    pub fn set_playback_speed(&mut self, speed: f64) {
        self.playback_speed = speed;
    }

    /// Set the value for the given controller parameter
    pub fn set_param_value(&mut self, name: &str, value: f32) {
        self.parameters.insert(name.to_string(), value); // :(
    }

    /// Return the value for the given controller parameter
    pub fn get_param_value(&self, name: &str) -> f32 {
        self.parameters[name]
    }

    /// Return a read-only reference to the controller parameter map
    pub fn get_parameters(&self) -> &HashMap<String, f32> {
        &self.parameters
    }

    /// Calculate global skeletal joint poses for the given time since last update
    pub fn get_output_pose<TOutput: Transform + FromTransform<T>>(&mut self, ext_dt: f64, output_poses: &mut [TOutput]) {

        self.update_state(ext_dt);

        let elapsed_time = self.local_clock + ext_dt * self.playback_speed;

        let mut local_poses = [ T::identity(); MAX_JOINTS ];

        {
            let current_state = self.states.get_mut(&self.current_state[..]).unwrap();
            current_state.blend_tree.get_output_pose(elapsed_time as f32, &self.parameters, &mut local_poses[..]);
        }

        // TODO - would be kinda cool if you could just use a lerp node that pointed to the two
        // blend trees, but then we'd need RC pointers?

        if let Some((transition_start_time, ref transition)) = self.transition {

            // Blend with the target state ...

            let mut target_poses = [ T::identity(); MAX_JOINTS ];

            let target_state = self.states.get_mut(&transition.target_state[..]).unwrap();

            target_state.blend_tree.get_output_pose(elapsed_time as f32, &self.parameters, &mut target_poses[..]);

            let blend_parameter = ((self.local_clock + ext_dt - transition_start_time) / transition.duration as f64) as f32;

            for i in (0 .. output_poses.len()) {
                let pose_1 = &mut local_poses[i];
                let pose_2 = target_poses[i];
                *pose_1 = pose_1.lerp(pose_2, blend_parameter);
            }

        }

        self.calculate_global_poses(&local_poses[..], output_poses);
    }

    /// Calculate global poses from the controller's skeleton and the given local poses
    fn calculate_global_poses<TOutput: Transform + FromTransform<T>>(
        &self,
        local_poses: &[T],
        global_poses: &mut [TOutput],
    ) {

        for (joint_index, joint) in self.skeleton.joints.iter().enumerate() {

            let parent_pose = if !joint.is_root() {
                global_poses[joint.parent_index as usize]
            } else {
                TOutput::identity()
            };

            let local_pose = local_poses[joint_index];
            global_poses[joint_index] = parent_pose.concat(TOutput::from_transform(local_pose));
        }
    }
}
