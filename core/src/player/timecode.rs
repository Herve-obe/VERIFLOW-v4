//! Timecode SMPTE (charte §7.5) : conversions images <-> HH:MM:SS:FF,
//! cadences entières et NTSC (23.976, 29.97, 59.94), drop-frame et non drop-frame.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

/// Cadence exprimée en fraction exacte (ex. 30000/1001 pour 29.97).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FrameRate {
    pub num: u32,
    pub den: u32,
}

impl FrameRate {
    pub const fn new(num: u32, den: u32) -> Self {
        Self { num, den }
    }

    pub fn as_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Nombre d'images par seconde "nominal" utilisé pour compter le timecode
    /// (24 pour 23.976, 30 pour 29.97...).
    pub fn nominal(self) -> u32 {
        (self.as_f64()).round() as u32
    }

    /// Vrai pour les cadences NTSC (dénominateur 1001).
    pub fn is_ntsc(self) -> bool {
        self.den == 1001
    }

    /// Le drop-frame n'existe que pour 29.97 et 59.94.
    pub fn supports_drop_frame(self) -> bool {
        self.is_ntsc() && self.nominal().is_multiple_of(30)
    }

    /// Lit une cadence au format FFmpeg ("30000/1001", "25/1", "25").
    pub fn parse(s: &str) -> Option<Self> {
        let (num, den) = match s.split_once('/') {
            Some((n, d)) => (n.trim().parse().ok()?, d.trim().parse().ok()?),
            None => (s.trim().parse().ok()?, 1),
        };
        (num > 0 && den > 0).then_some(Self { num, den })
    }

    /// Durée d'une image en secondes.
    pub fn frame_duration(self) -> f64 {
        self.den as f64 / self.num as f64
    }
}

/// Timecode stocké sous forme d'un nombre d'images depuis 00:00:00:00.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Timecode {
    pub frames: i64,
    pub rate: FrameRate,
    pub drop_frame: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TimecodeParseError;

impl fmt::Display for TimecodeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("timecode invalide")
    }
}

impl std::error::Error for TimecodeParseError {}

impl Timecode {
    pub fn from_frames(frames: i64, rate: FrameRate, drop_frame: bool) -> Self {
        Self {
            frames,
            rate,
            drop_frame: drop_frame && rate.supports_drop_frame(),
        }
    }

    /// Images supprimées par minute en drop-frame (2 en 29.97, 4 en 59.94).
    fn drop_count(self) -> i64 {
        (self.rate.nominal() / 30 * 2) as i64
    }

    /// Décompose en (heures, minutes, secondes, images).
    pub fn components(self) -> (i64, i64, i64, i64) {
        let fps = self.rate.nominal() as i64;
        let mut n = self.frames.rem_euclid(24 * 3600 * fps);
        if self.drop_frame {
            let drop = self.drop_count();
            let per_10min = fps * 600 - drop * 9;
            let per_min = fps * 60 - drop;
            let tens = n / per_10min;
            let rem = n % per_10min;
            n += drop * 9 * tens;
            if rem > drop {
                n += drop * ((rem - drop) / per_min);
            }
        }
        (n / (fps * 3600), n / (fps * 60) % 60, n / fps % 60, n % fps)
    }

    /// Construit un timecode à partir de ses composants.
    pub fn from_components(
        h: i64,
        m: i64,
        s: i64,
        f: i64,
        rate: FrameRate,
        drop_frame: bool,
    ) -> Self {
        let fps = rate.nominal() as i64;
        let drop_frame = drop_frame && rate.supports_drop_frame();
        let mut frames = ((h * 60 + m) * 60 + s) * fps + f;
        if drop_frame {
            let drop = (rate.nominal() / 30 * 2) as i64;
            let total_minutes = h * 60 + m;
            frames -= drop * (total_minutes - total_minutes / 10);
        }
        Self {
            frames,
            rate,
            drop_frame,
        }
    }

    /// Ajoute un décalage en images.
    pub fn offset(self, frames: i64) -> Self {
        Self {
            frames: self.frames + frames,
            ..self
        }
    }

    /// Lit "HH:MM:SS:FF" (ou "HH:MM:SS;FF" en drop-frame).
    pub fn parse(s: &str, rate: FrameRate) -> Result<Self, TimecodeParseError> {
        let drop_frame = s.contains(';');
        let parts: Vec<i64> = s
            .split([':', ';', '.'])
            .map(|p| p.trim().parse::<i64>().map_err(|_| TimecodeParseError))
            .collect::<Result<_, _>>()?;
        let [h, m, sec, f] = parts[..] else {
            return Err(TimecodeParseError);
        };
        if m >= 60 || sec >= 60 || f >= rate.nominal() as i64 || h < 0 || m < 0 || sec < 0 || f < 0
        {
            return Err(TimecodeParseError);
        }
        Ok(Self::from_components(h, m, sec, f, rate, drop_frame))
    }
}

impl fmt::Display for Timecode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (h, m, s, fr) = self.components();
        let sep = if self.drop_frame { ';' } else { ':' };
        write!(f, "{h:02}:{m:02}:{s:02}{sep}{fr:02}")
    }
}

impl FromStr for FrameRate {
    type Err = TimecodeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(TimecodeParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R25: FrameRate = FrameRate::new(25, 1);
    const R2997: FrameRate = FrameRate::new(30000, 1001);
    const R5994: FrameRate = FrameRate::new(60000, 1001);
    const R23976: FrameRate = FrameRate::new(24000, 1001);

    #[test]
    fn ndf_25_roundtrip() {
        let tc = Timecode::parse("10:00:00:00", R25).unwrap();
        assert_eq!(tc.frames, 10 * 3600 * 25);
        assert_eq!(tc.offset(24).to_string(), "10:00:00:24");
        assert_eq!(tc.offset(25).to_string(), "10:00:01:00");
    }

    #[test]
    fn df_2997_skips_frames_0_and_1_each_minute() {
        let tc = Timecode::parse("00:00:59;29", R2997).unwrap();
        assert_eq!(tc.offset(1).to_string(), "00:01:00;02");
        // Pas de saut aux minutes multiples de 10.
        let tc = Timecode::parse("00:09:59;29", R2997).unwrap();
        assert_eq!(tc.offset(1).to_string(), "00:10:00;00");
    }

    #[test]
    fn df_2997_one_hour_is_107892_frames() {
        let tc = Timecode::parse("01:00:00;00", R2997).unwrap();
        assert_eq!(tc.frames, 107_892);
        assert_eq!(
            Timecode::from_frames(107_892, R2997, true).to_string(),
            "01:00:00;00"
        );
    }

    #[test]
    fn df_5994_drops_four_frames() {
        let tc = Timecode::parse("00:00:59;59", R5994).unwrap();
        assert_eq!(tc.offset(1).to_string(), "00:01:00;04");
    }

    #[test]
    fn df_roundtrip_every_frame_over_11_minutes() {
        for n in 0..(11 * 60 * 30) {
            let tc = Timecode::from_frames(n, R2997, true);
            let back = Timecode::parse(&tc.to_string(), R2997).unwrap();
            assert_eq!(back.frames, n, "image {n} -> {tc}");
        }
    }

    #[test]
    fn drop_frame_ignored_when_unsupported() {
        let tc = Timecode::from_frames(100, R23976, true);
        assert!(!tc.drop_frame);
        assert_eq!(tc.to_string(), "00:00:04:04");
    }

    #[test]
    fn wraps_at_24h() {
        let tc = Timecode::parse("23:59:59:24", R25).unwrap().offset(1);
        assert_eq!(tc.to_string(), "00:00:00:00");
    }

    #[test]
    fn rejects_invalid() {
        assert!(Timecode::parse("00:00:00:25", R25).is_err());
        assert!(Timecode::parse("00:61:00:00", R25).is_err());
        assert!(Timecode::parse("abc", R25).is_err());
    }

    #[test]
    fn parses_ffmpeg_rates() {
        assert_eq!(FrameRate::parse("30000/1001"), Some(R2997));
        assert_eq!(FrameRate::parse("25"), Some(R25));
        assert_eq!(FrameRate::parse("0/0"), None);
        assert!(R2997.supports_drop_frame());
        assert!(!R23976.supports_drop_frame());
    }
}
