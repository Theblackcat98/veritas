use sysinfo::System;

#[cfg(target_os = "linux")]
use nvml_wrapper::Nvml;

/// Real telemetry for the sidebar. Raw counters, not ratios, so titles can
/// show true numbers (see D010). `vram = None` means "no usable NVIDIA GPU"
/// and the VRAM/GPU widgets are hidden (D009).
#[derive(Debug, Clone)]
pub struct SysStats {
    pub ram_used: u64,  // bytes
    pub ram_total: u64, // bytes
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

/// Owns the sysinfo/NVML handles and refreshes a `SysStats` in place.
/// Sampled inline from the TUI loop every 500 ms (D010) — no thread, no locks.
pub struct Sampler {
    sys: System,
    nvml: Option<Nvml>,
}

impl Sampler {
    pub fn new() -> Self {
        let sys = System::new();
        let nvml = Nvml::init().ok();
        Self { sys, nvml }
    }

    pub fn sample(&mut self, stats: &mut SysStats) {
        self.sys.refresh_memory();
        stats.ram_used = self.sys.used_memory();
        stats.ram_total = self.sys.total_memory();

        let Some(nvml) = self.nvml.as_ref() else {
            stats.vram = None;
            stats.gpu_history.clear();
            return;
        };
        let sample = nvml.device_by_index(0).and_then(|dev| {
            let mem = dev.memory_info()?;
            let util = dev.utilization_rates()?;
            Ok((mem.used, mem.total, util.gpu))
        });
        match sample {
            Ok((used, total, util_pct)) => {
                stats.vram = Some((used, total));
                stats.gpu_history.push(util_pct as u64);
                if stats.gpu_history.len() > 60 {
                    stats.gpu_history.remove(0);
                }
            }
            Err(_) => {
                stats.vram = None;
                stats.gpu_history.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_never_crashes_and_hides_gpu_without_nvidia() {
        let mut sampler = Sampler::new();
        let mut stats = SysStats::new();
        sampler.sample(&mut stats);
        // RAM should be real on any machine with a working OS.
        assert!(stats.ram_total > 0);
        assert!(stats.ram_used <= stats.ram_total);
        if sampler.nvml.is_none() {
            assert!(stats.vram.is_none());
            assert!(stats.gpu_history.is_empty());
        }
    }
}
