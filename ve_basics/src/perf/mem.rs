//! The process's resident memory.

/// The process's resident memory in bytes: `VmRSS` of `/proc/self/status` on Linux, `None` elsewhere and on
/// the web. A read of procfs, a few microseconds: call it about once a second, not per frame.
pub fn rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        linux::rss_bytes()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::io::Read;

    /// `/proc/self/status` is about 1.5 KiB; more is not read.
    const STATUS_BYTES_MAX: u64 = 16 * 1024;

    #[expect(
        clippy::disallowed_methods,
        reason = "Linux only, never on the web; procfs answers from memory"
    )]
    pub(super) fn rss_bytes() -> Option<u64> {
        let file = std::fs::File::open("/proc/self/status").ok()?;
        let mut text = String::new();
        file.take(STATUS_BYTES_MAX).read_to_string(&mut text).ok()?;
        parse_vm_rss(&text)
    }

    pub(super) fn parse_vm_rss(status: &str) -> Option<u64> {
        let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
        let kib: u64 = line
            .trim_start_matches("VmRSS:")
            .trim()
            .trim_end_matches("kB")
            .trim()
            .parse()
            .ok()?;
        kib.checked_mul(1024)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn vm_rss_is_read_in_bytes() {
            let status = "Name:\tx\nVmPeak:\t  100 kB\nVmRSS:\t   2048 kB\nThreads:\t1\n";
            assert_eq!(super::parse_vm_rss(status), Some(2048 * 1024));
            assert_eq!(super::parse_vm_rss("Name:\tx\n"), None);
        }

        #[test]
        fn this_process_has_resident_memory() {
            assert!(super::rss_bytes().is_some_and(|b| b > 0));
        }
    }
}
