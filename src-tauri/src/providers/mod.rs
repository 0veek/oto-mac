pub mod deepgram;
pub mod deepgram_stream;
pub mod local_whisper;
pub mod openai_compat;
pub mod presets;
pub mod stream;
pub mod traits;

pub use deepgram::DeepgramClient;
pub use deepgram_stream::DeepgramStream;
pub use local_whisper::LocalWhisperClient;
pub use openai_compat::{client_from_config, OpenAiCompatClient};
pub use stream::SttStream;
pub use traits::{PolishContext, SpeechToText, TextPolisher, TranscriptionContext};
