//! `ip.renouveler` : les cartes réseau dont l'Aide rend puis redemande l'adresse à la box. Seulement les cartes
//! allumées et connectées, réglées en automatique (DHCP), avec une passerelle (la box), filaires ou Wi-Fi ; jamais une
//! carte à adresse fixe, une carte virtuelle (machines virtuelles, partage de connexion de Hyper-V) ni un VPN.

use crate::texte;

/// Ce que Windows dit d'une carte réseau (IPv4)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Carte {
    /// Son numéro (IfIndex)
    pub index: u32,
    /// Son nom (« Ethernet », « Wi-Fi »)
    pub nom: String,
    /// Sa description (« Intel(R) Ethernet Connection I219-V »)
    pub description: String,
    /// Son genre (IfType : 6 filaire, 71 Wi-Fi, 23 PPP, 24 bouclage, 131 tunnel...)
    pub genre: u32,
    /// Allumée et connectée (OperStatus « Up »)
    pub active: bool,
    /// Réglée en automatique (DHCP)
    pub dhcp: bool,
    /// Elle a une passerelle (la box)
    pub passerelle: bool,
    /// Ses adresses IPv4
    pub adresses: Vec<String>,
}

/// Les genres de cartes pris : filaire (Ethernet) et Wi-Fi
pub const GENRES: [u32; 2] = [6, 71];

/// Les mots qui trahissent une carte virtuelle ou un VPN (dans la description, sans tenir compte des majuscules)
pub const VIRTUELLES: [&str; 12] = [
    "Virtual Ethernet",
    "VirtualBox",
    "VMware Virtual",
    "VMnet",
    "Host-Only",
    "TAP-",
    "WireGuard",
    "Wintun",
    "ZeroTier",
    "Tailscale",
    "Npcap",
    "Loopback",
];

/// La carte est-elle à renouveler ?
pub fn a_renouveler(c: &Carte) -> bool {
    c.active
        && c.dhcp
        && c.passerelle
        && GENRES.contains(&c.genre)
        && !VIRTUELLES.iter().any(|v| texte::contient(&c.description, v) || texte::contient(&c.nom, v))
}

/// Une adresse que Windows se donne quand la box ne répond pas (169.254.x.x) : pas d'internet
pub fn adresse_de_secours(ip: &str) -> bool {
    ip.starts_with("169.254.")
}

/// La carte a-t-elle une vraie adresse donnée par la box ?
pub fn adresse_recue(adresses: &[String]) -> bool {
    adresses.iter().any(|a| !a.is_empty() && !adresse_de_secours(a) && a != "0.0.0.0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carte(nom: &str, description: &str, genre: u32) -> Carte {
        Carte {
            index: 7,
            nom: nom.into(),
            description: description.into(),
            genre,
            active: true,
            dhcp: true,
            passerelle: true,
            adresses: vec!["192.168.1.20".into()],
        }
    }

    #[test]
    fn seulement_les_vraies_cartes_en_automatique() {
        assert!(a_renouveler(&carte("Ethernet", "Intel(R) Ethernet Connection I219-V", 6)));
        assert!(a_renouveler(&carte("Wi-Fi", "Intel(R) Wi-Fi 6 AX201 160MHz", 71)));
        // (la carte d'une machine virtuelle de Azure, celle de GitHub : une vraie carte pour cette machine)
        assert!(a_renouveler(&carte("Ethernet", "Microsoft Hyper-V Network Adapter", 6)));
        for (nom, d) in [
            ("vEthernet (Default Switch)", "Hyper-V Virtual Ethernet Adapter"),
            ("Ethernet 3", "VirtualBox Host-Only Ethernet Adapter"),
            ("VMware Network Adapter VMnet8", "VMware Virtual Ethernet Adapter for VMnet8"),
            ("OpenVPN", "TAP-Windows Adapter V9"),
            ("wg0", "WireGuard Tunnel"),
            ("Ethernet 5", "ZeroTier Virtual Port"),
        ] {
            assert!(!a_renouveler(&carte(nom, d, 6)), "{d}");
        }
        assert!(!a_renouveler(&carte("VPN", "WAN Miniport (PPTP)", 23)));
        assert!(!a_renouveler(&carte("Bouclage", "Software Loopback Interface 1", 24)));
        let mut c = carte("Ethernet", "Realtek PCIe GbE", 6);
        c.dhcp = false;
        assert!(!a_renouveler(&c), "adresse fixe");
        let mut c = carte("Ethernet", "Realtek PCIe GbE", 6);
        c.passerelle = false;
        assert!(!a_renouveler(&c), "sans box");
        let mut c = carte("Ethernet", "Realtek PCIe GbE", 6);
        c.active = false;
        assert!(!a_renouveler(&c), "débranchée");
    }

    #[test]
    fn adresses() {
        assert!(adresse_de_secours("169.254.10.3"));
        assert!(!adresse_de_secours("192.168.1.2"));
        assert!(adresse_recue(&["169.254.1.1".into(), "10.0.0.4".into()]));
        assert!(!adresse_recue(&["169.254.1.1".into()]));
        assert!(!adresse_recue(&["0.0.0.0".into()]));
        assert!(!adresse_recue(&[]));
    }
}
