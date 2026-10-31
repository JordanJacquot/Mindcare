//! Ce que la lecture demande à Windows, et sous quelle forme : le trait [`SourceWindows`]. Le vrai Windows le remplit
//! dans le module `windows` (le seul qui lui parle) ; les tests le remplissent avec un faux Windows (les données des
//! juges). Les données sont brutes : toutes les règles de Mindcare (ce qui compte, les textes) sont dans
//! [`crate::indices`].
//!
//! Chaque lecture peut rater (`Err` avec la raison) : c'est le cas « mesure ratée » du constat 20, que les règles
//! distinguent d'une mesure qui n'a rien trouvé.

use crate::moment::Moment;

/// Une mesure : la valeur lue, ou la raison de l'échec.
pub type Mesure<T> = Result<T, String>;

/// Une requête au journal Système (`Get-WinEvent -FilterHashtable @{ LogName = 'System'; ... } -MaxEvents`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtre {
    /// ProviderName (sans tenir compte des majuscules).
    pub fournisseur: String,
    /// Id (vide : tous).
    pub ids: Vec<u32>,
    /// Level (vide : tous).
    pub niveaux: Vec<u8>,
    /// StartTime : les événements notés à cet instant ou après.
    pub depuis: Moment,
    /// MaxEvents : les plus récents d'abord, au plus autant.
    pub max: usize,
}

/// Un événement du journal Système, avec ce que les règles en lisent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evenement {
    pub id: u32,
    pub fournisseur: String,
    pub niveau: u8,
    /// TimeCreated, à l'heure locale.
    pub date: Moment,
    /// Le texte de l'événement (`.Message` ; vide si Windows ne sait pas l'écrire).
    pub message: String,
    /// Les valeurs de `.Properties`, chacune écrite comme PowerShell l'écrit (`"$($_.Value)"`).
    pub proprietes: Vec<String>,
}

/// Un disque du PC (Get-Disk, MSFT_Disk) : son numéro, et son bus (le nombre brut de Windows : 7 = USB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disque {
    pub numero: u32,
    pub bus: Option<u16>,
}

/// Une partition (Get-Partition, MSFT_Partition) : son disque, et sa lettre (le code du caractère ; 0 : aucune).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub disque: u32,
    pub lettre: u16,
}

/// Un disque physique (Get-PhysicalDisk, MSFT_PhysicalDisk) : son nom, son bus et sa santé (les nombres bruts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisquePhysique {
    pub nom: String,
    pub bus: Option<u16>,
    pub sante: Option<u16>,
}

/// La place du disque de Windows (Get-PSDrive : FreeSpace et Size de Win32_LogicalDisk), en octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub libre: u64,
    pub taille: u64,
}

/// La première batterie du rapport de Windows (`powercfg /batteryreport /xml`) : ses capacités telles qu'écrites
/// dans le rapport (DesignCapacity, FullChargeCapacity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batterie {
    pub origine: String,
    pub pleine: String,
}

/// Une barrette de mémoire (Win32_PhysicalMemory).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Barrette {
    /// Capacity, en octets (rien : inconnue).
    pub capacite: Option<u64>,
    /// Manufacturer.
    pub fabricant: String,
    /// PartNumber.
    pub reference: String,
    /// DeviceLocator.
    pub emplacement: String,
}

/// Le PC tel que Windows le décrit (Get-InfosPC) : les valeurs brutes, la mise en forme est ailleurs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InfosPc {
    /// Win32_ComputerSystem.Manufacturer.
    pub fabricant: String,
    /// Win32_ComputerSystem.Model.
    pub modele: String,
    /// Win32_OperatingSystem.Caption.
    pub systeme: String,
}

/// Ce que la lecture demande à Windows. Tout se lit sans droits administrateur et sans rien modifier.
pub trait SourceWindows: Send + Sync {
    /// La sonde : l'événement le plus ancien du journal Système (rien : journal vide ; `Err` : journal illisible).
    fn plus_ancien(&self) -> Mesure<Option<Moment>>;
    /// Les événements du journal Système qui répondent au filtre, les plus récents d'abord. Aucun : une liste vide
    /// (pas une erreur, contrairement à Get-WinEvent).
    fn evenements(&self, filtre: &Filtre) -> Mesure<Vec<Evenement>>;
    /// Get-Disk.
    fn disques(&self) -> Mesure<Vec<Disque>>;
    /// Get-Partition.
    fn partitions(&self) -> Mesure<Vec<Partition>>;
    /// Get-PhysicalDisk.
    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>>;
    /// Le lecteur de Windows (`$env:SystemDrive`, « C: »).
    fn lecteur_systeme(&self) -> String;
    /// La place de ce lecteur.
    fn place(&self, lecteur: &str) -> Mesure<Place>;
    /// La première batterie du rapport de Windows (rien : le rapport ne cite aucune batterie).
    fn batterie(&self) -> Mesure<Option<Batterie>>;
    /// Le dernier démarrage (Win32_OperatingSystem.LastBootUpTime), à l'heure locale.
    fn demarrage(&self) -> Mesure<Moment>;
    /// Les barrettes de mémoire (Win32_PhysicalMemory).
    fn barrettes(&self) -> Mesure<Vec<Barrette>>;
    /// Le modèle du PC et la version de Windows (Win32_ComputerSystem, Win32_OperatingSystem).
    fn infos_pc(&self) -> Mesure<InfosPc>;
}

/// L'heure locale du PC (Get-Date). Les tests en donnent une fausse.
pub trait Horloge: Send + Sync {
    fn maintenant(&self) -> Moment;
}
