extern crate sdl2;
use protracktor::ModPlayer;
use sdl2::audio::{AudioCallback, AudioSpecDesired};
use std::env;
use std::fs;
use std::io::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct SDLSound {
    player: ModPlayer,
}

impl AudioCallback for SDLSound {
    type Channel = f32;

    fn callback(&mut self, out: &mut [Self::Channel]) {
        self.player.render(out);
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 {
        let term = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;
        let module = fs::read(&args[1]).unwrap();
        let player: ModPlayer = ModPlayer::load(module);
        println!("PLAYING: {}", player.name);
        println!("Samples: {}", player.samples.len());

        let sdl_context = sdl2::init().unwrap();
        let audio_subsystem = sdl_context.audio().expect("Audio system failed");

        let desired_spec = AudioSpecDesired {
            freq: Some(48_000),
            channels: Some(2), // mono
            samples: None,     // default sample size
        };
        let device = audio_subsystem
            .open_playback(None, &desired_spec, |spec| {
                println!("Spec: {:?}", spec);
                SDLSound { player }
            })
            .expect("Device open failed");
        device.resume();
        while !term.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(200));
        }
    } else {
        panic!("No module given");
    }
    Ok(())
}
