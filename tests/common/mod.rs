#![allow(dead_code)]
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "bastionkv-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    pub fn db(&self) -> PathBuf {
        self.0.join("db")
    }
    pub fn wal(&self) -> PathBuf {
        std::fs::read_dir(self.db())
            .unwrap()
            .map(|x| x.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "bkw"))
            .unwrap()
    }
    pub fn segment(&self) -> PathBuf {
        std::fs::read_dir(self.db())
            .unwrap()
            .map(|x| x.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "bks"))
            .unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}
