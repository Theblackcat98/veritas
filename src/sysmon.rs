use std::fs;
use std::path::{Path, PathBuf};

use sysinfo::System;

/// Real telemetry for the sidebar. Raw counters, not ratios, so titles can
/// show true numbers (see D010). `vram = None` means "no usable GPU" and
/// the VRAM/GPU widgets show n/a in place (rule 9).
#[derive(Debug, Clone)]
pub struct SysStats {
    pub ram_used: u64,            // bytes
    pub ram_total: u64,           // bytes
    pub vram: Option<(u64, u64)>, // (used, total) bytes
    pub gpu_history: Vec<u64>,    // percent 0..100
    pub ctx_used: u64,            // tokens
}

impl SysStats {
    pub fn new() -> Self {
        Self {
            ram_used: 0,
            ram_total: 0,
            vram: None,
            gpu_history: Vec::new(),
            ctx_used: 0,
        }
    }
}

/// Owns the sysinfo handle and the amdgpu sysfs path, refreshing a
/// `SysStats` in place. Sampled inline from the TUI loop every 500 ms (D010)
/// — no thread, no locks.
pub struct Sampler {
    sys: System,
    gpu: Option<PathBuf>, // /sys/class/drm/cardN/device
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            sys: System::new(),
            gpu: find_amdgpu(),
        }
    }

    pub fn sample(&mut self, stats: &mut SysStats) {
        self.sys.refresh_memory();
        stats.ram_used = self.sys.used_memory();
        stats.ram_total = self.sys.total_memory();

        let Some(dev) = self.gpu.as_ref() else {
            stats.vram = None;
            stats.gpu_history.clear();
            return;
        };
        let vram_used = read_u64(&dev.join("mem_info_vram_used"));
        let vram_total = read_u64(&dev.join("mem_info_vram_total"));
        let busy = read_percent(&dev.join("gpu_busy_percent"));
        match (vram_used, vram_total) {
            (Some(used), Some(total)) if total > 0 => {
                stats.vram = Some((used, total));
                stats.gpu_history.push(busy.unwrap_or(0).min(100));
                if stats.gpu_history.len() > 60 {
                    stats.gpu_history.remove(0);
                }
            }
            _ => {
                stats.vram = None;
                stats.gpu_history.clear();
            }
        }
    }
}

/// Locate an AMD GPU through DRM sysfs. AMD vendor id is `0x1002`. Among
/// multiple AMD cards the one with the largest reported VRAM wins, so a
/// discrete GPU beats an iGPU carve-out. None on other setups.
fn find_amdgpu() -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let entries = fs::read_dir("/sys/class/drm").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(idx) = name.strip_prefix("card") else {
            continue;
        };
        if idx.is_empty() || !idx.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let dev = entry.path().join("device");
        let vendor = fs::read_to_string(dev.join("vendor")).unwrap_or_default();
        if vendor.trim() != "0x1002" {
            continue;
        }
        let total = read_u64(&dev.join("mem_info_vram_total")).unwrap_or(0);
        if best.as_ref().is_none_or(|(t, _)| total > *t) {
            best = Some((total, dev));
        }
    }
    best.filter(|(total, _)| *total > 0).map(|(_, dev)| dev)
}

fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse::<u64>().ok()
}

/// gpu_busy_percent may render as "42" or "42 %"; take the numeric token.
fn read_percent(path: &Path) -> Option<u64> {
    let s = fs::read_to_string(path).ok()?;
    s.split_whitespace().next()?.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_never_crashes_and_hides_gpu_without_amdgpu() {
        let mut sampler = Sampler::new();
        let mut stats = SysStats::new();
        sampler.sample(&mut stats);
        // RAM should be real on any machine with a working OS.
        assert!(stats.ram_total > 0);
        assert!(stats.ram_used <= stats.ram_total);
        if sampler.gpu.is_none() {
            assert!(stats.vram.is_none());
            assert!(stats.gpu_history.is_empty());
        }
    }
}
