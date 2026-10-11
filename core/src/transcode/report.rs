//! Traçabilité d'un lot : empreinte XXH128 de chaque fichier produit et
//! rapport CSV (sources, fichiers produits, encodeurs, mesures).

use std::io::Read;
use std::path::{Path, PathBuf};

use super::FileResult;
use crate::offload::hash::{HashAlgo, MultiHasher};
use crate::Result;

/// Empreinte XXH128 d'un fichier (celle de l'OFFLOAD et des ASC MHL).
pub fn xxh128(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = MultiHasher::new(&[HashAlgo::Xxh128]);
    let mut buf = vec![0u8; 4 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finish()
        .into_iter()
        .next()
        .map(|(_, d)| d)
        .unwrap_or_default())
}

fn cell(v: &str) -> String {
    if v.contains([';', '"', '\n']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_owned()
    }
}

fn num(v: Option<f64>, digits: usize) -> String {
    v.filter(|x| x.is_finite())
        .map(|x| format!("{x:.digits$}").replace('.', ","))
        .unwrap_or_default()
}

/// Contenu du rapport CSV (séparateur point-virgule, lisible dans Excel).
pub fn to_csv(preset: &str, results: &[FileResult]) -> String {
    let mut out = String::from(
        "\u{feff}Source;Fichier produit;Préréglage;Encodeur;État;Temps de traitement (s);Taille (octets);XXH128;LUFS;LRA (LU);True Peak (dBTP);Gain (dB);VMAF;Message\n",
    );
    for r in results {
        let l = r.loudness.as_ref();
        let row = [
            r.source.display().to_string(),
            r.output
                .as_ref()
                .map(|o| o.display().to_string())
                .unwrap_or_default(),
            preset.to_owned(),
            r.encoder
                .as_ref()
                .map(|e| e.name.clone())
                .unwrap_or_default(),
            format!("{:?}", r.status).to_lowercase(),
            num(Some(r.seconds), 1),
            r.size.map(|s| s.to_string()).unwrap_or_default(),
            r.checksum.clone().unwrap_or_default(),
            num(l.map(|l| l.integrated), 1),
            num(l.map(|l| l.range), 1),
            num(l.map(|l| l.true_peak), 1),
            num(l.and_then(|l| l.gain), 2),
            num(r.vmaf, 2),
            r.message.clone().unwrap_or_default(),
        ];
        out += &row.iter().map(|c| cell(c)).collect::<Vec<_>>().join(";");
        out.push('\n');
    }
    out
}

/// Écrit le rapport du lot dans `dir`.
pub fn write(dir: &Path, preset: &str, results: &[FileResult]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = format!(
        "VERIFLOW_TRANSCODE_{}.csv",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = dir.join(name);
    std::fs::write(&path, to_csv(preset, results))?;
    Ok(path)
}
