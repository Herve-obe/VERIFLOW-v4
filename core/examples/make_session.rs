//! Crée une session de démonstration : WAV polyphonique avec noms de pistes iXML.
//! `cargo run --example make_session -- <fichier.wav> [pistes] [fréquence]`
use veriflow_core::media::wav::write_test_wav;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("chemin de sortie");
    let tracks: u16 = args.next().map(|v| v.parse().unwrap()).unwrap_or(32);
    let rate: u32 = args.next().map(|v| v.parse().unwrap()).unwrap_or(48_000);
    let names = [
        "Perche",
        "HF Marie",
        "HF Paul",
        "HF Léa",
        "Ambiance G",
        "Ambiance D",
        "Mix G",
        "Mix D",
    ];
    let mut xml = String::from("<BWFXML><PROJECT>Démo VERIFLOW</PROJECT><SCENE>12A</SCENE><TAKE>3</TAKE><CIRCLED>TRUE</CIRCLED><TRACK_LIST>");
    for i in 0..tracks {
        let name = names
            .get(i as usize)
            .map(|s| s.to_string())
            .unwrap_or(format!("Iso {}", i + 1));
        xml += &format!(
            "<TRACK><CHANNEL_INDEX>{}</CHANNEL_INDEX><NAME>{}</NAME></TRACK>",
            i + 1,
            name
        );
    }
    xml += "</TRACK_LIST></BWFXML>";
    // Chaque piste : sinus de fréquence différente, niveau décroissant.
    write_test_wav(
        path.as_ref(),
        tracks,
        rate,
        24,
        false,
        rate as u64 * 20,
        Some(&xml),
        |n, c| {
            let f = 110.0 * (c as f64 + 1.0);
            let level = 0.5 / (1.0 + c as f64 * 0.3);
            level * (2.0 * std::f64::consts::PI * f * n as f64 / rate as f64).sin()
        },
    )
    .unwrap();
}
