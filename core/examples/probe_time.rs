//! Chronomètre les étapes d'ouverture du son d'un média décodé par FFmpeg.
use std::time::Instant;
fn main() {
    let p = std::path::PathBuf::from(std::env::args().nth(1).expect("fichier"));
    let t = Instant::now();
    let _ = veriflow_core::media::probe::probe(&p).unwrap();
    println!("probe : {:?}", t.elapsed());
    let t = Instant::now();
    let mut r = veriflow_core::player::audio::decoded::DecodedReader::open_all(&p).unwrap();
    println!("open_all : {:?}", t.elapsed());
    let mut buf = Vec::new();
    let t = Instant::now();
    r[0].read(192_000, 1024, &mut buf).unwrap();
    println!("premier bloc à 4 s : {:?}", t.elapsed());
}
