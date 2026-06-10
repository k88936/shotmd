use std::sync::{Arc, RwLock};
use egui::load::{BytesLoadResult, BytesLoader, BytesPoll, LoadError};

pub struct ShotmdBytesLoader {
    bytes: Arc<RwLock<Option<Vec<u8>>>>,
}

impl ShotmdBytesLoader {
    pub fn new(bytes: Arc<RwLock<Option<Vec<u8>>>>) -> Self {
        Self { bytes }
    }
}

impl BytesLoader for ShotmdBytesLoader {
    fn id(&self) -> &str {
        concat!(module_path!(), "::ShotmdBytesLoader")
    }

    fn load(&self, _ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if uri.starts_with("shotmd://capture") {
            if let Some(bytes) = self.bytes.read().unwrap().as_ref() {
                return Ok(BytesPoll::Ready {
                    size: None,
                    bytes: bytes.clone().into(),
                    mime: Some("image/png".to_string()),
                });
            }
        }
        Err(LoadError::NotSupported)
    }

    fn forget(&self, uri: &str) {
        let _ = uri;
    }

    fn forget_all(&self) {}

    fn byte_size(&self) -> usize {
        self.bytes
            .read()
            .unwrap()
            .as_ref()
            .map(|b| b.len())
            .unwrap_or(0)
    }
}