//! Deepgram live streaming speech-to-text over WebSocket.
//!
//! Docs: https://developers.deepgram.com/docs/streaming
//! Audio is sent as raw little-endian linear16 binary frames; results arrive as
//! JSON text frames. `{"type":"CloseStream"}` asks the server to flush and finish.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

use crate::error::{OtoError, OtoResult};

use super::deepgram::DeepgramClient;
use super::stream::{join_finals, to_linear16, SttStream};
use super::traits::TranscriptionContext;

const MAX_KEYTERMS: usize = 100;
/// Silence Deepgram waits for before finalizing an utterance, in milliseconds.
const ENDPOINTING_MS: u32 = 300;

/// Shared between the socket task and the handle the pipeline holds.
#[derive(Default)]
struct StreamShared {
    /// Finalized fragments, in order.
    finals: Vec<String>,
    /// Current unfinalized tail.
    interim: String,
    /// Server reported end of utterance.
    speech_final: bool,
    /// Transport or protocol failure. Presence means "fall back to batch".
    error: Option<String>,
}

impl StreamShared {
    fn best_text(&self) -> String {
        let mut text = join_finals(&self.finals);
        let interim = self.interim.trim();
        if !interim.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(interim);
        }
        text
    }
}

pub struct DeepgramStream {
    /// Dropping this closes the socket task's audio channel, which is the
    /// signal to send CloseStream and drain the remaining results.
    audio_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    shared: Arc<Mutex<StreamShared>>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
    last_emitted: String,
}

/// Build the `wss://` results URL from a Deepgram REST base URL.
fn stream_url(
    base_url: &str,
    model: &str,
    ctx: &TranscriptionContext,
    sample_rate: u32,
) -> OtoResult<String> {
    let root = base_url.trim().trim_end_matches('/');
    let root = if root.is_empty() {
        "https://api.deepgram.com"
    } else {
        root
    };
    // Accept either the API root or a path already ending in /v1.
    let endpoint = if root.ends_with("/v1") {
        format!("{root}/listen")
    } else {
        format!("{root}/v1/listen")
    };

    let mut url = reqwest::Url::parse(&endpoint)
        .map_err(|e| OtoError::Message(format!("invalid Deepgram base URL: {e}")))?;
    let scheme = match url.scheme() {
        "http" | "ws" => "ws",
        _ => "wss",
    };
    url.set_scheme(scheme)
        .map_err(|_| OtoError::Message("could not switch Deepgram URL to WebSocket".into()))?;

    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("model", model);
        pairs.append_pair("smart_format", "true");
        pairs.append_pair("interim_results", "true");
        pairs.append_pair("encoding", "linear16");
        pairs.append_pair("sample_rate", &sample_rate.to_string());
        pairs.append_pair("channels", "1");
        pairs.append_pair("endpointing", &ENDPOINTING_MS.to_string());
        if let Some(lang) = ctx.language.as_deref().filter(|l| !l.trim().is_empty()) {
            pairs.append_pair("language", lang);
        } else {
            // Nova-3 code-switches across languages under `multi`, which is the
            // streaming equivalent of the batch path's detect_language.
            pairs.append_pair("language", "multi");
        }
        for term in ctx.keyterms.iter().take(MAX_KEYTERMS) {
            let trimmed = term.trim();
            if !trimmed.is_empty() {
                pairs.append_pair("keyterm", trimmed);
            }
        }
    }
    Ok(url.to_string())
}

/// Fold one server frame into the shared state. Returns true if the server
/// signalled that it is done sending results.
fn apply_message(shared: &Mutex<StreamShared>, payload: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return false;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("Results") => {}
        // Metadata is the last frame after CloseStream.
        Some("Metadata") => return true,
        Some("Error") => {
            let detail = value
                .get("description")
                .and_then(|d| d.as_str())
                .or_else(|| value.get("message").and_then(|m| m.as_str()))
                .unwrap_or("unknown streaming error")
                .to_string();
            if let Ok(mut state) = shared.lock() {
                state.error.get_or_insert(detail);
            }
            return true;
        }
        _ => return false,
    }

    let transcript = value
        .get("channel")
        .and_then(|c| c.get("alternatives"))
        .and_then(|a| a.as_array())
        .and_then(|alts| alts.first())
        .and_then(|alt| alt.get("transcript"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let is_final = value.get("is_final").and_then(|f| f.as_bool()).unwrap_or(false);
    let speech_final = value
        .get("speech_final")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);

    if let Ok(mut state) = shared.lock() {
        if is_final {
            if !transcript.is_empty() {
                state.finals.push(transcript);
            }
            // A finalized fragment supersedes whatever interim text preceded it.
            state.interim.clear();
        } else {
            state.interim = transcript;
        }
        if speech_final {
            state.speech_final = true;
        }
    }
    false
}

impl DeepgramStream {
    /// Open a live session. Errors here mean the caller should use batch upload.
    pub async fn connect(
        client: &DeepgramClient,
        ctx: &TranscriptionContext,
        sample_rate: u32,
    ) -> OtoResult<Self> {
        let url = stream_url(&client.base_url, &client.model, ctx, sample_rate)?;
        let mut request = url
            .into_client_request()
            .map_err(|e| OtoError::Message(format!("invalid Deepgram stream request: {e}")))?;
        let auth = HeaderValue::from_str(&format!("Token {}", client.api_key))
            .map_err(|_| OtoError::Message("Deepgram API key is not a valid header".into()))?;
        request.headers_mut().insert("Authorization", auth);

        let (socket, _response) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| OtoError::Message(format!("Deepgram stream connect failed: {e}")))?;

        let (mut sink, mut source) = socket.split();
        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let shared = Arc::new(Mutex::new(StreamShared::default()));

        let task_shared = Arc::clone(&shared);
        let task = tauri::async_runtime::spawn(async move {
            let mut sending = true;
            loop {
                tokio::select! {
                    chunk = audio_rx.recv(), if sending => match chunk {
                        Some(bytes) => {
                            if let Err(error) = sink.send(Message::Binary(bytes.into())).await {
                                if let Ok(mut state) = task_shared.lock() {
                                    state.error.get_or_insert(format!("audio send failed: {error}"));
                                }
                                break;
                            }
                        }
                        None => {
                            // Handle dropped: ask the server to flush, then keep
                            // reading until it acknowledges with Metadata.
                            sending = false;
                            let close = Message::Text(r#"{"type":"CloseStream"}"#.into());
                            if sink.send(close).await.is_err() {
                                break;
                            }
                        }
                    },
                    frame = source.next() => match frame {
                        Some(Ok(Message::Text(text))) => {
                            if apply_message(&task_shared, text.as_str()) {
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Ok(_)) => {}
                        Some(Err(error)) => {
                            if let Ok(mut state) = task_shared.lock() {
                                state.error.get_or_insert(format!("stream error: {error}"));
                            }
                            break;
                        }
                    },
                }
            }
            let _ = sink.close().await;
        });

        Ok(Self {
            audio_tx: Some(audio_tx),
            shared,
            task: Some(task),
            last_emitted: String::new(),
        })
    }
}

#[async_trait]
impl SttStream for DeepgramStream {
    async fn feed(&mut self, samples: &[i16]) -> OtoResult<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let Some(tx) = self.audio_tx.as_ref() else {
            return Ok(());
        };
        // A closed channel means the socket task exited; `failed()` reports why.
        let _ = tx.send(to_linear16(samples));
        Ok(())
    }

    fn take_partial(&mut self) -> Option<String> {
        let text = self.shared.lock().ok()?.best_text();
        if text.is_empty() || text == self.last_emitted {
            return None;
        }
        self.last_emitted = text.clone();
        Some(text)
    }

    fn endpointed(&self) -> bool {
        self.shared
            .lock()
            .map(|state| state.speech_final)
            .unwrap_or(false)
    }

    fn failed(&self) -> bool {
        self.shared
            .lock()
            .map(|state| state.error.is_some())
            .unwrap_or(true)
    }

    async fn finish(mut self: Box<Self>) -> OtoResult<String> {
        // Closing the channel is what triggers CloseStream in the socket task.
        self.audio_tx = None;
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        let state = self
            .shared
            .lock()
            .map_err(|_| OtoError::Message("streaming state poisoned".into()))?;
        if let Some(error) = state.error.as_ref() {
            return Err(OtoError::Message(error.clone()));
        }
        Ok(state.best_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> Mutex<StreamShared> {
        Mutex::new(StreamShared::default())
    }

    #[test]
    fn stream_url_is_wss_with_streaming_params() {
        let ctx = TranscriptionContext::default();
        let url = stream_url("https://api.deepgram.com", "nova-3", &ctx, 48_000).unwrap();
        assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
        assert!(url.contains("encoding=linear16"));
        assert!(url.contains("sample_rate=48000"));
        assert!(url.contains("channels=1"));
        assert!(url.contains("interim_results=true"));
        assert!(url.contains("endpointing=300"));
        // No explicit language means multilingual code-switching, not forced English.
        assert!(url.contains("language=multi"));
    }

    #[test]
    fn stream_url_respects_explicit_language_and_v1_suffix() {
        let ctx = TranscriptionContext {
            language: Some("de".into()),
            ..Default::default()
        };
        let url = stream_url("https://api.deepgram.com/v1", "nova-3", &ctx, 16_000).unwrap();
        assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
        assert!(!url.contains("/v1/v1/"));
        assert!(url.contains("language=de"));
        assert!(!url.contains("language=multi"));
    }

    #[test]
    fn localhost_http_base_downgrades_to_ws() {
        let ctx = TranscriptionContext::default();
        let url = stream_url("http://localhost:8080", "nova-3", &ctx, 16_000).unwrap();
        assert!(url.starts_with("ws://localhost:8080/v1/listen?"));
    }

    #[test]
    fn keyterms_are_forwarded() {
        let ctx = TranscriptionContext {
            keyterms: vec!["Kubernetes".into(), "Oto".into()],
            ..Default::default()
        };
        let url = stream_url("https://api.deepgram.com", "nova-3", &ctx, 48_000).unwrap();
        assert!(url.contains("keyterm=Kubernetes"));
        assert!(url.contains("keyterm=Oto"));
    }

    #[test]
    fn interim_results_are_replaced_and_finals_accumulate() {
        let state = shared();
        apply_message(
            &state,
            r#"{"type":"Results","is_final":false,"channel":{"alternatives":[{"transcript":"hello wor"}]}}"#,
        );
        assert_eq!(state.lock().unwrap().best_text(), "hello wor");

        // A later interim replaces the earlier one rather than appending.
        apply_message(
            &state,
            r#"{"type":"Results","is_final":false,"channel":{"alternatives":[{"transcript":"hello world"}]}}"#,
        );
        assert_eq!(state.lock().unwrap().best_text(), "hello world");

        apply_message(
            &state,
            r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":"Hello world."}]}}"#,
        );
        assert_eq!(state.lock().unwrap().best_text(), "Hello world.");

        apply_message(
            &state,
            r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":"How are you?"}]}}"#,
        );
        assert_eq!(
            state.lock().unwrap().best_text(),
            "Hello world. How are you?"
        );
    }

    #[test]
    fn speech_final_marks_the_endpoint() {
        let state = shared();
        assert!(!state.lock().unwrap().speech_final);
        apply_message(
            &state,
            r#"{"type":"Results","is_final":true,"speech_final":true,"channel":{"alternatives":[{"transcript":"Done."}]}}"#,
        );
        assert!(state.lock().unwrap().speech_final);
    }

    #[test]
    fn metadata_ends_the_read_loop() {
        let state = shared();
        assert!(apply_message(&state, r#"{"type":"Metadata","duration":1.5}"#));
    }

    #[test]
    fn server_error_is_recorded_and_stops_the_loop() {
        let state = shared();
        assert!(apply_message(
            &state,
            r#"{"type":"Error","description":"invalid model"}"#
        ));
        assert_eq!(
            state.lock().unwrap().error.as_deref(),
            Some("invalid model")
        );
    }

    #[test]
    fn empty_final_does_not_add_a_blank_fragment() {
        let state = shared();
        apply_message(
            &state,
            r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":""}]}}"#,
        );
        assert_eq!(state.lock().unwrap().best_text(), "");
        assert!(state.lock().unwrap().finals.is_empty());
    }

    #[test]
    fn unparseable_frames_are_ignored() {
        let state = shared();
        assert!(!apply_message(&state, "not json"));
        assert!(!apply_message(&state, r#"{"type":"SpeechStarted"}"#));
        assert_eq!(state.lock().unwrap().best_text(), "");
    }
}
