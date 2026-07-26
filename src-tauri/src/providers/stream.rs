//! Streaming speech-to-text.
//!
//! A streaming session consumes audio while the user is still speaking and
//! produces interim text, so the transcript is largely finished by the time the
//! hotkey is released. Providers that only accept a completed upload are not
//! modelled here — the orchestrator keeps its batch path for those, and falls
//! back to it whenever a stream cannot be established or dies mid-session.

use async_trait::async_trait;

use crate::error::OtoResult;

#[async_trait]
pub trait SttStream: Send {
    /// Feed newly captured mono samples.
    async fn feed(&mut self, samples: &[i16]) -> OtoResult<()>;

    /// Best transcript so far, returned only when it changed since the last call.
    fn take_partial(&mut self) -> Option<String>;

    /// True once the provider reported end-of-utterance.
    fn endpointed(&self) -> bool;

    /// True when the transport failed. The caller falls back to batch upload.
    fn failed(&self) -> bool;

    /// Close the stream and return the final transcript.
    async fn finish(self: Box<Self>) -> OtoResult<String>;
}

/// Encode mono samples as little-endian linear16, the wire format every
/// streaming provider here expects.
pub fn to_linear16(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Join finalized transcript fragments into one transcript.
pub fn join_finals(finals: &[String]) -> String {
    let mut out = String::new();
    for part in finals {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear16_is_little_endian_pairs() {
        assert_eq!(to_linear16(&[1, -1]), vec![0x01, 0x00, 0xFF, 0xFF]);
        assert!(to_linear16(&[]).is_empty());
    }

    #[test]
    fn finals_join_with_single_spaces_and_skip_blanks() {
        let parts = vec![
            "Hello there.".to_string(),
            "   ".to_string(),
            " General Kenobi.".to_string(),
        ];
        assert_eq!(join_finals(&parts), "Hello there. General Kenobi.");
        assert_eq!(join_finals(&[]), "");
    }
}
