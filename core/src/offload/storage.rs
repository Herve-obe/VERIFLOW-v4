//! Supports de stockage : liste des volumes, détection des disques durs
//! mécaniques (avertissement de lenteur) et éjection sécurisée (charte §7.1).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{DiskKind, Disks};

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct Volume {
    pub mount_point: PathBuf,
    pub name: String,
    pub file_system: String,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
    /// "ssd", "hdd" ou "unknown".
    pub kind: &'static str,
}

fn kind_str(k: DiskKind) -> &'static str {
    match k {
        DiskKind::SSD => "ssd",
        DiskKind::HDD => "hdd",
        _ => "unknown",
    }
}

/// Nom lisible d'un volume : nom du dossier de montage (/Volumes/CARTE, /media/CARTE),
/// sinon étiquette du disque, sinon point de montage. Windows : « Étiquette (C:) ».
fn display_name(mount: &Path, label: &str) -> String {
    let label = label.trim();
    if cfg!(windows) {
        let letter = mount
            .display()
            .to_string()
            .trim_end_matches(['\\', '/'])
            .to_owned();
        return if label.is_empty() {
            format!("Disque local ({letter})")
        } else {
            format!("{label} ({letter})")
        };
    }
    if let Some(f) = mount
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .filter(|f| !f.is_empty())
    {
        return f;
    }
    if !label.is_empty() && !label.starts_with("/dev/") {
        return label.to_owned();
    }
    if mount == Path::new("/") {
        return "Système (/)".to_owned();
    }
    mount.display().to_string()
}

/// Éjection en cours : la liste des volumes n'est plus relue (sysinfo ouvre
/// chaque volume pour en lire la taille et le type, ce qui peut faire
/// refuser l'éjection par Windows), la dernière lecture est rendue.
static EJECTING: AtomicUsize = AtomicUsize::new(0);
static LAST_VOLUMES: Mutex<Vec<Volume>> = Mutex::new(Vec::new());

struct Ejecting;
impl Ejecting {
    fn start() -> Self {
        EJECTING.fetch_add(1, Ordering::SeqCst);
        Ejecting
    }
}
impl Drop for Ejecting {
    fn drop(&mut self) {
        EJECTING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Volumes montés, hors volumes système virtuels.
pub fn volumes() -> Vec<Volume> {
    if EJECTING.load(Ordering::SeqCst) > 0 {
        if let Ok(last) = LAST_VOLUMES.lock() {
            return last.clone();
        }
    }
    let list = read_volumes();
    if let Ok(mut last) = LAST_VOLUMES.lock() {
        *last = list.clone();
    }
    list
}

fn read_volumes() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    let mut out: Vec<Volume> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .filter(|d| {
            let fs = d.file_system().to_string_lossy().to_lowercase();
            !["tmpfs", "devtmpfs", "overlay", "squashfs", "proc", "sysfs"].contains(&fs.as_str())
        })
        .map(|d| Volume {
            mount_point: d.mount_point().to_path_buf(),
            name: display_name(d.mount_point(), &d.name().to_string_lossy()),
            file_system: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            available: d.available_space(),
            removable: d.is_removable() || external_bus(d.mount_point()),
            kind: kind_str(d.kind()),
        })
        .collect();
    out.sort_by(|a, b| {
        b.removable
            .cmp(&a.removable)
            .then(a.mount_point.cmp(&b.mount_point))
    });
    out
}

/// Retire le préfixe `\\?\` que `canonicalize` ajoute sous Windows
/// (`\\?\E:\A001` devient `E:\A001`) : sinon le chemin ne commence pas par
/// le point de montage `E:\` et son volume n'est pas trouvé.
fn without_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// Volume contenant un chemin (point de montage le plus long qui le précède).
pub fn volume_of(path: &Path) -> Option<Volume> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = without_verbatim(&path);
    volumes()
        .into_iter()
        .filter(|v| path.starts_with(&v.mount_point))
        .max_by_key(|v| v.mount_point.as_os_str().len())
}

/// Vrai si le chemin est sur un disque dur mécanique (copie plus lente).
pub fn is_hdd(path: &Path) -> bool {
    volume_of(path).is_some_and(|v| v.kind == "hdd")
}

/// Disque externe branché en USB, FireWire ou lecteur de cartes (Windows).
/// Windows ne déclare « amovibles » que les clés et les cartes : un disque dur
/// ou un SSD USB est un disque « fixe », que l'Explorateur sait pourtant
/// éjecter. Le résultat est gardé en mémoire : la liste des volumes est relue
/// toutes les 2 s et chaque lecture ouvre le volume, ce qui gênerait une
/// éjection en cours.
#[cfg(windows)]
fn external_bus(mount: &Path) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        BusType1394, BusTypeMmc, BusTypeSd, BusTypeUsb, CreateFileW, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY,
        STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    static CACHE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(known) = cache.lock().ok().and_then(|c| c.get(mount).copied()) {
        return known;
    }
    let letter = mount.display().to_string();
    let letter = letter.trim_end_matches(['\\', '/']);
    // Le disque du système n'est jamais éjecté, même installé sur un disque USB.
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let external = !letter.eq_ignore_ascii_case(&system) && {
        let device: Vec<u16> = format!(r"\\.\{letter}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY : appels Win32 documentés ; tampons et tailles cohérents,
        // descripteur relu sans hypothèse d'alignement, handle refermé.
        unsafe {
            let h = CreateFileW(
                device.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                false
            } else {
                let query = STORAGE_PROPERTY_QUERY {
                    PropertyId: StorageDeviceProperty,
                    QueryType: PropertyStandardQuery,
                    AdditionalParameters: [0],
                };
                let mut buf = [0u8; 1024];
                let mut len = 0u32;
                let ok = DeviceIoControl(
                    h,
                    IOCTL_STORAGE_QUERY_PROPERTY,
                    (&query as *const STORAGE_PROPERTY_QUERY).cast(),
                    std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                    buf.as_mut_ptr().cast(),
                    buf.len() as u32,
                    &mut len,
                    std::ptr::null_mut(),
                );
                CloseHandle(h);
                if ok == 0 || (len as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
                    false
                } else {
                    let d: STORAGE_DEVICE_DESCRIPTOR =
                        std::ptr::read_unaligned(buf.as_ptr().cast());
                    d.RemovableMedia
                        || [BusTypeUsb, BusType1394, BusTypeSd, BusTypeMmc].contains(&d.BusType)
                }
            }
        }
    };
    if let Ok(mut c) = cache.lock() {
        c.insert(mount.to_path_buf(), external);
    }
    external
}

/// Ailleurs, le système déclare lui-même les disques externes amovibles.
#[cfg(not(windows))]
fn external_bus(_mount: &Path) -> bool {
    false
}

/// Programmes qui tiennent ouverts des fichiers de ce dossier (Windows,
/// gestionnaire de redémarrage : le même service que l'installeur de
/// Windows utilise pour dire « fermez tel programme »). 2 000 fichiers au
/// plus ; vide ailleurs ou si rien n'est trouvé.
#[cfg(windows)]
pub fn processes_using(dir: &Path) -> Vec<String> {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
        RM_PROCESS_INFO,
    };

    let mut files: Vec<Vec<u16>> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            } else if files.len() < 2000 {
                use std::os::windows::ffi::OsStrExt;
                files.push(p.as_os_str().encode_wide().chain(Some(0)).collect());
            }
        }
        if files.len() >= 2000 {
            break;
        }
    }
    if files.is_empty() {
        return Vec::new();
    }
    let ptrs: Vec<*const u16> = files.iter().map(|f| f.as_ptr()).collect();
    let mut names: Vec<String> = Vec::new();
    // SAFETY : appels documentés du gestionnaire de redémarrage ; les
    // chaînes restent en vie pendant la session, refermée dans tous les cas.
    unsafe {
        let mut session = 0u32;
        let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
        if RmStartSession(&mut session, 0, key.as_mut_ptr()) != ERROR_SUCCESS {
            return Vec::new();
        }
        if RmRegisterResources(
            session,
            ptrs.len() as u32,
            ptrs.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        ) == ERROR_SUCCESS
        {
            let mut needed = 0u32;
            let mut count = 0u32;
            let mut reasons = 0u32;
            let mut list: Vec<RM_PROCESS_INFO> = Vec::new();
            let mut status = RmGetList(
                session,
                &mut needed,
                &mut count,
                std::ptr::null_mut(),
                &mut reasons,
            );
            if status == ERROR_MORE_DATA && needed > 0 {
                list = vec![RM_PROCESS_INFO::default(); needed as usize];
                count = needed;
                status = RmGetList(
                    session,
                    &mut needed,
                    &mut count,
                    list.as_mut_ptr(),
                    &mut reasons,
                );
            }
            if status == ERROR_SUCCESS {
                for info in list.iter().take(count as usize) {
                    let raw = &info.strAppName;
                    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                    let name = String::from_utf16_lossy(&raw[..len]);
                    let name = format!("{name} (PID {})", info.Process.dwProcessId);
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        RmEndSession(session);
    }
    names
}

#[cfg(not(windows))]
pub fn processes_using(_dir: &Path) -> Vec<String> {
    Vec::new()
}

/// Éjection par le gestionnaire de périphériques (comme « Retirer le
/// périphérique en toute sécurité ») : le disque du volume est retrouvé par
/// son numéro, puis Windows est prié d'éjecter le périphérique, en remontant
/// vers le boîtier USB si le disque seul ne s'éjecte pas. `None` : disque
/// introuvable ; `Some(Err)` : refus, avec la raison donnée par Windows.
#[cfg(windows)]
fn pnp_eject(letter: &str) -> Option<std::result::Result<(), String>> {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_Parent, CM_Request_Device_EjectW, PNP_VetoIllegalDeviceRequest,
        PNP_VetoNonDisableable, PNP_VetoTypeUnknown, SetupDiDestroyDeviceInfoList,
        SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
        CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, PNP_VETO_TYPE, SP_DEVICE_INTERFACE_DATA,
        SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    // Numéro de disque d'un chemin de périphérique (volume ou disque).
    // SAFETY : appels Win32 documentés, handle refermé, tampon à la taille.
    let number = |path: &[u16]| -> Option<u32> {
        unsafe {
            let h = CreateFileW(
                path.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut n = STORAGE_DEVICE_NUMBER::default();
            let mut len = 0u32;
            let ok = DeviceIoControl(
                h,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                std::ptr::null(),
                0,
                (&mut n as *mut STORAGE_DEVICE_NUMBER).cast(),
                std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                &mut len,
                std::ptr::null_mut(),
            );
            CloseHandle(h);
            (ok != 0).then_some(n.DeviceNumber)
        }
    };
    let volume: Vec<u16> = format!(r"\\.\{letter}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let wanted = number(&volume)?;

    // SAFETY : énumération SetupAPI ; la liste est détruite dans tous les cas,
    // le tampon de détail est aligné (u32) et dimensionné par Windows.
    let devinst = unsafe {
        let set = SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_DISK,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == -1 {
            return None;
        }
        let mut found = None;
        let mut index = 0u32;
        loop {
            let mut iface = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(
                set,
                std::ptr::null(),
                &GUID_DEVINTERFACE_DISK,
                index,
                &mut iface,
            ) == 0
            {
                break;
            }
            index += 1;
            let mut needed = 0u32;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &iface,
                std::ptr::null_mut(),
                0,
                &mut needed,
                std::ptr::null_mut(),
            );
            if needed == 0 {
                continue;
            }
            let mut buf = vec![0u32; (needed as usize).div_ceil(4)];
            let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            let mut info = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &iface,
                detail,
                needed,
                std::ptr::null_mut(),
                &mut info,
            ) == 0
            {
                continue;
            }
            // Chemin terminé par un zéro, juste après cbSize.
            let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let max = (needed as usize - 4) / 2;
            let path = std::slice::from_raw_parts(start, max);
            let end = path.iter().position(|&c| c == 0).unwrap_or(max);
            let mut path = path[..end].to_vec();
            path.push(0);
            if number(&path) == Some(wanted) {
                found = Some(info.DevInst);
                break;
            }
        }
        SetupDiDestroyDeviceInfoList(set);
        found
    }?;

    // Le disque, puis ses parents (boîtier USB, concentrateur), deux niveaux.
    let mut node = devinst;
    let mut last = String::new();
    for _ in 0..3 {
        for _attempt in 0..3 {
            let mut veto: PNP_VETO_TYPE = PNP_VetoTypeUnknown;
            let mut name = [0u16; 260];
            // SAFETY : tampon de 260 caractères annoncé tel quel.
            let cr = unsafe {
                CM_Request_Device_EjectW(node, &mut veto, name.as_mut_ptr(), name.len() as u32, 0)
            };
            if cr == CR_SUCCESS && veto == PNP_VetoTypeUnknown {
                return Some(Ok(()));
            }
            let len = name.iter().position(|&c| c == 0).unwrap_or(0);
            let who = String::from_utf16_lossy(&name[..len]);
            if veto == PNP_VetoIllegalDeviceRequest || veto == PNP_VetoNonDisableable {
                break;
            }
            last = veto_text(veto, &who);
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        let mut parent = 0u32;
        // SAFETY : appel cfgmgr32 documenté.
        if unsafe { CM_Get_Parent(&mut parent, node, 0) } != CR_SUCCESS {
            break;
        }
        node = parent;
    }
    Some(Err(if last.is_empty() {
        "Windows refuse d'éjecter ce disque".to_owned()
    } else {
        last
    }))
}

/// Raison d'un refus d'éjection, d'après le type de veto de Windows.
#[allow(non_upper_case_globals)]
#[cfg(windows)]
fn veto_text(veto: i32, who: &str) -> String {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
    let why = match veto {
        PNP_VetoOutstandingOpen => "des fichiers ou dossiers du disque sont encore ouverts",
        PNP_VetoWindowsApp => "un programme refuse",
        PNP_VetoWindowsService => "un service de Windows refuse",
        PNP_VetoDevice | PNP_VetoDriver | PNP_VetoLegacyDevice | PNP_VetoLegacyDriver => {
            "un pilote refuse"
        }
        PNP_VetoPendingClose => "fermeture en cours, réessaie dans quelques secondes",
        PNP_VetoInsufficientRights => "droits insuffisants",
        PNP_VetoAlreadyRemoved => "le disque est déjà retiré",
        _ => "refus de Windows",
    };
    if who.is_empty() {
        why.to_owned()
    } else {
        format!("{why} ({who})")
    }
}

/// Sous Windows, le dossier courant d'un programme garde son volume ouvert.
/// Un sélecteur de fichiers peut l'avoir placé sur le disque à éjecter : on
/// le ramène dans le dossier temporaire.
fn leave_volume(mount_point: &Path) {
    if !cfg!(windows) {
        return;
    }
    let Ok(cwd) = std::env::current_dir() else {
        return;
    };
    let lower = |p: &Path| {
        p.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase()
    };
    let mp = lower(mount_point);
    if !mp.is_empty() && lower(&cwd).starts_with(mp.trim_end_matches(['\\', '/'])) {
        let _ = std::env::set_current_dir(std::env::temp_dir());
    }
}

/// Dernier refus d'éjection noté par Windows (journal Système, Kernel-PnP,
/// événement 225, deux dernières minutes) : il nomme le programme bloquant.
#[cfg(windows)]
fn eject_veto() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
        Get-WinEvent -MaxEvents 1 -ErrorAction SilentlyContinue -FilterHashtable \
        @{LogName='System'; ProviderName='Microsoft-Windows-Kernel-PnP'; Id=225; \
        StartTime=(Get-Date).AddMinutes(-2)} | ForEach-Object { $_.Message }";
    let out = Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

#[cfg(not(windows))]
fn eject_veto() -> Option<String> {
    None
}

/// Éjecte proprement un volume amovible (après la copie).
///
/// Les dossiers suivis par l'explorateur sur ce volume doivent être libérés
/// avant (sous Windows, chaque dossier surveillé garde le volume ouvert).
/// `hint` : dossier copié, dont les fichiers servent à nommer, en cas de
/// refus, les programmes qui les tiennent ouverts (Windows).
pub fn eject(mount_point: &Path, hint: Option<&Path>) -> Result<()> {
    let _ejecting = Ejecting::start();
    let mp = mount_point.display().to_string();
    leave_volume(mount_point);
    // Windows : retrait sécurisé par le gestionnaire de périphériques, qui
    // éjecte aussi les disques durs USB et donne la raison d'un refus. La
    // commande « Éjecter » de l'Explorateur ne sert plus que de secours.
    #[cfg(windows)]
    match pnp_eject(mp.trim_end_matches(['\\', '/'])) {
        Some(Ok(())) => return Ok(()),
        Some(Err(why)) => {
            let mut message = format!("Windows refuse l'éjection : {why}");
            let users = processes_using(hint.unwrap_or(mount_point));
            if !users.is_empty() {
                message.push_str(&format!(" ; fichiers ouverts par : {}", users.join(", ")));
            }
            if let Some(veto) = eject_veto() {
                message.push_str(&format!(". Journal de Windows : {veto}"));
            }
            return Err(Error::Tool {
                tool: "éjection".into(),
                message,
            });
        }
        None => {}
    }
    let output = if cfg!(target_os = "macos") {
        Command::new("diskutil").args(["eject", &mp]).output()
    } else if cfg!(windows) {
        // Même commande que « Éjecter » dans l'Explorateur (retrait sécurisé,
        // disques USB compris). Elle rend la main tout de suite sans dire si
        // l'éjection a réussi : on attend que la lettre disparaisse (5 s),
        // et on réessaie deux fois (un programme peut lâcher le disque
        // entre-temps : indexation, antivirus). GetLogicalDrives n'ouvre pas
        // le volume, contrairement à Test-Path.
        let letter = mp.trim_end_matches(['\\', '/']).to_owned();
        let script = format!(
            "$d='{letter}'; \
             $gone={{ -not ([IO.Directory]::GetLogicalDrives() -contains ($d + '\\')) }}; \
             for ($a=0; $a -lt 3; $a++) {{ \
               $i=(New-Object -ComObject Shell.Application).Namespace(17).ParseName($d); \
               if ($null -eq $i) {{ if (& $gone) {{ exit 0 }} else {{ exit 2 }} }}; \
               $i.InvokeVerb('Eject'); \
               for ($n=0; $n -lt 10; $n++) {{ \
                 if (& $gone) {{ exit 0 }}; \
                 Start-Sleep -Milliseconds 500 }} }}; \
             exit 1"
        );
        #[allow(unused_mut)]
        let mut cmd = Command::new("powershell");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
    } else {
        // Linux : démontage utilisateur (GNOME, KDE), sinon umount.
        Command::new("gio")
            .args(["mount", "--unmount"])
            .arg(&mp)
            .output()
            .and_then(|o| {
                if o.status.success() {
                    Ok(o)
                } else {
                    Command::new("umount").arg(&mp).output()
                }
            })
    };
    // Windows : messages écrits ici (la console PowerShell abîme les accents).
    let windows_message = |code: Option<i32>| match code {
        Some(1) => {
            let users = processes_using(hint.unwrap_or(mount_point));
            let mut msg = if users.is_empty() {
                "le disque est encore utilisé par un autre programme \
                 (Explorateur, antivirus, indexation, lecteur vidéo...)"
                    .to_owned()
            } else {
                format!("le disque est encore utilisé par : {}", users.join(", "))
            };
            // Le gestionnaire de redémarrage ne voit que les fichiers ouverts ;
            // Windows journalise le programme qui a refusé l'éjection, quel
            // que soit ce qu'il tient ouvert (dossier, volume).
            if let Some(veto) = eject_veto() {
                msg.push_str(&format!(". Windows indique : {veto}"));
            }
            Some(msg)
        }
        Some(2) => Some("volume introuvable".to_owned()),
        _ => None,
    };
    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) if cfg!(windows) && matches!(o.status.code(), Some(1) | Some(2)) => {
            Err(Error::Tool {
                tool: "éjection".into(),
                message: windows_message(o.status.code()).unwrap_or_default(),
            })
        }
        Ok(o) => Err(Error::Tool {
            tool: "éjection".into(),
            message: String::from_utf8_lossy(&o.stderr).trim().to_owned(),
        }),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_windows_verbatim_prefix() {
        assert_eq!(
            without_verbatim(Path::new(r"\\?\E:\A001")),
            PathBuf::from(r"E:\A001")
        );
        // Partage réseau en chemin étendu : laissé tel quel.
        let unc = Path::new(r"\\?\UNC\serveur\rushes");
        assert_eq!(without_verbatim(unc), unc.to_path_buf());
        assert_eq!(
            without_verbatim(Path::new("/Volumes/A001")),
            PathBuf::from("/Volumes/A001")
        );
    }

    #[test]
    fn lists_volumes_and_finds_the_one_of_a_path() {
        let dir = tempfile::tempdir().unwrap();
        // Dans un conteneur, la liste peut être vide : on vérifie seulement la cohérence.
        if let Some(v) = volume_of(dir.path()) {
            assert!(dir
                .path()
                .canonicalize()
                .unwrap()
                .starts_with(&v.mount_point));
            assert!(v.total >= v.available);
        }
        let _ = is_hdd(dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn readable_volume_names() {
        assert_eq!(display_name(Path::new("/"), "/dev/vda"), "Système (/)");
        assert_eq!(display_name(Path::new("/"), "Macintosh HD"), "Macintosh HD");
        assert_eq!(display_name(Path::new("/Volumes/A001"), "disk4s1"), "A001");
    }
}
