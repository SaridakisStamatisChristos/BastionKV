#[cfg(feature = "fault-injection")]
use crate::Error;
use crate::Result;

/// Hooks only exist in the explicitly enabled testing build. No production environment backdoor.
pub(crate) fn point(name: &str) -> Result<()> {
    #[cfg(feature = "fault-injection")]
    {
        if std::env::var("BASTIONKV_CRASH").as_deref() == Ok(name) {
            std::process::abort();
        }
        if std::env::var("BASTIONKV_IO_FAIL").as_deref() == Ok(name) {
            return Err(Error::io(
                format!("injected I/O error at {name}"),
                std::io::Error::other("deterministic injected failure"),
            ));
        }
    }
    #[cfg(not(feature = "fault-injection"))]
    let _ = name;
    Ok(())
}
