#[cfg(target_os = "linux")]
use anyhow::anyhow;
#[cfg(target_os = "linux")]
use log::{debug, error};
#[cfg(target_os = "linux")]
use nix::errno::Errno;
#[cfg(target_os = "linux")]
use nix::NixPath;

#[cfg(target_os = "linux")]
use crate::error::Error;
use crate::Result;

#[cfg(target_os = "linux")]
pub async fn cleanup_mounts(parent_dir: &str) -> Result<()> {
    let parent_dir = if parent_dir.is_empty() {
        "."
    } else {
        parent_dir
    };
    let mounts = tokio::fs::read_to_string("/proc/mounts")
        .await
        .map_err(Error::IO)?;
    for line in mounts.lines() {
        let fields = line.split_whitespace().collect::<Vec<&str>>();
        if fields.len() < 2 {
            continue;
        }
        let path = fields[1];
        if path.starts_with(parent_dir) {
            unmount(path, libc::MNT_DETACH | libc::UMOUNT_NOFOLLOW).unwrap_or_else(|e| {
                error!("failed to remove {}, err: {}", path, e);
            });
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn unmount(target: &str, flags: i32) -> Result<()> {
    let res = target
        .with_nix_path(|cstr| unsafe { libc::umount2(cstr.as_ptr(), flags) })
        .map_err(|e| anyhow!("failed to umount {}, {}", target, e))?;
    let err = Errno::result(res).map(drop);
    match err {
        Ok(_) => Ok(()),
        Err(e) => {
            if e == Errno::ENOENT {
                debug!("the umount path {} not exist", target);
                return Ok(());
            }

            Err(anyhow!("failed to umount {}, {}", target, e).into())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn cleanup_mounts(_parent_dir: &str) -> Result<()> {
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn unmount(_target: &str, _flags: i32) -> Result<()> {
    Ok(())
}
