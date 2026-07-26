pub mod capture;
pub mod cues;
pub mod devices;
pub mod vad;
pub mod wav;

// Re-exported for pipeline (Task 9+); may appear unused until wired.
#[allow(unused_imports)]
pub use capture::{AudioRecorder, CaptureTuning};
#[allow(unused_imports)]
pub use cues::Cue;
#[allow(unused_imports)]
pub use devices::{list_input_devices, InputDevice};
#[allow(unused_imports)]
pub use vad::{VadSnapshot, VadTracker};
