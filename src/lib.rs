#![feature(collections)]
#![feature(core)]
#![feature(custom_attribute)]
#![feature(old_path)]
#![feature(plugin)]
#![feature(convert)]
#![feature(std_misc)]
#![plugin(gfx_macros)]

extern crate collada;
extern crate gfx;
extern crate gfx_debug_draw;
extern crate gfx_device_gl;
extern crate gfx_texture;
extern crate quack;
extern crate quaternion;
extern crate vecmath;
extern crate interpolation;
extern crate rustc_serialize;

pub mod animation;
pub mod skinned_renderer;
pub mod blend_tree;
pub mod controller;
pub mod manager;
pub mod skeleton;
mod math;

pub use animation::{
    AnimationClip,
    AnimationSample,
    SQT,
};

pub use skeleton::{
    draw_skeleton,
};

pub use blend_tree::{
    BlendTreeNode,
    BlendTreeNodeDef,
};

pub use manager::AssetManager;

pub use controller::AnimationController;

pub use skinned_renderer::SkinnedRenderer;
