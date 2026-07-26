//! Input-device enumeration and selection.

use cpal::traits::HostTrait;
use serde::{Deserialize, Serialize};

use crate::error::{OtoError, OtoResult};

/// cpal 0.18 dropped `Device::name()` in favour of `Display`.
fn device_name(device: &cpal::Device) -> String {
    device.to_string()
}

/// One selectable microphone, as shown in settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputDevice {
    pub name: String,
    /// True for the device the system would pick on its own.
    pub is_default: bool,
}

/// Every input device the default host exposes, system default first.
pub fn list_input_devices() -> OtoResult<Vec<InputDevice>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .map(|d| device_name(&d))
        .unwrap_or_default();

    let devices = host
        .input_devices()
        .map_err(|e| OtoError::Message(format!("could not enumerate input devices: {e}")))?;

    let mut out: Vec<InputDevice> = Vec::new();
    for device in devices {
        let name = device_name(&device);
        if name.trim().is_empty() || out.iter().any(|d| d.name == name) {
            continue;
        }
        let is_default = !default_name.is_empty() && name == default_name;
        out.push(InputDevice { name, is_default });
    }
    out.sort_by(|a, b| b.is_default.cmp(&a.is_default).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Open `preferred` by name, falling back to the system default.
///
/// A device named in config can disappear between sessions — a USB headset gets
/// unplugged, a Bluetooth mic drops. Failing the dictation over that would be
/// worse than quietly recording from whatever is actually there, so the fallback
/// is reported through the returned flag rather than an error.
pub fn open_input_device(preferred: Option<&str>) -> OtoResult<(cpal::Device, bool)> {
    let host = cpal::default_host();
    let wanted = preferred.map(str::trim).filter(|n| !n.is_empty());

    if let Some(wanted) = wanted {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if device_name(&device) == wanted {
                    return Ok((device, false));
                }
            }
        }
        eprintln!("oto audio: input device {wanted:?} not found — using system default");
    }

    let device = host
        .default_input_device()
        .ok_or_else(|| OtoError::Message("no default input device".into()))?;
    Ok((device, wanted.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_serialize_for_the_settings_dropdown() {
        let device = InputDevice {
            name: "Blue Yeti".into(),
            is_default: true,
        };
        let json = serde_json::to_string(&device).unwrap();
        assert!(json.contains("\"name\":\"Blue Yeti\""));
        assert!(json.contains("\"is_default\":true"));
    }
}
