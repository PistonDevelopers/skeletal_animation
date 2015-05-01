# skeletal_animation

[![Build Status](https://travis-ci.org/PistonDevelopers/skeletal_animation.png?branch=master)](https://travis-ci.org/PistonDevelopers/skeletal_animation)

A Rust library for data-driven skeletal animation.

[Documentation](http://docs.piston.rs/skeletal_animation/skeletal_animation/)

[Example Demo](https://github.com/stjahns/skeletal_animation_demo)

## Overview

This library allows you to define animation clips, state machines, and blend trees in JSON to be loaded and reloaded at runtime without needing to recompile your Rust project. 

## Usage

### Asset Definition File

Animation assets, which currently include AnimationClips, DifferenceClips, and AnimationControllers are declared in defined in a JSON file with the following format:

```json
{
    "animation_clips": [ ... ]
    "difference_clips": [ ... ]
    "animation_controllers": [ ... ]
}
```

At runtime, assets can be loaded from one or more definition files through the `AssetManager` as follows:

```Rust
let mut asset_manager = AssetManager::<QVTransform>::new(); // To use QVTransforms (Quaternions for rotations, Vector3s for translations)
let mut asset_manager = AssetManager::<DualQuaternion<f32>>::new(); // To use DualQuaternions
asset_manager.load_animations("assets/animation_assets.json");
asset_manager.load_animations("assets/more_animation_assets.json");
```

#### Animation Clips

Animation clips are declared as follows:

```json
{
    "animation_clips": [{
        "name": "walk-forward",
        "source": "assets/walk.dae",
    }, {
        "name": "run-forward",
        "source": "assets/run.dae",
    }]
}
```

#### Difference Clips

Difference Clips are animation clips defined by the _difference_ between two animation clips. They are intended to be used by additive blend nodes,
where an additive clip (e.g. head turning to the left) is added to the output of another node (e.g. a walking animation).

```json
{
    "difference_clips": [{
        "name": "head-look-left-additive",
	    "source_clip": "head-look-left",
	    "reference_clip": "reference-pose"
    }]
}
```

where:
* `name` should be a unique identifier for the animation clip that can be referenced from the animation controller definition.
* `source_clip` is the path to a COLLADA file containing the desired animation, e.g. a character in "T-Pose" with the head turned left
* `reference_clip` is the path to a COLLADA file containing the desired reference animation, e.g. a character in "T-Pose"

#### Animation Controllers

Animation controllers are state machines, which consist of:
* A list of all parameters that will be referenced by state transition conditions and blend tree nodes within this controller.
* A list of states, where each state consists of:
	* A uniquely identifying name for the state.
	* A blend tree that blends one or more animation clips together according to some parameter values.
	* A list of transitions to other states within the same controller, where each transition has:
		* A target state name.
		* A condition based on some parameter value.
		* A duration for the transition.
* The name of the initial state the controller should start in.

An example controller definition:

```json
{
    "animation_controllers: [{
        "parameters": [
            "forward-speed",
            "forward-to-strafe",
            "walk-to-run",
            "left-to-right"
        ],

        "states": [ {
            "name": "walking-forward",
            "blend_tree": {
                "type": "LerpNode",
                "param": "forward-to-strafe",
                "inputs": [ {
                    "type": "LerpNode",
                    "param": "walk-to-run",
                    "inputs": [{
                        "type": "ClipNode",
                        "clip_source": "walk-forward"
                    }, {
                        "type": "ClipNode",
                        "clip_source": "run-forward"
                    }]

                }, {
                    "type": "LerpNode",
                    "param": "left-to-right",
                    "inputs": [{
                        "type": "ClipNode",
                        "clip_source": "walk-left",
                    }, {
                        "type": "ClipNode",
                        "clip_source": "walk-right"
                    }]
                }]
            },

            "transitions": [ {
                "target_state": "stand-idle",
                "duration": 0.5,
                "condition": {
                    "parameter": "forward-speed",
                    "operator": "<",
                    "value": 0.1
                }
            }]

        }, {
            "name": "stand-idle",
            "blend_tree": {
                "type": "ClipNode",
                "clip_source": "stand-idle"
            },
            "transitions": [ {
                "target_state": "walking-forward",
                "duration": 0.5,
                "condition": {
                    "parameter": "forward-speed",
                    "operator": ">",
                    "value": 0.1
                }
            } ]
        } ],

        "initial_state": "stand-idle"
    }]
}

```

At runtime, after loading into the AssetManger, an `AnimationController` can be initialized as follows:

```Rust
// First, need to load the shared skeleton object(eg from a COLLADA document)
// This will become more elegant, i promise :)
let skeleton = {
	let collada_document = ColladaDocument::from_path(&Path::new("assets/suit_guy.dae")).unwrap();
	let skeleton_set = collada_document.get_skeletons().unwrap();
	Rc::new(Skeleton::from_collada(&skeleton_set[0]))
}

// Create the AnimationController from the definition, the skeleton, and the clips previously loaded 
// by the animation manager
let controller_def = asset_manager.controller_defs["human-controller"].clone();
let controller = AnimationController::new(controller_def, skeleton.clone(), &asset_manager.animation_clips);
```

Currently, `skeletal_animation` assumes a Piston-style event loop, where we have separate `update` (with delta-time) and `render` (with extrapolated delta-time since last update) events, so on each `update` in the game loop we need to:

```Rust
// Set any relevant parameters on the controller:
controller.set_param_value("forward-speed", 1.8);

// Update the controller's local clock
controller.update(delta_time);
```

Then, on `render`, we can get the current skeletal pose represented with either matrices or dual-quaternions with:

```Rust
// Matrices:
let mut global_poses: [Matrix4<f32>; 64] = [ mat4_id(); 64 ];
controller.get_output_pose(args.ext_dt, &mut global_poses[0 .. skeleton.joints.len()]);

// DualQuaternions:
let mut global_poses: [DualQuaternion<f32>; 64] = [ dual_quaternion::id(); 64 ];
controller.get_output_pose(args.ext_dt, &mut global_poses[0 .. skeleton.joints.len()]);
```

where `args.ext_dt` is the extrapolated time since the last update. To actually render something with the skeletal pose, you can:

* Draw the posed skeleton with [gfx_debug_draw](https://github.com/PistonDevelopers/gfx-debug-draw):
```Rust
skeleton.draw(
	&global_poses,       // The joint poses output from the controller
	&mut debug_renderer, // gfx_debug_draw::DebugRenderer
	true,                // True to label each joint with their name
);
```
where `skeleton` is the shared skeleton instance. Will work with both `Matrix4` and `DualQuaternion`.

* Draw a smoothly-skinned, textured mesh with skeletal_animation::SkinnedRenderer:
```Rust
// On initialization...

// To use matrices with a Linear Blend Skinning (LBS) shader
let skinned_renderer = SkinnedRenderer::<_, Matrix4<f32>>::from_collada_with_canvas(
    canvas, // gfx::Canvas
	collada_document, // the parsed Collada document for the rigged mesh
	["assets/skin.png", "assets/hair.png", "assets/eyes.png"], // Textures for each submesh in the Collada source
).unwrap();

// To use dual-quaternions with a Dual-Quaternion Linear Blend Skinning (DLB) shader
let skinned_renderer = SkinnedRenderer::<_, Matrix4<f32>>::from_collada_with_canvas(
    canvas, // gfx::Canvas
	collada_document, // the parsed Collada document for the rigged mesh
	["assets/skin.png", "assets/hair.png", "assets/eyes.png"], // Textures for each submesh in the Collada source
).unwrap();

...

// Later in event loop...
skinned_renderer.render(
	&mut graphics, // gfx::Graphics
	&output, // gfx::Output
	camera_view, // Matrix4<f32>
	camera_projection, // <Matrix4<f32>
	&global_poses // The output poses from the controller
);
```

See the [example demo](https://github.com/stjahns/skeletal_animation_demo) for a more thorough example of usage.
