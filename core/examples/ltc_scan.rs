//! Cherche du LTC sur chaque canal d'un WAV et affiche le timecode lu.
//! Usage : cargo run -p veriflow-core --example ltc_scan -- <fichier.wav>
use veriflow_core::media::{ltc, wav::WavReader};

fn main() {
    let path = std::path::PathBuf::from(std::env::args().nth(1).expect("fichier"));
    let mut r = WavReader::open(&path).expect("WAV");
    let info = r.info().clone();
    let frames = (info.sample_rate as usize * 4).min(info.frames as usize);
    let mut buf = Vec::new();
    r.read(0, frames, &mut buf).unwrap();
    let ch = info.channels as usize;
    for c in 0..ch {
        let mono: Vec<f32> = buf.iter().skip(c).step_by(ch).copied().collect();
        match ltc::detect(&mono, info.sample_rate) {
            Some(d) => println!(
                "canal {} : LTC {} à {} i/s ({} trames)",
                c + 1,
                d.timecode,
                d.fps,
                d.frames
            ),
            None => println!("canal {} : pas de LTC", c + 1),
        }
    }
}
