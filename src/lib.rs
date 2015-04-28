#![feature(custom_attribute)]
#![feature(plugin)]
#![feature(convert)]
#![feature(std_misc)]
#![plugin(gfx_macros)]

//! A library for data-driven skeletal animation.

extern crate collada;
extern crate gfx;
extern crate gfx_debug_draw;
extern crate gfx_device_gl;
extern crate gfx_texture;
extern crate quaternion;
extern crate dual_quaternion;
extern crate vecmath;
extern crate interpolation;
extern crate rustc_serialize;

pub mod animation;
pub mod skinned_renderer;
pub mod blend_tree;
pub mod controller;
pub mod manager;
pub mod skeleton;
pub mod math;
mod transform;

pub use animation::{
    AnimationClip,
    AnimationSample,
};

pub use transform::{Transform, QVTransform};

pub use skeleton::{
    Skeleton,
};

pub use blend_tree::{
    BlendTreeNode,
    BlendTreeNodeDef,
};

pub use manager::{
    AssetManager,
    AssetDefs,
};

pub use controller::AnimationController;

pub use skinned_renderer::{SkinnedRenderer, HasShaderSources};
