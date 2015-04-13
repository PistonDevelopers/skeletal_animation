# skeletal_animation

[![Build Status](https://travis-ci.org/PistonDevelopers/skeletal_animation.png?branch=master)](https://travis-ci.org/PistonDevelopers/skeletal_animation)

A Rust library for data-driven skeletal animation.

[Documentation](http://docs.piston.rs/skeletal_animation/skeletal_animation/)

## Overview

This library allows you to define animation clips, state machines, and blend trees in JSON to be loaded and reloaded at runtime without needing to recompile your Rust project. 

## Usage

### Animation Clips

Declare animation clips to be loaded in a JSON file, eg `animation_clips.json`:

```json
[{
	"name": "walk-forward",
	"source": "assets/walk.dae",
}, {
	"name": "run-forward",
	"source": "assets/run.dae",
}]
```

where:
* `name` should be a unique identifier for the animation clip that can be referenced from the animation controller definition.
* `source` is the path to a COLLADA file containing the desired animation.

At runtime, animation clips can be loaded through the `AssetManager` with:

```Rust
let mut asset_manager = AssetManager::new();
asset_manager.load_animations("assets/animation_clips.json");
```

### Animation Controllers

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

An example controller definition, eg `human_controller.json`:

```json
{
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
}

```

At runtime, an `AnimationController` can be initialized as follows:

```Rust
// First, need to load the shared skeleton object(eg from a COLLADA document)
// This will become more elegant, i promise :)
let skeleton = {
	let collada_document = ColladaDocument::from_path(&Path::new("assets/suit_guy.dae")).unwrap();
	let skeleton_set = collada_document.get_skeletons().unwrap();
	Rc::new(Skeleton::from_collada(&skeleton_set[0]))
}

// Load the AnimationControllerDef
let controller_def = AssetManager::load_def_from_path("assets/human_controller.json").unwrap();

// Create the AnimationController from the definition, the skeleton, and the clips previously loaded 
// by the animation manager
let mut controller = AnimationController::new(controller_def, skeleton.clone(), &asset_manager.animation_clips);
```

Currently, `skeletal_animation` assumes a Piston-style event loop, where we have separate `update` (with delta-time) and `render` (with extrapolated delta-time since last update) events, so on each `update` in the game loop we need to:

```Rust
// Set any relevant parameters on the controller:
controller.set_param_value("forward-speed", 1.8);

// Update the controller's local clock
controller.update(delta_time);
```

Then, on `render`, we can get the current skeletal pose with:

```Rust
let mut global_poses: [Matrix4<f32>; 64] = [ mat4_id(); 64 ];
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
where `skeleton` is the shared skeleton instance.

* Draw a smoothly-skinned, textured mesh with skeletal_animation::SkinnedRenderer:
```Rust
// On initialization...
let mut skinned_renderer = SkinnedRenderer::from_collada(
	&mut graphics, // gfx::Graphics
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
