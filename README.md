# skeletal_animation

[![Build Status](https://travis-ci.org/PistonDevelopers/skeletal_animation.png?branch=master)](https://travis-ci.org/PistonDevelopers/skeletal_animation)

A Rust library for data-driven skeletal animation.

Usage:

Declare animation clips to be loaded in a JSON file, eg `animation_clips.json`:

```
[{
	"name": "walk-forward",
	"source": "assets/walk.dae",
	"looping": true,
	"duration": 1.0
}, {
	"name": "run-forward",
	"source": "assets/run.dae",
	"looping": true,
	"duration": 1.0
}]
```

Define the AnimationController in another JSON file, eg `human_controller.json`:

```
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

Load the skeleton from a Collada source file:

```
let skeleton = {
	let collada_document = ColladaDocument::from_path(&Path::new("assets/suit_guy.dae")).unwrap();
	let skeleton_set = collada_document.get_skeletons().unwrap();
	Skeleton::from_collada(&skeleton_set[0])
};

```

Load animation clips through the AssetManager:

```
let mut asset_manager = AssetManager::new();
asset_manager.load_animations("assets/animation_clips.json");
```

Load the animation controller definition:

```
let controller_def = AssetManager::load_def_from_path("assets/human_controller.json").unwrap();
```

Instantiate an AnimationController using the controller definition, the skeleton, and a mapping of names to AnimationClips.

```
let mut controller = AnimationController::new(controller_def, skeleton.clone(), &asset_manager.animation_clips);

```

(Optional) Instantiate the bundled SkinnedRenderer (for rendering a skinned mesh given a skeletal pose)

```
let mut skinned_renderer = SkinnedRenderer::from_collada(&mut graphics, collada_document, texture_paths).unwrap();
```

...

In event loop, on update:

```
// Set any relevant parameters on the controller:
controller.set_param_value("forward-speed", 1.8);

// Update the controller's local clock
controller.update(delta_time);

```

...

In event loop, on render:

```
// Get output pose for skeleton from the controller:
let mut global_poses: [Matrix4<f32>; 64] = [ mat4_id(); 64 ];
controller.get_output_pose(args.ext_dt, &mut global_poses[0 .. skeleton.borrow().joints.len()]);

// Use pose to render smooth-skinned mesh with SkinnedRenderer:
skinned_renderer.render(&mut graphics, &frame, camera_view, camera_projection, &global_poses);

// Use pose to render skeleton with labeled bones: (see gfx_debug_draw)
skeleton.draw(&global_poses, &mut debug_renderer, true);

```
