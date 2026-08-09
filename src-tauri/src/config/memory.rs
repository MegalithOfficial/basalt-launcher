use crate::error::{Error, Result};

use super::{Instance, LauncherSettings};

pub const DEFAULT_MIN_MEMORY_MB: u32 = 1024;
pub const DEFAULT_MAX_MEMORY_MB: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLimits {
    pub min_mb: u32,
    pub max_mb: u32,
}

impl MemoryLimits {
    pub fn new(min_mb: u32, max_mb: u32) -> Result<Self> {
        if min_mb == 0 {
            return Err(Error::other("Minimum memory must be greater than zero."));
        }
        if max_mb == 0 {
            return Err(Error::other("Maximum memory must be greater than zero."));
        }
        if min_mb > max_mb {
            return Err(Error::other(
                "Minimum memory cannot be greater than maximum memory.",
            ));
        }
        Ok(Self { min_mb, max_mb })
    }

    pub fn resolve(
        defaults: &LauncherSettings,
        min_override: Option<u32>,
        max_override: Option<u32>,
    ) -> Result<Self> {
        Self::new(
            min_override.unwrap_or(defaults.min_memory_mb),
            max_override.unwrap_or(defaults.max_memory_mb),
        )
    }

    pub fn suggested_max_after_oom(self, total_memory_mb: u64) -> Option<u32> {
        let installed = u32::try_from(total_memory_mb).unwrap_or(u32::MAX);
        let suggested = self.max_mb.saturating_mul(2).min(installed);
        (suggested > self.max_mb).then_some(suggested)
    }
}

impl LauncherSettings {
    pub fn memory_limits(&self) -> Result<MemoryLimits> {
        MemoryLimits::new(self.min_memory_mb, self.max_memory_mb)
    }
}

impl Instance {
    pub fn memory_limits(&self, defaults: &LauncherSettings) -> Result<MemoryLimits> {
        MemoryLimits::resolve(defaults, self.min_memory_mb, self.max_memory_mb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_overrides_resolve_independently() {
        let defaults = LauncherSettings::default();
        assert_eq!(
            MemoryLimits::resolve(&defaults, Some(1536), None).unwrap(),
            MemoryLimits {
                min_mb: 1536,
                max_mb: DEFAULT_MAX_MEMORY_MB,
            }
        );
        assert_eq!(
            MemoryLimits::resolve(&defaults, None, Some(6144)).unwrap(),
            MemoryLimits {
                min_mb: DEFAULT_MIN_MEMORY_MB,
                max_mb: 6144,
            }
        );
    }

    #[test]
    fn invalid_ranges_are_rejected() {
        assert!(MemoryLimits::new(0, 2048).is_err());
        assert!(MemoryLimits::new(512, 0).is_err());
        assert!(MemoryLimits::new(4096, 2048).is_err());
    }

    #[test]
    fn oom_suggestions_do_not_exceed_installed_memory() {
        let limits = MemoryLimits::new(512, 4096).unwrap();
        assert_eq!(limits.suggested_max_after_oom(16_384), Some(8192));
        assert_eq!(limits.suggested_max_after_oom(6144), Some(6144));
        assert_eq!(limits.suggested_max_after_oom(4096), None);
    }
}
