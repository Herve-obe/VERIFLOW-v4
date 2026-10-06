//! Accès disque pour la vérification : relire les données sur le support
//! physique et non dans le cache mémoire du système, sinon la « vérification »
//! ne ferait que relire ce qui vient d'être écrit en mémoire.

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Alignement requis par les lectures non bufferisées (taille de secteur).
const ALIGN: usize = 4096;

/// Tampon aligné sur 4 Kio (exigé par FILE_FLAG_NO_BUFFERING sous Windows).
pub struct AlignedBuf {
    ptr: *mut u8,
    len: usize,
}

// Le tampon est un bloc mémoire exclusif : il peut changer de fil.
unsafe impl Send for AlignedBuf {}

impl AlignedBuf {
    pub fn new(len: usize) -> Self {
        let len = len.div_ceil(ALIGN) * ALIGN;
        let layout = Layout::from_size_align(len, ALIGN).expect("taille valide");
        // SAFETY : taille non nulle et alignement puissance de deux.
        let ptr = unsafe { alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "mémoire insuffisante");
        Self { ptr, len }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY : bloc alloué de `len` octets, possédé par cette structure.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY : même disposition qu'à l'allocation.
        unsafe { dealloc(self.ptr, Layout::from_size_align_unchecked(self.len, ALIGN)) }
    }
}

/// Ouvre un fichier en lecture en contournant le cache du système quand c'est possible.
pub fn open_uncached(path: &Path) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN)
            .open(path)
    }
    #[cfg(not(windows))]
    {
        let file = File::open(path)?;
        drop_cache(&file);
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::io::AsRawFd;
            // SAFETY : descripteur valide, F_NOCACHE désactive le cache pour ce fichier.
            unsafe {
                libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1);
            }
        }
        Ok(file)
    }
}

/// Demande au système d'oublier les pages en cache d'un fichier (après fsync).
pub fn drop_cache(file: &File) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        // SAFETY : descripteur valide ; simple conseil au noyau.
        unsafe {
            libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = file;
}

/// Lit jusqu'à remplir `buf` (ou fin de fichier). Renvoie le nombre d'octets lus.
pub fn read_full(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}
