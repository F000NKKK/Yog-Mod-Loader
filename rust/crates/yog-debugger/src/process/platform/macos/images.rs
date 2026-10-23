//! The task's loaded images, as dyld lists them for debuggers:
//! `TASK_DYLD_INFO` points at its `dyld_all_image_infos`, whose array holds
//! each image's Mach header address (its load base) and path.

use std::path::PathBuf;

use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_types::task_t;
use mach2::task::task_info;
use mach2::task_info::{TASK_DYLD_INFO, TASK_DYLD_INFO_COUNT, task_dyld_info};

use super::sys;
use crate::process::target::Module;

/// `dyld_all_image_infos`: its image count, then the array's address.
const COUNT_AT: u64 = 4;
const ARRAY_AT: u64 = 8;
/// `dyld_image_info`: load address, path address, modification date.
const IMAGE_SIZE: u64 = 24;
const PATH_AT: u64 = 8;
/// A path is read this much at a time (never across a page: the next may
/// not be mapped), up to the longest one there is.
const PATH_CHUNK: usize = 256;
const PAGE: u64 = 4096;
const MAX_PATH: usize = 4096;

fn word(task: task_t, addr: u64) -> Option<u64> {
    let mut buf = [0u8; 8];
    sys::read(task, addr, &mut buf).then(|| u64::from_le_bytes(buf))
}

/// The NUL-terminated string at `addr`.
fn c_string(task: task_t, addr: u64) -> Option<String> {
    let mut out = Vec::new();
    while out.len() < MAX_PATH {
        let mut chunk = [0u8; PATH_CHUNK];
        let at = addr + out.len() as u64;
        let len = PATH_CHUNK.min((PAGE - at % PAGE) as usize);
        if !sys::read(task, at, &mut chunk[..len]) {
            return None;
        }
        match chunk[..len].iter().position(|&b| b == 0) {
            Some(end) => {
                out.extend_from_slice(&chunk[..end]);
                return Some(String::from_utf8_lossy(&out).into_owned());
            }
            None => out.extend_from_slice(&chunk[..len]),
        }
    }
    None
}

/// The images loaded into `task`.
pub(super) fn modules(task: task_t) -> Vec<Module> {
    // SAFETY: a zeroed info struct is valid for the call to fill.
    let mut info: task_dyld_info = unsafe { std::mem::zeroed() };
    let mut count = TASK_DYLD_INFO_COUNT;
    // SAFETY: `info` is the flavor's struct, `count` its size in words.
    let kr = unsafe {
        task_info(
            task,
            TASK_DYLD_INFO,
            (&mut info as *mut task_dyld_info).cast(),
            &mut count,
        )
    };
    let infos = info.all_image_info_addr;
    if kr != KERN_SUCCESS || infos == 0 {
        return Vec::new();
    }
    let mut header = [0u8; 4];
    let images = sys::read(task, infos + COUNT_AT, &mut header).then(|| u32::from_le_bytes(header));
    let (Some(images), Some(array)) = (images, word(task, infos + ARRAY_AT)) else {
        return Vec::new();
    };
    (0..u64::from(images))
        .filter_map(|i| {
            let at = array + i * IMAGE_SIZE;
            let base = word(task, at)?;
            let path = c_string(task, word(task, at + PATH_AT)?)?;
            Some(Module {
                base,
                path: PathBuf::from(path),
            })
        })
        .collect()
}
