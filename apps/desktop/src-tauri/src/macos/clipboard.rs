//! Owned buffers cross the Swift bridge; no AppKit object or borrowed data does.
use serde_json::{json, Value};

pub(super) fn snapshot_is_current(previous: Option<u64>, current: Option<u64>, incoming: u64) -> bool {
    previous == current && current != Some(incoming)
}

pub(super) fn history_text(text: &str) -> Option<Value> {
    let secret = text.trim().starts_with("gsk_") || text.trim().starts_with("sk-proj-");
    (!secret && !text.is_empty() && text.len() <= 262_144).then(|| {
        json!({"kind":"text","text":text,"isUrl":text.starts_with("https://") || text.starts_with("http://")})
    })
}

#[cfg(target_os = "macos")]
mod bridge {
    use std::ptr;

    const MAX_BYTES: usize = 160_000_000;
    extern "C" {
        fn whispera_clipboard_sequence() -> i64;
        fn whispera_clipboard_read(previous: i64, sequence: *mut i64, kind: *mut i32, len: *mut usize) -> *mut u8;
        fn whispera_clipboard_free(data: *mut u8);
        fn whispera_clipboard_write_files(files: *const u8, files_len: usize, expected: *const u8, expected_len: usize, sequence: *mut i64) -> i32;
    }

    pub(crate) enum Content {
        Empty,
        Files(Vec<String>),
        Image(Vec<u8>),
        Text(String),
    }
    pub(crate) struct Snapshot {
        pub sequence: u64,
        pub content: Content,
    }
    struct Buffer(*mut u8);
    impl Drop for Buffer {
        fn drop(&mut self) {
            if !self.0.is_null() { unsafe { whispera_clipboard_free(self.0); } }
        }
    }
    pub(crate) fn sequence() -> Option<u64> {
        u64::try_from(unsafe { whispera_clipboard_sequence() }).ok()
    }
    pub(crate) fn snapshot(previous: Option<u64>) -> Result<Snapshot, String> {
        let (mut sequence, mut kind, mut len) = (-1, -1, 0);
        let buffer = Buffer(unsafe {
            whispera_clipboard_read(previous.and_then(|n| i64::try_from(n).ok()).unwrap_or(-1), &mut sequence, &mut kind, &mut len)
        });
        if kind < 0 || len > MAX_BYTES || (buffer.0.is_null() && len != 0) {
            return Err("No se pudo leer una copia estable del portapapeles".into());
        }
        let bytes = if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(buffer.0, len) } };
        let content = match kind {
            0 => Content::Empty,
            1 => Content::Files(serde_json::from_slice(bytes).map_err(|e| e.to_string())?),
            2 => Content::Image(bytes.to_vec()),
            3 => Content::Text(String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())?),
            _ => return Err("Formato de portapapeles no reconocido".into()),
        };
        Ok(Snapshot { sequence: u64::try_from(sequence).map_err(|_| "Secuencia de portapapeles invalida")?, content })
    }
    // None means that the user changed the clipboard; it is not a successful write.
    pub(crate) fn write_files(files: &[String], expected: Option<&[Vec<String>]>) -> Result<Option<u64>, String> {
        let files = serde_json::to_vec(files).map_err(|e| e.to_string())?;
        let expected = expected.map(serde_json::to_vec).transpose().map_err(|e| e.to_string())?;
        let (expected_ptr, expected_len) = expected.as_ref().map(|v| (v.as_ptr(), v.len())).unwrap_or((ptr::null(), 0));
        let mut sequence = -1;
        let status = unsafe {
            whispera_clipboard_write_files(files.as_ptr(), files.len(), expected_ptr, expected_len, &mut sequence)
        };
        match status {
            0 if expected.is_some() => Ok(None),
            1 => Ok(Some(u64::try_from(sequence).map_err(|_| "Secuencia de portapapeles invalida")?)),
            _ => Err("No se pudieron copiar los archivos al portapapeles".into()),
        }
    }
}
#[cfg(target_os = "macos")]
pub(crate) use bridge::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_keeps_text_urls_and_the_existing_secret_filter() {
        assert_eq!(history_text("https://example.com/a%20b").unwrap()["isUrl"], true);
        assert_eq!(history_text("plain\ntext").unwrap()["text"], "plain\ntext");
        for text in ["", "  gsk_fixture", "\nsk-proj-fixture"] {
            assert!(history_text(text).is_none());
        }
        assert!(history_text(&"a".repeat(262_144)).is_some());
        assert!(history_text(&"a".repeat(262_145)).is_none());
    }
    #[test]
    fn stale_reads_cannot_undo_suppression_or_another_collection() {
        assert!(snapshot_is_current(None, None, 0));
        assert!(snapshot_is_current(Some(1), Some(1), 2));
        assert!(!snapshot_is_current(Some(1), Some(1), 1));
        assert!(!snapshot_is_current(Some(1), Some(2), 2));
        assert!(!snapshot_is_current(Some(1), Some(3), 2));
    }
}
