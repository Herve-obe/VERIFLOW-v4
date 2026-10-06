//! Mesure du moteur audio sur une session 32 pistes 192 kHz 32 bits flottant :
//! `cargo run --release --example audio_bench`.
use std::time::Instant;
use veriflow_core::media::wav::write_test_wav;
use veriflow_core::player::audio::producer::Producer;

fn main() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("32ch_192k.wav");
    let seconds = 10u64;
    write_test_wav(
        &path,
        32,
        192_000,
        32,
        true,
        192_000 * seconds,
        None,
        |n, c| 0.1 * ((n as f64) * 0.001 * (c as f64 + 1.0)).sin(),
    )
    .unwrap();
    for out_rate in [192_000, 48_000] {
        let mut p = Producer::open(std::slice::from_ref(&path), out_rate).unwrap();
        let mut out = Vec::new();
        let t = Instant::now();
        while p.next(&mut out).unwrap() > 0 {}
        let elapsed = t.elapsed().as_secs_f64();
        println!(
            "32 pistes 192 kHz -> sortie {} Hz : {seconds} s traitées en {:.2} s ({:.0}x temps réel)",
            out_rate,
            elapsed,
            seconds as f64 / elapsed
        );
    }
}
