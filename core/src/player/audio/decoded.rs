//! Son décodé par FFmpeg, pour les médias qui ne sont pas des WAV : pistes son
//! des vidéos (MOV, MXF, MP4), MP3, FLAC, AIFF...
//!
//! Un décodeur par piste son du média (une caméra enregistre souvent 4 à 8
//! pistes mono). FFmpeg tourne dans un processus séparé et envoie du
//! flottant 32 bits entrelacé. La lecture suivie consomme le flux ; un saut relance le
//! décodage à la position demandée (`-ss`, précis à l'échantillon près).

use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Stdio};

use crate::media::probe::probe;
use crate::media::wav::WavInfo;
use crate::{tools, Error, Result};

pub struct DecodedReader {
    info: WavInfo,
    /// Rang de la piste son dans le média (0 = première).
    stream: usize,
    child: Option<Child>,
    stdout: Option<ChildStdout>,
    /// Image source que délivrera le prochain octet du flux.
    next: u64,
    /// Fin du flux atteinte à la position `next`.
    ended: bool,
    carry: Vec<u8>,
}

impl DecodedReader {
    /// Ouvre toutes les pistes son d'un média que FFmpeg sait lire. Les
    /// pistes sont nommées « Piste N » dans l'ordre de leurs canaux.
    pub fn open_all(path: &Path) -> Result<Vec<Self>> {
        let p = probe(path)?;
        if p.audio.is_empty() {
            return Err(Error::Unsupported(format!(
                "{} : aucune piste son",
                path.display()
            )));
        }
        let mut channel = 0usize;
        let mut out = Vec::new();
        for (stream, a) in p.audio.iter().enumerate() {
            let sample_rate = a.sample_rate.max(1);
            let channels = a.channels.clamp(1, 64) as u16;
            let frames = (p.duration.max(0.0) * sample_rate as f64).round() as u64;
            let mut info = WavInfo::decoded(path, channels, sample_rate, frames);
            info.ixml.track_names = (0..channels as usize)
                .map(|c| Some(format!("Piste {}", channel + c + 1)))
                .collect();
            channel += channels as usize;
            let mut reader = Self {
                info,
                stream,
                child: None,
                stdout: None,
                next: 0,
                ended: false,
                carry: Vec::new(),
            };
            // Décodage lancé dès l'ouverture : au premier lancement, FFmpeg
            // (et l'antivirus sous Windows) peut mettre plusieurs secondes à
            // démarrer ; mieux vaut que ce soit avant d'appuyer sur lecture.
            let _ = reader.spawn(0);
            out.push(reader);
        }
        Ok(out)
    }

    pub fn info(&self) -> &WavInfo {
        &self.info
    }

    fn stop(&mut self) {
        self.stdout = None;
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn spawn(&mut self, start: u64) -> Result<()> {
        self.stop();
        let sr = self.info.sample_rate;
        let mut cmd = tools::command("ffmpeg")?;
        cmd.args(["-v", "error", "-nostdin"]);
        if start > 0 {
            cmd.args(["-ss", &format!("{:.6}", start as f64 / sr as f64)]);
        }
        cmd.arg("-i").arg(&self.info.path);
        cmd.args(["-map", &format!("0:a:{}", self.stream)]);
        let mut child = cmd
            .args([
                "-vn",
                "-ac",
                &self.info.channels.to_string(),
                "-ar",
                &sr.to_string(),
            ])
            .args(["-f", "f32le", "-acodec", "pcm_f32le", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        self.stdout = child.stdout.take();
        self.child = Some(child);
        self.next = start;
        self.ended = false;
        self.carry.clear();
        Ok(())
    }

    /// Lance le décodage à `start` sans attendre la lecture : le démarrage de
    /// FFmpeg se fait pendant la pause, pas au moment d'appuyer sur lecture.
    pub fn prepare(&mut self, start: u64) -> Result<()> {
        if start < self.info.frames
            && (start != self.next || (self.stdout.is_none() && !self.ended))
        {
            self.spawn(start)?;
        }
        Ok(())
    }

    /// Même contrat que `WavReader::read` : jusqu'à `frames` images à partir
    /// de `start`, entrelacées, dans `out` ; 0 en fin de média.
    pub fn read(&mut self, start: u64, frames: usize, out: &mut Vec<f32>) -> Result<usize> {
        out.clear();
        if start >= self.info.frames {
            return Ok(0);
        }
        if start != self.next || (self.stdout.is_none() && !self.ended) {
            self.spawn(start)?;
        }
        if self.ended {
            self.next = start + frames as u64;
            return Ok(0);
        }
        let frame_bytes = self.info.channels as usize * 4;
        let want = frames * frame_bytes;
        let mut raw = [0u8; 65_536];
        while self.carry.len() < want {
            let Some(stdout) = self.stdout.as_mut() else {
                break;
            };
            let n = stdout.read(&mut raw)?;
            if n == 0 {
                self.ended = true;
                break;
            }
            self.carry.extend_from_slice(&raw[..n]);
        }
        let whole = (self.carry.len().min(want) / frame_bytes) * frame_bytes;
        out.extend(
            self.carry[..whole]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c)),
        );
        self.carry.drain(..whole);
        let got = whole / frame_bytes;
        // Flux plus court que la durée annoncée : la suite est du silence,
        // sans relancer FFmpeg à chaque bloc.
        self.next = if self.ended {
            start + frames as u64
        } else {
            start + got as u64
        };
        Ok(got)
    }
}

impl Drop for DecodedReader {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ffmpeg_available() -> bool {
        tools::locate("ffmpeg").is_some() && tools::locate("ffprobe").is_some()
    }

    #[test]
    fn decodes_video_sound_and_seeks() {
        if !ffmpeg_available() {
            eprintln!("FFmpeg absent : test ignoré");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("clip.mov");
        // 2 s de vidéo, son : 1 s de sinus puis 1 s de silence, plus une piste mono.
        let out = tools::command("ffmpeg")
            .unwrap()
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=25:duration=2",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1,apad=whole_dur=2",
            ])
            .args(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=mono:d=2"])
            .args([
                "-map",
                "0:v",
                "-map",
                "1:a",
                "-map",
                "2:a",
                "-c:v",
                "mjpeg",
                "-c:a",
                "pcm_s24le",
            ])
            .arg(&clip)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let mut all = DecodedReader::open_all(&clip).unwrap();
        assert_eq!(all.len(), 2, "deux pistes mono");
        assert_eq!(all[1].info().track_name(0), "Piste 2");
        let mut r = all.remove(0);
        assert_eq!(r.info().channels, 1);
        assert_eq!(r.info().sample_rate, 48_000);
        assert!((r.info().duration() - 2.0).abs() < 0.05);
        let mut buf = Vec::new();
        let peak = |b: &[f32]| b.iter().fold(0f32, |a, s| a.max(s.abs()));
        assert_eq!(r.read(0, 4800, &mut buf).unwrap(), 4800);
        assert!(peak(&buf) > 0.1, "début audible");
        // Lecture suivie puis saut dans le silence.
        assert_eq!(r.read(4800, 4800, &mut buf).unwrap(), 4800);
        assert!(r.read(72_000, 4800, &mut buf).unwrap() > 0);
        assert!(peak(&buf) < 1e-3, "seconde moitié silencieuse");
        // Saut arrière : retour dans le sinus.
        r.read(24_000, 4800, &mut buf).unwrap();
        assert!(peak(&buf) > 0.1);
        // Au-delà de la fin : rien.
        assert_eq!(r.read(200_000, 4800, &mut buf).unwrap(), 0);

        // Analyse LTC : aucune des deux pistes n'en contient.
        let scan =
            crate::player::audio::producer::scan_ltc(std::slice::from_ref(&clip), 2.0).unwrap();
        assert_eq!(scan, vec![None, None]);

        // Moteur complet : la vidéo s'ouvre comme une session de 2 pistes.
        let mut p =
            crate::player::audio::producer::Producer::open(std::slice::from_ref(&clip), 48_000)
                .unwrap();
        assert_eq!(p.info().tracks.len(), 2);
        assert_eq!(p.info().tracks[1].name, "Piste 2");
        let mut block = Vec::new();
        assert!(p.next(&mut block).unwrap() > 0);
        assert!(peak(&block) > 0.05, "son mixé audible");
    }

    #[test]
    fn finds_ltc_on_right_channel_of_a_video() {
        if !ffmpeg_available() {
            eprintln!("FFmpeg absent : test ignoré");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        // Gauche : sinus (micro) ; droite : LTC 14:59:23:03 à 25 i/s.
        let ltc = crate::media::ltc::tests::encode((14, 59, 23, 3), 25, 100, 48_000);
        let wav = dir.path().join("son.wav");
        crate::media::wav::write_test_wav(
            &wav,
            2,
            48_000,
            24,
            false,
            ltc.len() as u64,
            None,
            |n, c| {
                if c == 0 {
                    (n as f64 * 2.0 * std::f64::consts::PI * 440.0 / 48_000.0).sin() * 0.3
                } else {
                    ltc[n as usize] as f64
                }
            },
        )
        .unwrap();
        let clip = dir.path().join("cam.mov");
        let out = tools::command("ffmpeg")
            .unwrap()
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=25:duration=4",
            ])
            .arg("-i")
            .arg(&wav)
            .args([
                "-map",
                "0:v",
                "-map",
                "1:a",
                "-c:v",
                "mjpeg",
                "-c:a",
                "pcm_s24be",
                "-shortest",
            ])
            .arg(&clip)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let scan =
            crate::player::audio::producer::scan_ltc(std::slice::from_ref(&clip), 3.0).unwrap();
        assert_eq!(scan.len(), 2);
        assert!(scan[0].is_none(), "le micro n'est pas du LTC");
        let d = scan[1].as_ref().expect("LTC sur la piste droite");
        // Première trame complète de l'extrait (la trame 03 commence avant lui).
        assert_eq!(d.timecode, "14:59:23:04");
    }
}
