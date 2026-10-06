//! Liste les sorties audio vues par VERIFLOW (tous pilotes).
//! Usage : cargo run -p veriflow-core --example audio_devices

fn main() {
    for d in veriflow_core::player::audio::engine::output_devices() {
        println!(
            "{}{} | {} | {} canaux | {}",
            if d.default { "* " } else { "  " },
            d.host,
            d.name,
            d.channels,
            d.id
        );
    }
}
