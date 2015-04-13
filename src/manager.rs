use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::rc::Rc;

use collada::document::ColladaDocument;
use rustc_serialize::{self, Decodable, Decoder, json};

use animation::AnimationClip;
use math;
use skeleton::Skeleton;

///
/// Asset manager - manages memory for loaded assets...?
///
pub struct AssetManager {
    pub animation_clips: HashMap<String, Rc<AnimationClip>>
}

impl AssetManager {

    pub fn new() -> AssetManager {
        AssetManager {
            animation_clips: HashMap::new(),
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

    // TODO load/manage blend tree defs

    pub fn load_animations(&mut self, path: &str) ->  Result<(), &'static str> {

        let json = try!(read_json_file(path));

        let mut decoder = json::Decoder::new(json);

        decoder.read_seq(|decoder, len| {
            for i in (0 .. len) {
                decoder.read_struct("root", 0, |decoder| {

                    let name = try!(decoder.read_struct_field("name", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    let source = try!(decoder.read_struct_field("source", 0, |decoder| { Ok(try!(decoder.read_str())) }));
                    let looping = try!(decoder.read_struct_field("looping", 0, |decoder| { Ok(try!(decoder.read_bool())) }));
                    let duration = try!(decoder.read_struct_field("duration", 0, |decoder| { Ok(try!(decoder.read_f32())) }));
                    let rotate_z_angle = try!(decoder.read_struct_field("rotate-z", 0, |decoder| { Ok(try!(decoder.read_f32())) }));

                    // Wacky. Shouldn't it be an error if the struct field isn't present?
                    let adjust = if !rotate_z_angle.is_nan() {
                        math::mat4_rotate_z(rotate_z_angle.to_radians())
                    } else {
                        math::mat4_id()
                    };

                    let collada_document = ColladaDocument::from_path(&Path::new(&source[..])).unwrap();
                    let animations = collada_document.get_animations();
                    let mut skeleton_set = collada_document.get_skeletons().unwrap();
                    let skeleton = Skeleton::from_collada(&skeleton_set[0]);

                    let mut clip = AnimationClip::from_collada(&skeleton, &animations, &adjust);

                    if !duration.is_nan() {
                        clip.set_duration(duration);
                    }

                    self.animation_clips.insert(name, Rc::new(clip));

                    Ok(())
                });
            }

            Ok(())
        });

        Ok(())
    }

}

///
/// Read the json definition file at the given path and parse it into a Json struct
/// TODO better error messages
///
fn read_json_file(path: &str) -> Result<json::Json, &'static str> {

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

    let json = match json::Json::from_str(&json_string[..]) {
        Ok(x) => x,
        Err(e) => return Err("Failed to parse JSON")
    };

    Ok(json)
}
