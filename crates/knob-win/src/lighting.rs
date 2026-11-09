//! Applies a lighting layout to the keyboard over its 2.4 GHz receiver.

use std::fmt;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use knob_core::LightingLayout;
use knob_rgb::{
    Command, FRAME_LEN, LED_LEN, LedState, PID_DONGLE, USAGE, USAGE_PAGE, VID, WIRE_LEN,
    parse_reply, read_request, to_wire,
};

const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);

/// Why a layout could not be applied. `Display` gives the codes the Settings
/// UI translates, or the detail of an unexpected failure.
#[derive(Debug, PartialEq, Eq)]
pub enum LightingError {
    /// The 2.4 GHz receiver isn't plugged in.
    NoReceiver,
    /// The receiver is there but the keyboard is asleep or off.
    NoReply,
    Failed(String),
}

impl fmt::Display for LightingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LightingError::NoReceiver => f.write_str("no_receiver"),
            LightingError::NoReply => f.write_str("no_reply"),
            LightingError::Failed(detail) => f.write_str(detail),
        }
    }
}

fn failed(what: &str, detail: impl fmt::Display) -> LightingError {
    LightingError::Failed(format!("{what}: {detail}"))
}

/// Writes `layout`, keeping any setting it doesn't cover, and checks what the
/// keyboard reports back.
pub fn apply(layout: &LightingLayout) -> Result<(), LightingError> {
    let device = open()?;
    let current = read_led(&device)?;
    let wanted = merged(current, layout)?;
    exchange(&device, Command::WriteLed, &wanted.write_request())?;
    let check = read_led(&device)?;
    if check.same_settings(&wanted) {
        Ok(())
    } else {
        Err(LightingError::Failed(
            "the keyboard reported different settings".into(),
        ))
    }
}

fn merged(current: LedState, layout: &LightingLayout) -> Result<LedState, LightingError> {
    let rgb = layout
        .rgb()
        .ok_or_else(|| LightingError::Failed(format!("bad colour {}", layout.color)))?;
    let refused = |e| failed("layout refused", format!("{e:?}"));

    let mut led = current.normalized();
    led.set_effect(layout.effect).map_err(refused)?;
    led.set_color(rgb);
    led.set_colorful(layout.colorful);
    led.set_brightness(layout.brightness).map_err(refused)?;
    led.set_speed(layout.speed).map_err(refused)?;
    Ok(led)
}

fn open() -> Result<HidDevice, LightingError> {
    let api = HidApi::new().map_err(|e| failed("HID init", e))?;
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VID
                && d.product_id() == PID_DONGLE
                && d.usage_page() == USAGE_PAGE
                && d.usage() == USAGE
        })
        .ok_or(LightingError::NoReceiver)?;
    info.open_device(&api).map_err(|e| failed("open", e))
}

fn read_led(device: &HidDevice) -> Result<LedState, LightingError> {
    let frames = read_request(Command::ReadLed, LED_LEN);
    let block = exchange(device, Command::ReadLed, &frames)?;
    LedState::parse(&block).map_err(|e| failed("unexpected lighting data", format!("{e:?}")))
}

/// Sends each frame and waits for its acknowledgement, returning the payload
/// bytes of the replies.
fn exchange(
    device: &HidDevice,
    command: Command,
    frames: &[[u8; FRAME_LEN]],
) -> Result<Vec<u8>, LightingError> {
    let mut block = Vec::new();
    for frame in frames {
        device
            .write(&to_wire(frame))
            .map_err(|e| failed("write", e))?;
        block.extend_from_slice(&await_reply(device, command)?);
    }
    Ok(block)
}

fn await_reply(device: &HidDevice, command: Command) -> Result<Vec<u8>, LightingError> {
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut buf = [0u8; WIRE_LEN];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(LightingError::NoReply);
        }
        let n = device
            .read_timeout(&mut buf, left.as_millis() as i32)
            .map_err(|e| failed("read", e))?;
        if let Some(reply) = parse_reply(&buf[..n]).filter(|r| r.command == command.code()) {
            return Ok(reply.payload.to_vec());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_read() -> LedState {
        LedState::parse(&[1, 255, 255, 255, 0, 0, 0, 0, 1, 3, 3, 0, 0, 0, 0, 0]).unwrap()
    }

    fn layout() -> LightingLayout {
        LightingLayout {
            name: "Test".into(),
            effect: 11,
            color: "#102030".into(),
            colorful: false,
            brightness: 5,
            speed: 2,
        }
    }

    #[test]
    fn a_layout_replaces_only_its_own_settings() {
        let mut raw = *first_read().raw();
        raw[5] = 0x11; // secondary colour
        raw[11] = 1; // direction
        let current = LedState::parse(&raw).unwrap();

        let led = merged(current, &layout()).unwrap();
        assert_eq!(led.effect(), 11);
        assert_eq!(led.color(), [0x10, 0x20, 0x30]);
        assert!(!led.colorful());
        assert_eq!((led.brightness(), led.speed()), (5, 2));
        assert_eq!((led.raw()[5], led.direction()), (0x11, 1));
        assert_eq!(led.raw()[14..16], [0xAA, 0x55]);
    }

    #[test]
    fn colorful_layouts_turn_cycling_on() {
        let colorful = LightingLayout {
            colorful: true,
            ..layout()
        };
        assert!(merged(first_read(), &colorful).unwrap().colorful());
    }

    #[test]
    fn a_bad_layout_is_refused_before_anything_is_sent() {
        let bad = LightingLayout {
            brightness: 9,
            ..layout()
        };
        assert!(matches!(
            merged(first_read(), &bad),
            Err(LightingError::Failed(_))
        ));
    }

    #[test]
    fn errors_use_the_codes_the_ui_translates() {
        assert_eq!(LightingError::NoReceiver.to_string(), "no_receiver");
        assert_eq!(LightingError::NoReply.to_string(), "no_reply");
    }
}
