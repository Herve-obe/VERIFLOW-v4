//! Mesure des temps de saut et de lecture : `cargo run --release --example seek_bench -- <fichier>`.
use std::time::Instant;
use veriflow_core::player::video::VideoPlayer;

fn main() {
    let path = std::env::args().nth(1).expect("chemin du clip");
    let mut p = VideoPlayer::open(path.as_ref(), 1280, 720).unwrap();
    let n = p.clip().frame_count;
    println!(
        "{} images, affichage {}x{}",
        n,
        p.clip().display_width,
        p.clip().display_height
    );
    for target in [n / 2, n / 5, n - 10, 7] {
        let t = Instant::now();
        p.frame(target).unwrap();
        println!(
            "saut vers {target}: {:.0} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    let t = Instant::now();
    let start = n / 5;
    for i in start..start + 100 {
        p.frame(i).unwrap();
    }
    println!(
        "lecture continue : {:.0} images/s",
        100.0 / t.elapsed().as_secs_f64()
    );
    let t = Instant::now();
    for i in (start + 76..start + 100).rev() {
        p.frame(i).unwrap();
    }
    println!(
        "retour arrière (cache) : {:.2} ms/image",
        t.elapsed().as_secs_f64() * 1000.0 / 24.0
    );
}
