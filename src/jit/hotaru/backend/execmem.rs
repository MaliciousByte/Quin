// ─────────────────────────────────────────────────────────────────────────────
// Executable Memory Allocation — Cross-platform
//
// Allocates read-write-execute memory for JIT-compiled code.
// Windows: VirtualAlloc with PAGE_EXECUTE_READWRITE
// Unix:    mmap with PROT_READ | PROT_WRITE | PROT_EXEC
// ─────────────────────────────────────────────────────────────────────────────

/// Copy `code` into a freshly allocated executable memory region.
/// Returns a pointer to the start of the executable code.
/// The caller is responsible for freeing the memory with `free_executable`.
pub fn alloc_executable(code: &[u8]) -> *const u8 {
    let size = code.len();
    if size == 0 {
        return std::ptr::null();
    }

    #[cfg(target_os = "windows")]
    {
        alloc_executable_windows(code, size)
    }

    #[cfg(not(target_os = "windows"))]
    {
        alloc_executable_unix(code, size)
    }
}

#[cfg(target_os = "windows")]
fn alloc_executable_windows(code: &[u8], size: usize) -> *const u8 {
    use std::ptr;

    // Windows constants
    const MEM_COMMIT: u32 = 0x1000;
    const MEM_RESERVE: u32 = 0x2000;
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;

    extern "system" {
        fn VirtualAlloc(
            lpAddress: *mut u8,
            dwSize: usize,
            flAllocationType: u32,
            flProtect: u32,
        ) -> *mut u8;
    }

    unsafe {
        let mem = VirtualAlloc(
            ptr::null_mut(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        );
        if mem.is_null() {
            return ptr::null();
        }
        ptr::copy_nonoverlapping(code.as_ptr(), mem, size);
        mem as *const u8
    }
}

#[cfg(not(target_os = "windows"))]
fn alloc_executable_unix(code: &[u8], size: usize) -> *const u8 {
    use std::ptr;

    // POSIX constants
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const PROT_EXEC: i32 = 4;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_FAILED: *mut u8 = !0 as *mut u8;

    extern "C" {
        fn mmap(
            addr: *mut u8,
            length: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut u8;
    }

    unsafe {
        let mem = mmap(
            ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE | PROT_EXEC,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        );
        if mem == MAP_FAILED {
            return ptr::null();
        }
        ptr::copy_nonoverlapping(code.as_ptr(), mem, size);
        mem as *const u8
    }
}

/// Free executable memory allocated by `alloc_executable`.
///
/// # Safety
/// `ptr` must have been returned by `alloc_executable` and `size` must match
/// the original code length.
#[allow(dead_code)]
pub unsafe fn free_executable(ptr: *const u8, size: usize) {
    if ptr.is_null() {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        const MEM_RELEASE: u32 = 0x8000;
        extern "system" {
            fn VirtualFree(lpAddress: *mut u8, dwSize: usize, dwFreeType: u32) -> i32;
        }
        VirtualFree(ptr as *mut u8, 0, MEM_RELEASE);
        let _ = size; // size not needed for MEM_RELEASE
    }

    #[cfg(not(target_os = "windows"))]
    {
        extern "C" {
            fn munmap(addr: *mut u8, length: usize) -> i32;
        }
        munmap(ptr as *mut u8, size);
    }
}
