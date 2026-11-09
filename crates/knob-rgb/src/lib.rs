//! Lighting protocol of the AK820 MAX 2.4 GHz receiver (`0C45:FEFC`), usage
//! page `0xFF80`. See `docs/rgb-research.md` for where it comes from.
//!
//! This crate only builds and parses bytes; it never touches a device.
//!
//! [`Command`] is deliberately closed. The same protocol has commands that
//! factory-reset the keyboard or write its flash, and the safest way never to
//! send them is to have no way of encoding them.

/// Vendor and product of the 2.4 GHz receiver.
pub const VID: u16 = 0x0C45;
pub const PID_DONGLE: u16 = 0xFEFC;
/// The HID collection that carries this protocol.
pub const USAGE_PAGE: u16 = 0xFF80;
pub const USAGE: u16 = 0x0001;

/// Bytes in a frame, not counting the HID report id.
pub const FRAME_LEN: usize = 32;
/// Bytes on the wire: report id 0 followed by a frame.
pub const WIRE_LEN: usize = FRAME_LEN + 1;

const HEADER_LEN: usize = 8;
const CHUNK_LEN: usize = FRAME_LEN - HEADER_LEN;
const REQUEST: u8 = 0xAA;
const REPLY: u8 = 0x55;

/// Size of the block returned by [`Command::ReadInfo`].
pub const INFO_LEN: usize = 32;
/// Size of the lighting block.
pub const LED_LEN: usize = 16;

/// Everything this program can ask the keyboard to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ReadInfo,
    ReadLed,
    WriteLed,
}

impl Command {
    pub const fn code(self) -> u8 {
        match self {
            Command::ReadInfo => 0x10,
            Command::ReadLed => 0x13,
            Command::WriteLed => 0x23,
        }
    }
}

/// Splits `payload` into frames the way the vendor driver does.
///
/// Every frame but a multi-chunk transfer's short tail claims a full chunk,
/// even when it carries less. The last frame has byte 6 set.
pub fn frames(command: Command, payload: &[u8]) -> Vec<[u8; FRAME_LEN]> {
    let tail = payload.len() % CHUNK_LEN;
    let count = payload.len().div_ceil(CHUNK_LEN).max(1);
    (0..count)
        .map(|i| {
            let last = i == count - 1;
            let start = (i * CHUNK_LEN).min(payload.len());
            let end = (start + CHUNK_LEN).min(payload.len());
            let offset = (i * CHUNK_LEN) as u16;
            let len = if last && i != 0 && tail != 0 {
                tail
            } else {
                CHUNK_LEN
            };

            let mut frame = [0u8; FRAME_LEN];
            frame[0] = REQUEST;
            frame[1] = command.code();
            frame[2] = len as u8;
            frame[3..5].copy_from_slice(&offset.to_le_bytes());
            frame[6] = u8::from(last);
            frame[HEADER_LEN..HEADER_LEN + (end - start)].copy_from_slice(&payload[start..end]);
            frame
        })
        .collect()
}

/// Prepends the HID report id.
pub fn to_wire(frame: &[u8; FRAME_LEN]) -> [u8; WIRE_LEN] {
    let mut wire = [0u8; WIRE_LEN];
    wire[1..].copy_from_slice(frame);
    wire
}

/// The frames that read a block of `len` bytes.
pub fn read_request(command: Command, len: usize) -> Vec<[u8; FRAME_LEN]> {
    frames(command, &vec![0u8; len])
}

/// A frame received from the keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply<'a> {
    pub command: u8,
    pub payload: &'a [u8],
}

/// Parses a report. Some HID backends strip report id 0 and some don't, so
/// both layouts are accepted.
pub fn parse_reply(report: &[u8]) -> Option<Reply<'_>> {
    let start = match report {
        [REPLY, ..] => 0,
        [0, REPLY, ..] => 1,
        _ => return None,
    };
    let body = &report[start..];
    if body.len() < HEADER_LEN {
        return None;
    }
    Some(Reply {
        command: body[1],
        payload: &body[HEADER_LEN..],
    })
}

/// Fields of the device information block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Firmware version times 100, for example 113 for 1.13.
    pub firmware: u16,
    /// Charge in percent, when the keyboard reported one.
    pub battery: Option<u8>,
}

impl DeviceInfo {
    pub fn parse(block: &[u8]) -> Option<Self> {
        if block.len() < 18 {
            return None;
        }
        let bcd = u16::from(block[8] & 0x0F) + 10 * u16::from(block[8] >> 4);
        let battery = block[17];
        Some(Self {
            firmware: bcd + 100 * u16::from(block[9]),
            battery: (1..=100).contains(&battery).then_some(battery),
        })
    }
}

/// Why a lighting change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedError {
    /// The block lacks the `AA 55` marker, so its layout can't be trusted.
    BadMarker,
    WrongLength,
    BrightnessOutOfRange,
    SpeedOutOfRange,
    UnknownEffect,
}

/// Lowest and highest brightness and speed the vendor driver offers.
pub const LEVEL_RANGE: std::ops::RangeInclusive<u8> = 1..=5;

/// Effect value of the per-key custom pattern.
pub const EFFECT_CUSTOM: u8 = 0x80;
/// Highest numbered built-in effect.
pub const EFFECT_MAX: u8 = 0x13;

/// The keyboard's lighting block. It keeps all 16 bytes so that fields this
/// program doesn't edit are written back exactly as they were read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedState {
    raw: [u8; LED_LEN],
}

impl LedState {
    /// Accepts a block carrying the marker the driver writes, or one with no
    /// marker at all whose brightness and speed are still valid. The second
    /// form is what a keyboard reported before any driver had written to it.
    pub fn parse(block: &[u8]) -> Result<Self, LedError> {
        let raw: [u8; LED_LEN] = block
            .get(..LED_LEN)
            .and_then(|b| b.try_into().ok())
            .ok_or(LedError::WrongLength)?;
        let marked = raw[14..16] == [0xAA, 0x55];
        let unmarked = raw[14..16] == [0, 0]
            && LEVEL_RANGE.contains(&raw[9])
            && LEVEL_RANGE.contains(&raw[10]);
        if !marked && !unmarked {
            return Err(LedError::BadMarker);
        }
        Ok(Self { raw })
    }

    /// Compares the fields the keyboard reports back. It applies a colour
    /// and the fixed fields on a write, but its read-back never shows them.
    pub fn same_settings(&self, other: &Self) -> bool {
        let fields = |led: &Self| {
            let mut raw = led.raw;
            for i in [1, 2, 3, 4, 8, 14, 15] {
                raw[i] = 0;
            }
            raw
        };
        fields(self) == fields(other)
    }

    /// This state with the two fixed fields the driver always writes.
    pub fn normalized(mut self) -> Self {
        self.raw[4] = 0xFF;
        self.raw[14] = 0xAA;
        self.raw[15] = 0x55;
        self
    }

    pub fn raw(&self) -> &[u8; LED_LEN] {
        &self.raw
    }

    /// 0 means the lighting is off.
    pub fn effect(&self) -> u8 {
        self.raw[0]
    }

    pub fn color(&self) -> [u8; 3] {
        [self.raw[1], self.raw[2], self.raw[3]]
    }

    /// True when the effect cycles colours instead of using [`Self::color`].
    pub fn colorful(&self) -> bool {
        self.raw[8] != 0
    }

    pub fn brightness(&self) -> u8 {
        self.raw[9]
    }

    pub fn speed(&self) -> u8 {
        self.raw[10]
    }

    pub fn direction(&self) -> u8 {
        self.raw[11]
    }

    pub fn set_effect(&mut self, effect: u8) -> Result<(), LedError> {
        if effect > EFFECT_MAX && effect != EFFECT_CUSTOM {
            return Err(LedError::UnknownEffect);
        }
        self.raw[0] = effect;
        Ok(())
    }

    /// Sets a single colour and turns off colour cycling.
    pub fn set_color(&mut self, [r, g, b]: [u8; 3]) {
        self.raw[1] = r;
        self.raw[2] = g;
        self.raw[3] = b;
        self.raw[8] = 0;
    }

    /// Turns colour cycling on or off, keeping the stored colour.
    pub fn set_colorful(&mut self, on: bool) {
        self.raw[8] = u8::from(on);
    }

    pub fn set_brightness(&mut self, level: u8) -> Result<(), LedError> {
        if !LEVEL_RANGE.contains(&level) {
            return Err(LedError::BrightnessOutOfRange);
        }
        self.raw[9] = level;
        Ok(())
    }

    pub fn set_speed(&mut self, level: u8) -> Result<(), LedError> {
        if !LEVEL_RANGE.contains(&level) {
            return Err(LedError::SpeedOutOfRange);
        }
        self.raw[10] = level;
        Ok(())
    }

    /// The frames that store this state on the keyboard.
    pub fn write_request(&self) -> Vec<[u8; FRAME_LEN]> {
        frames(Command::WriteLed, &self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LedState {
        LedState::parse(&[1, 255, 0, 10, 255, 0, 0, 0, 0, 5, 3, 0, 0, 0, 0xAA, 0x55]).unwrap()
    }

    #[test]
    fn a_short_write_is_one_frame_claiming_a_full_chunk() {
        let frames = sample().write_request();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0][..8], [0xAA, 0x23, 0x18, 0, 0, 0, 1, 0]);
        assert_eq!(frames[0][8..24], *sample().raw());
        assert!(frames[0][24..].iter().all(|&b| b == 0));
    }

    #[test]
    fn the_info_read_splits_into_two_chunks() {
        let frames = read_request(Command::ReadInfo, INFO_LEN);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0][..8], [0xAA, 0x10, 24, 0, 0, 0, 0, 0]);
        assert_eq!(frames[1][..8], [0xAA, 0x10, 8, 24, 0, 0, 1, 0]);
    }

    #[test]
    fn an_exact_multiple_keeps_a_full_last_chunk() {
        let frames = frames(Command::ReadLed, &[0; 48]);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1][..8], [0xAA, 0x13, 24, 24, 0, 0, 1, 0]);
    }

    #[test]
    fn the_report_id_is_prepended() {
        let frame = [7u8; FRAME_LEN];
        let wire = to_wire(&frame);
        assert_eq!(wire[0], 0);
        assert_eq!(wire[1..], frame);
    }

    #[test]
    fn only_the_three_safe_commands_exist() {
        let codes = [Command::ReadInfo, Command::ReadLed, Command::WriteLed].map(Command::code);
        assert_eq!(codes, [0x10, 0x13, 0x23]);
    }

    #[test]
    fn replies_parse_with_and_without_the_report_id() {
        let mut frame = [0u8; 32];
        frame[0] = 0x55;
        frame[1] = 0x13;
        frame[8] = 9;
        let bare = parse_reply(&frame).unwrap();
        assert_eq!((bare.command, bare.payload[0]), (0x13, 9));

        let mut prefixed = vec![0u8];
        prefixed.extend_from_slice(&frame);
        let with_id = parse_reply(&prefixed).unwrap();
        assert_eq!((with_id.command, with_id.payload[0]), (0x13, 9));
    }

    #[test]
    fn unrelated_reports_are_not_replies() {
        assert_eq!(parse_reply(&[0xAA, 0x10, 0, 0, 0, 0, 0, 0, 0]), None);
        assert_eq!(parse_reply(&[]), None);
        assert_eq!(parse_reply(&[0x55, 0x10]), None);
    }

    #[test]
    fn device_info_decodes_firmware_and_battery() {
        let mut block = [0u8; INFO_LEN];
        block[8] = 0x13;
        block[9] = 1;
        block[17] = 76;
        let info = DeviceInfo::parse(&block).unwrap();
        assert_eq!(info.firmware, 113);
        assert_eq!(info.battery, Some(76));
    }

    #[test]
    fn a_zero_battery_byte_means_no_reading() {
        let info = DeviceInfo::parse(&[0u8; INFO_LEN]).unwrap();
        assert_eq!(info.battery, None);
    }

    #[test]
    fn a_block_that_looks_wrong_is_refused() {
        assert_eq!(LedState::parse(&[0u8; LED_LEN]), Err(LedError::BadMarker));
        assert_eq!(LedState::parse(&[0u8; 4]), Err(LedError::WrongLength));

        let mut half_marker = *sample().raw();
        half_marker[15] = 0;
        assert_eq!(LedState::parse(&half_marker), Err(LedError::BadMarker));
    }

    const FIRST_READ: [u8; LED_LEN] = [1, 255, 255, 255, 0, 0, 0, 0, 1, 3, 3, 0, 0, 0, 0, 0];

    #[test]
    fn the_block_read_from_a_real_keyboard_is_accepted() {
        let led = LedState::parse(&FIRST_READ).unwrap();
        assert_eq!((led.effect(), led.brightness(), led.speed()), (1, 3, 3));
        assert_eq!(led.raw(), &FIRST_READ);
    }

    #[test]
    fn read_back_ignores_only_the_fixed_fields() {
        let plain = LedState::parse(&FIRST_READ).unwrap();
        assert!(plain.same_settings(&plain.normalized()));

        let mut blue = plain;
        blue.set_color([0x66, 0xCC, 0xFF]);
        assert!(plain.same_settings(&blue));

        let mut dimmer = plain;
        dimmer.set_brightness(1).unwrap();
        assert!(!plain.same_settings(&dimmer));
    }

    #[test]
    fn normalizing_only_sets_the_fixed_fields() {
        let led = LedState::parse(&FIRST_READ).unwrap().normalized();
        let mut expected = FIRST_READ;
        expected[4] = 255;
        expected[14] = 0xAA;
        expected[15] = 0x55;
        assert_eq!(led.raw(), &expected);
    }

    #[test]
    fn edits_leave_unknown_bytes_alone() {
        let mut block = *sample().raw();
        block[4] = 0x7B;
        block[12] = 2;
        block[13] = 0x44;
        let mut led = LedState::parse(&block).unwrap();
        led.set_color([1, 2, 3]);
        led.set_brightness(2).unwrap();
        assert_eq!(
            (led.raw()[4], led.raw()[12], led.raw()[13]),
            (0x7B, 2, 0x44)
        );
    }

    #[test]
    fn setting_a_color_stops_color_cycling() {
        let mut block = *sample().raw();
        block[8] = 1;
        let mut led = LedState::parse(&block).unwrap();
        assert!(led.colorful());
        led.set_color([9, 8, 7]);
        assert!(!led.colorful());
        assert_eq!(led.color(), [9, 8, 7]);
    }

    #[test]
    fn out_of_range_values_are_refused() {
        let mut led = sample();
        assert_eq!(led.set_brightness(0), Err(LedError::BrightnessOutOfRange));
        assert_eq!(led.set_brightness(6), Err(LedError::BrightnessOutOfRange));
        assert_eq!(led.set_speed(9), Err(LedError::SpeedOutOfRange));
        assert_eq!(led.set_effect(0x14), Err(LedError::UnknownEffect));
        assert_eq!(led, sample());
    }

    #[test]
    fn off_and_custom_are_valid_effects() {
        let mut led = sample();
        assert_eq!(led.set_effect(0), Ok(()));
        assert_eq!(led.set_effect(EFFECT_CUSTOM), Ok(()));
        assert_eq!(led.effect(), EFFECT_CUSTOM);
    }
}
