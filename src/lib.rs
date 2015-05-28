//! A library for data-driven skeletal animation.

extern crate collada;
#[macro_use]
extern crate gfx;
extern crate gfx_debug_draw;
extern crate gfx_texture;
extern crate quaternion;
extern crate dual_quaternion;
extern crate vecmath;
extern crate interpolation;
extern crate rustc_serialize;
extern crate float;

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

pub use transform::{Transform, QVTransform, FromTransform};

pub use skeleton::{
    Skeleton,
};

pub use manager::{
    AssetManager,
    AssetDefs,
};

pub use controller::AnimationController;

pub use skinned_renderer::{SkinnedRenderer, HasShaderSources};
