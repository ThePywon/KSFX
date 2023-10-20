use std::fs::{File, read_dir};
use std::env;
use std::io::{Read, Write};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use rodio::{Decoder, OutputStream, Sink, Source};
use device_query::{DeviceQuery, DeviceState};
use rand::random;

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum SoundPackSettings {
  Basic(String),
  Advanced {
    name: String,
    volume: Option<f32>,
    pitch_start: Option<f32>,
    pitch_range: Option<f32>,
    pitch_steps: Option<f32>,
    fast_threshold: Option<f32>
  }
}

fn get_path(settings: &SoundPackSettings) -> &String {
  match settings {
    SoundPackSettings::Basic(path) => return path,
    SoundPackSettings::Advanced { name, volume: _,
      pitch_start: _, pitch_range: _, pitch_steps: _,
      fast_threshold: _ } => return name
  }
}

fn get_name(settings: &SoundPackSettings) -> String {
  let path = get_path(settings);
  if let Some(idx) = path.chars().rev().position(|c| c == '/' || c == '\\') {
    return path[(path.chars().count() - idx - 1)..].to_string();
  }
  else {
    return path.clone();
  }
}

#[derive(Deserialize, Serialize)]
pub struct Settings {
  sound_packs: Vec<SoundPackSettings>,
  previous_sound_pack: Option<Vec<String>>,
  next_sound_pack: Option<Vec<String>>,
  terminate: Option<Vec<String>>,
  toggle: Option<Vec<String>>,
  volume: Option<f32>,
  pitch_start: Option<f32>,
  pitch_range: Option<f32>,
  pitch_steps: Option<f32>,
  fast_threshold: Option<f32>
}

fn main() {
  let (_stream, stream_handle) = OutputStream::try_default()
    .expect("Could not get default output stream handle");

  let mut active = true;

  let sink = Sink::try_new(&stream_handle).unwrap();
  let device_state = DeviceState::new();


  let config_path;
  if let Some(path) = env::args().nth(1) {
    config_path = path;
  }
  else {
    config_path = String::from("ksfx.json");
  }

  let mut serialized_config = String::new();
  let settings: Settings;
  if let Ok(mut file) = File::open(&config_path) {
    file.read_to_string(&mut serialized_config)
      .expect(&format!("Could not read from config file at \"{}\"", config_path));
    settings = serde_json::from_str(&serialized_config)
      .expect(&format!("Config file invalid at \"{}\"", config_path));
  }
  else {
    println!("Config file not found at path \"{}\"", config_path);
    serialized_config = String::from(
"{
  \"sound_packs\": [\"assets\"],
  \"previous_sound_pack\": [\"F4\"],
  \"next_sound_pack\": [\"F5\"],
  \"terminate\": [\"F2\"],
  \"toggle\": [\"F3\"],
  \"volume\": 1.0,
  \"pitch_start\": 0.5,
  \"pitch_range\": 0.5,
  \"pitch_steps\": 0.005,
  \"fast_threshold\": 1.0
}"
    );
    let file = File::create(&config_path);
    if let Ok(mut f) = file {
      f.write_all(serialized_config.as_bytes()).unwrap();
      println!("Created config file with default settings");
    }
    else {
      println!("Could not create config file in local directory!");
    }
    settings = Settings { sound_packs: vec![SoundPackSettings::Basic(String::from("assets"))],
      previous_sound_pack: Some(vec![String::from("F4")]),
      next_sound_pack: Some(vec![String::from("F5")]),
      terminate: Some(vec![String::from("F2")]),
      toggle: Some(vec![String::from("F3")]), volume: Some(1.0),
      pitch_start: Some(0.5), pitch_range: Some(0.5),
      pitch_steps: Some(0.005), fast_threshold: Some(1.0) };
  }



  let mut sound_packs = Vec::new();
  for sound_pack in settings.sound_packs.iter() {
    let path = get_path(sound_pack);
    let mut sounds = Vec::new();
    let dir = read_dir(path)
      .expect(&format!("Sound pack folder not found at \"{}\"", path));
    for entry in dir.into_iter() {
      sounds.push(Decoder::new(File::open(entry.unwrap().path()).unwrap()).unwrap().buffered());
    }
    sound_packs.push(sounds);
  }



  let mut previous_key_amt = 0;
  let mut last_press = Instant::now();
  let mut pitch = settings.pitch_start.unwrap_or(0.5);
  let mut toggled = false;
  let mut switched_pack = false;
  let mut current_sound_pack = 0;

  loop {
    let keys =  device_state.get_keys();
    let key_names: Vec<String> = keys.clone().iter().map(|x| x.to_string()).collect();
    
    if let Some(keybind) = settings.terminate.clone() {
      if keybind.len() == key_names.len() &&
        keybind.iter().all(|x| key_names.contains(x)) {
          return println!("Program terminated!");
      }
    }
    if let Some(keybind) = settings.toggle.clone() {
      if !toggled && keybind.len() == key_names.len() &&
        keybind.iter().all(|x| key_names.contains(x)) {
          toggled = true;
          active = !active;
          println!("Toggled keyboard sound effects.");
      }
    }
    if keys.len() == 0 { toggled = false; switched_pack = false; }
    if let Some(keybind) = settings.previous_sound_pack.clone() {
      if !switched_pack && keybind.len() == key_names.len() &&
        keybind.iter().all(|x| key_names.contains(x)) {
          switched_pack = true;
          if current_sound_pack == 0 {
            current_sound_pack = sound_packs.len();
          }
          else {
            current_sound_pack -= 1;
          }
          current_sound_pack %= sound_packs.len();
          println!("Changed sound pack to \"{}\"", get_name(&settings.sound_packs[current_sound_pack]));
      }
    }
    if let Some(keybind) = settings.next_sound_pack.clone() {
      if !switched_pack && keybind.len() == key_names.len() &&
        keybind.iter().all(|x| key_names.contains(x)) {
          switched_pack = true;
          current_sound_pack += 1;
          current_sound_pack %= sound_packs.len();
          println!("Changed sound pack to \"{}\"", get_name(&settings.sound_packs[current_sound_pack]));
      }
    }

    if !active { continue; }

    if keys.len() > previous_key_amt {
      let selection = random::<f32>() * sound_packs[current_sound_pack].len() as f32;
      let (volume, pitch_start,
        pitch_range, pitch_steps, fast_threshold);
      match settings.sound_packs[current_sound_pack] {
        SoundPackSettings::Advanced { name: _, volume: a, pitch_start: b,
          pitch_range: c, pitch_steps: d, fast_threshold: e } => {
            volume = a.unwrap_or(settings.volume.unwrap_or(1.0));
            pitch_start = b.unwrap_or(settings.pitch_start.unwrap_or(0.5));
            pitch_range = c.unwrap_or(settings.pitch_range.unwrap_or(0.5));
            pitch_steps = d.unwrap_or(settings.pitch_steps.unwrap_or(0.005));
            fast_threshold = e.unwrap_or(settings.fast_threshold.unwrap_or(1.0));
          }
        SoundPackSettings::Basic(_) => {
          volume = settings.volume.unwrap_or(1.0);
          pitch_start = settings.pitch_start.unwrap_or(0.5);
          pitch_range = settings.pitch_range.unwrap_or(0.5);
          pitch_steps = settings.pitch_steps.unwrap_or(0.005);
          fast_threshold = settings.fast_threshold.unwrap_or(1.0);
        }
      }

      let fast = last_press.elapsed() < Duration::from_millis((fast_threshold * 1000.0) as u64);
      if pitch < pitch_start + pitch_range && fast {
        pitch += pitch_steps;
      }
      else if !fast {
        pitch = pitch_start;
      }

      last_press = Instant::now();

      sink.stop();
      sink.empty();
      sink.set_speed(pitch);
      sink.set_volume(volume);
      sink.append(sound_packs[current_sound_pack][selection as usize].clone());
    }

    previous_key_amt = keys.len();
  }
}
