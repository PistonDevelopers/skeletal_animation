use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;

use rustc_serialize::{Decodable, Decoder, json};

use animation::{AnimationClip, AnimationClipDef, DifferenceClipDef};
use controller::AnimationControllerDef;

/// A collection of asset definitions, to be loaded from a JSON definition file
#[derive(Debug, RustcDecodable)]
pub struct AssetDefs {
    animation_clips: Option<Vec<AnimationClipDef>>,
    difference_clips: Option<Vec<DifferenceClipDef>>,
    animation_controllers: Option<Vec<AnimationControllerDef>>,
}

///
/// Asset manager - manages memory for loaded assets...?
///
pub struct AssetManager {
    pub animation_clips: HashMap<String, Rc<AnimationClip>>,
    pub controller_defs: HashMap<String, AnimationControllerDef>
}

impl AssetManager {

    pub fn new() -> AssetManager {
        AssetManager {
            animation_clips: HashMap::new(),
            controller_defs: HashMap::new(),
        }
    }

    pub fn load_assets(&mut self, path: &str) {

        let asset_defs: AssetDefs = AssetManager::load_def_from_path(path).unwrap();

        if let Some(animation_clips) = asset_defs.animation_clips {
            for clip_def in animation_clips.iter() {
                let clip = AnimationClip::from_def(clip_def);
                self.animation_clips.insert(clip_def.name.clone(), Rc::new(clip));
            }
        }

        if let Some(difference_clips) = asset_defs.difference_clips {
            for difference_clip_def in difference_clips.iter() {

                let clip = {
                    let ref source_clip = self.animation_clips[&difference_clip_def.source_clip[..]];
                    let ref reference_clip = self.animation_clips[&difference_clip_def.reference_clip[..]];
                    AnimationClip::as_difference_clip(source_clip, reference_clip)
                };

                self.animation_clips.insert(difference_clip_def.name.clone(), Rc::new(clip));
            }
        }

        if let Some(animation_controllers) = asset_defs.animation_controllers {
            for controller_def in animation_controllers.iter() {
                self.controller_defs.insert(controller_def.name.clone(), controller_def.clone());
            }
        }
    }

    pub fn load_def_from_path<T>(path: &str) -> Result<T, &'static str>
        where T: Decodable
    {
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
