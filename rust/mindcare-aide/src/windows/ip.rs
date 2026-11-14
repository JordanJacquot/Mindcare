//! `ip.renouveler` : les cartes réseau lues par l'API IP Helper (`GetAdaptersAddresses`, IPv4), puis l'adresse rendue
//! (`IpReleaseAddress`) et redemandée à la box (`IpRenewAddress`). Jamais `ipconfig` (sa sortie change avec la
//! langue). Le canal de l'Aide est local : il n'est pas coupé quand internet l'est.

use mindcare_aide_regles::cartes::Carte;
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
    GetAdaptersAddresses, GetInterfaceInfo, IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_DHCP_ENABLED, IP_ADAPTER_INDEX_MAP,
    IP_INTERFACE_INFO, IpReleaseAddress, IpRenewAddress,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

/// ERROR_BUFFER_OVERFLOW / ERROR_INSUFFICIENT_BUFFER : le tampon est trop petit, Windows donne la bonne taille
const TROP_PETIT: [u32; 2] = [111, 122];

/// Une adresse IPv4 de Windows en texte (vide si ce n'est pas une adresse IPv4)
///
/// SAFETY (appelant) : `p` est nul, ou pointe sur une adresse de `longueur` octets rendue par Windows
unsafe fn ipv4(p: *const windows::Win32::Networking::WinSock::SOCKADDR, longueur: i32) -> String {
    if p.is_null() || (longueur as usize) < size_of::<SOCKADDR_IN>() {
        return String::new();
    }
    // SAFETY : voir plus haut ; la famille est lue avant le reste
    unsafe {
        if (*p).sa_family != AF_INET {
            return String::new();
        }
        let a = &*(p as *const SOCKADDR_IN);
        Ipv4Addr::from(u32::from_be(a.sin_addr.S_un.S_addr)).to_string()
    }
}

/// Les cartes réseau, avec leurs adresses IPv4
pub fn cartes() -> Result<Vec<Carte>, String> {
    let options =
        GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut taille = 16 * 1024u32;
    for _ in 0..4 {
        let mut b = vec![0u64; (taille as usize).div_ceil(8)];
        let p = b.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        // SAFETY : tampon aligné de `taille` octets ; Windows y écrit la liste chaînée des cartes
        let r = unsafe { GetAdaptersAddresses(u32::from(AF_INET.0), options, None, Some(p), &mut taille) };
        if TROP_PETIT.contains(&r) {
            continue;
        }
        if r != 0 {
            return Err(format!("GetAdaptersAddresses : erreur {r}"));
        }
        let mut cartes = Vec::new();
        let mut c = p as *const IP_ADAPTER_ADDRESSES_LH;
        // SAFETY : la liste chaînée vit dans `b`, gardé jusqu'à la fin de la lecture ; chaque pointeur vient de Windows
        unsafe {
            while !c.is_null() {
                let a = &*c;
                let mut adresses = Vec::new();
                let mut u = a.FirstUnicastAddress;
                while !u.is_null() {
                    let t = ipv4((*u).Address.lpSockaddr, (*u).Address.iSockaddrLength);
                    if !t.is_empty() {
                        adresses.push(t);
                    }
                    u = (*u).Next;
                }
                let mut passerelle = false;
                let mut g = a.FirstGatewayAddress;
                while !g.is_null() {
                    passerelle |= !ipv4((*g).Address.lpSockaddr, (*g).Address.iSockaddrLength).is_empty();
                    g = (*g).Next;
                }
                cartes.push(Carte {
                    index: a.Anonymous1.Anonymous.IfIndex,
                    nom: a.FriendlyName.to_string().unwrap_or_default(),
                    description: a.Description.to_string().unwrap_or_default(),
                    genre: a.IfType,
                    active: a.OperStatus == IfOperStatusUp,
                    dhcp: a.Anonymous2.Flags & IP_ADAPTER_DHCP_ENABLED != 0,
                    passerelle,
                    adresses,
                });
                c = a.Next;
            }
        }
        return Ok(cartes);
    }
    Err("GetAdaptersAddresses : liste trop grande".into())
}

/// La carte de ce numéro, telle que IpReleaseAddress et IpRenewAddress la veulent (`GetInterfaceInfo`)
fn carte(index: u32) -> Result<IP_ADAPTER_INDEX_MAP, String> {
    let mut taille = 0u32;
    // SAFETY : premier appel pour connaître la taille, second avec un tampon aligné de cette taille ; la liste est lue
    // dans ses limites (NumAdapters)
    unsafe {
        let r = GetInterfaceInfo(None, &mut taille);
        if r != 0 && !TROP_PETIT.contains(&r) {
            return Err(format!("GetInterfaceInfo : erreur {r}"));
        }
        let mut b = vec![0u64; (taille as usize).div_ceil(8).max(1)];
        let p = b.as_mut_ptr() as *mut IP_INTERFACE_INFO;
        let r = GetInterfaceInfo(Some(p), &mut taille);
        if r != 0 {
            return Err(format!("GetInterfaceInfo : erreur {r}"));
        }
        let n = usize::try_from((*p).NumAdapters).unwrap_or(0);
        std::slice::from_raw_parts((*p).Adapter.as_ptr(), n)
            .iter()
            .find(|m| m.Index == index)
            .copied()
            .ok_or_else(|| "carte disparue".to_string())
    }
}

/// Rend l'adresse de cette carte à la box
pub fn rendre(index: u32) -> Result<(), String> {
    let m = carte(index)?;
    // SAFETY : la carte telle que Windows l'a donnée
    let r = unsafe { IpReleaseAddress(&m) };
    if r == 0 { Ok(()) } else { Err(format!("IpReleaseAddress : erreur {r}")) }
}

/// Redemande une adresse à la box pour cette carte, en attendant au plus `limite` (au-delà, l'appel de Windows est
/// laissé finir seul dans son fil, et la réponse dit que la box n'a pas répondu)
pub fn redemander(index: u32, limite: Duration) -> Result<(), String> {
    let m = carte(index)?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // SAFETY : la carte telle que Windows l'a donnée, copiée dans ce fil
        let r = unsafe { IpRenewAddress(&m) };
        let _ = tx.send(r);
    });
    match rx.recv_timeout(limite) {
        Ok(0) => Ok(()),
        Ok(r) => Err(format!("IpRenewAddress : erreur {r}")),
        Err(_) => Err(format!("pas de réponse de la box en {} s", limite.as_secs())),
    }
}
