//! Mesure le délai entre « lecture » et le premier son produit pour un média
//! décodé par FFmpeg (son d'une vidéo).
//! Usage : cargo run --release -p veriflow-core --example av_latency -- <fichier> [secondes]

use std::time::{Duration, Instant};

use veriflow_core::player::audio::engine::AudioEngine;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().expect("fichier"));
    let at: f64 = args.next().map(|s| s.parse().unwrap()).unwrap_or(4.0);
    let t0 = Instant::now();
    let e = AudioEngine::open(std::slice::from_ref(&path)).expect("ouverture");
    println!("ouverture : {:?}", t0.elapsed());
    for round in 0..3 {
        let frame = ((at + round as f64 * 5.0) * e.info().sample_rate as f64) as u64;
        let t = Instant::now();
        e.seek(frame);
        e.play();
        loop {
            if e.position() > frame + e.info().sample_rate as u64 / 50 {
                println!(
                    "saut à {:.1} s : son au bout de {:?}",
                    frame as f64 / e.info().sample_rate as f64,
                    t.elapsed()
                );
                break;
            }
            if t.elapsed() > Duration::from_secs(5) {
                println!(
                    "saut à {frame} : AUCUN son en 5 s (position {})",
                    e.position()
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        e.pause();
    }
}
