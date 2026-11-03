//! Le son du micro, sans Windows : ce que `MindcareMicro` (natif.cs) mesure pendant l'écoute (`Analyse`) et le fichier
//! .wav donné à whisper (`SaveWav`). 16 000 échantillons par seconde, 16 bits, mono.

/// Échantillons par seconde
pub const FREQUENCE: u32 = 16_000;
/// La taille d'un tampon du micro : 100 ms (`BufBytes`)
pub const TAMPON: usize = 3_200;
/// Le son gardé au plus : 30 secondes (l'écoute s'arrête avant, à 25 secondes de son ou 30 secondes en tout)
pub const SON_MAX: usize = 30 * FREQUENCE as usize * 2;

/// Ce que le micro a entendu jusqu'ici (les champs publics de `MindcareMicro`)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mesures {
    /// Le niveau du son, de 0 à 1 (pour l'animation)
    pub niveau: f32,
    /// La durée enregistrée, en millisecondes
    pub millis: u32,
    /// On a entendu parler
    pub entendu: bool,
    /// Depuis combien de temps on s'est tu, en millisecondes
    pub millis_depuis_voix: u32,
}

/// L'analyse du son (`Analyse` de `MindcareMicro`) : le bruit de fond est mesuré sur les 3 premiers tampons (300 ms),
/// puis on entend parler quand le son dépasse trois fois ce bruit (et au moins 0,012).
#[derive(Clone, Debug, Default)]
pub struct Analyse {
    pub mesures: Mesures,
    fond: f64,
    tampons_fond: u32,
}

impl Analyse {
    /// Un tampon reçu du micro (échantillons de 16 bits, petit-boutiste)
    pub fn ajouter(&mut self, octets: &[u8]) {
        let n = octets.len();
        let compte = n / 2;
        let somme: f64 = octets.as_chunks::<2>().0.iter().map(|e| f64::from(i16::from_le_bytes(*e)).powi(2)).sum();
        let rms = if compte > 0 { (somme / compte as f64).sqrt() / 32768.0 } else { 0.0 };
        let ms = (compte as u64 * 1000 / u64::from(FREQUENCE)) as u32;
        let m = &mut self.mesures;
        m.millis = m.millis.saturating_add(ms);
        m.niveau = (rms * 8.0).min(1.0) as f32;
        if self.tampons_fond < 3 {
            self.fond = (self.fond * f64::from(self.tampons_fond) + rms) / f64::from(self.tampons_fond + 1);
            self.tampons_fond += 1;
            return;
        }
        let seuil = (self.fond * 3.0).max(0.012);
        if rms > seuil {
            m.entendu = true;
            m.millis_depuis_voix = 0;
        } else {
            m.millis_depuis_voix = m.millis_depuis_voix.saturating_add(ms);
        }
    }
}

/// Le fichier .wav (`SaveWav`) : l'en-tête RIFF de 44 octets, puis le son tel quel
pub fn wav(pcm: &[u8]) -> Vec<u8> {
    let taille = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36u32.saturating_add(taille)).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&FREQUENCE.to_le_bytes());
    w.extend_from_slice(&(FREQUENCE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&taille.to_le_bytes());
    w.extend_from_slice(pcm);
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tampon(valeur: i16) -> Vec<u8> {
        std::iter::repeat_n(valeur.to_le_bytes(), TAMPON / 2).flatten().collect()
    }

    #[test]
    fn bruit_de_fond_puis_voix_puis_silence() {
        let mut a = Analyse::default();
        for _ in 0..3 {
            a.ajouter(&tampon(100)); // bruit de fond : rms 0,003
        }
        assert_eq!(a.mesures.millis, 300);
        assert!(!a.mesures.entendu);
        a.ajouter(&tampon(300)); // 0,009 : sous 0,012
        assert!(!a.mesures.entendu);
        assert_eq!(a.mesures.millis_depuis_voix, 100);
        a.ajouter(&tampon(3000)); // 0,09 : on parle
        assert!(a.mesures.entendu);
        assert_eq!(a.mesures.millis_depuis_voix, 0);
        assert!((a.mesures.niveau - 0.7324).abs() < 0.001);
        for _ in 0..11 {
            a.ajouter(&tampon(0));
        }
        assert_eq!(a.mesures.millis_depuis_voix, 1100);
        assert_eq!(a.mesures.millis, 1600);
        assert_eq!(a.mesures.niveau, 0.0);
    }

    #[test]
    fn fond_bruyant_releve_le_seuil() {
        let mut a = Analyse::default();
        for _ in 0..3 {
            a.ajouter(&tampon(1000)); // fond 0,0305 : seuil 0,0916
        }
        a.ajouter(&tampon(2500)); // 0,076 : encore du bruit
        assert!(!a.mesures.entendu);
        a.ajouter(&tampon(3500));
        assert!(a.mesures.entendu);
    }

    #[test]
    fn en_tete_wav() {
        let w = wav(&[1, 2, 3, 4]);
        assert_eq!(w.len(), 48);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 40);
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u32::from_le_bytes(w[28..32].try_into().unwrap()), 32_000);
        assert_eq!(u16::from_le_bytes(w[34..36].try_into().unwrap()), 16);
        assert_eq!(&w[36..40], b"data");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 4);
        assert_eq!(&w[44..], &[1, 2, 3, 4]);
    }
}
