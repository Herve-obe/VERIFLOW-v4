//! Offload en ligne de commande (essais et démonstration) :
//! `cargo run --release --example offload_cli -- <source> <destination>... [--algo xxh128,md5]`
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use veriflow_core::offload::engine::{run, Event, OffloadSpec};
use veriflow_core::offload::hash::HashAlgo;
use veriflow_core::offload::mhl::{write_generation, MhlAuthor};
use veriflow_core::offload::report::{write_reports, ReportData, ReportInfo};
use veriflow_core::offload::scan::scan;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut algos = vec![HashAlgo::Xxh128];
    if let Some(i) = args.iter().position(|a| a == "--algo") {
        algos = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect();
        args.drain(i..=i + 1);
    }
    let source = PathBuf::from(&args[0]);
    let inv = scan(&source).unwrap();
    let destinations: Vec<PathBuf> = args[1..]
        .iter()
        .map(|d| PathBuf::from(d).join(&inv.name))
        .collect();
    let spec = OffloadSpec {
        destinations: destinations.clone(),
        algorithms: algos.clone(),
    };
    println!("{} fichiers, {} octets", inv.files.len(), inv.total_bytes);
    let summary = run(&inv, &spec, &AtomicBool::new(false), |e| {
        if let Event::FileDone { result, .. } = e {
            println!("{} {:?}", result.rel, result.destinations);
        }
    })
    .unwrap();
    println!(
        "{:.2} s, {} en échec",
        summary.duration_s, summary.failed_files
    );
    let mut mhl = Vec::new();
    for (i, d) in destinations.iter().enumerate() {
        let p = write_generation(&inv, &summary, i, d, &algos, &MhlAuthor::default()).unwrap();
        println!("MHL : {}", p.display());
        mhl.push(Some(p));
    }
    let info = ReportInfo { project: Some("Démo VERIFLOW".into()), operator: Some("DIT".into()), notes: None };
    let data = ReportData { inv: &inv, summary: &summary, spec: &spec, info: &info, mhl: &mhl };
    for r in write_reports(&data).unwrap() {
        println!("Rapport : {}", r.pdf.display());
    }
}
