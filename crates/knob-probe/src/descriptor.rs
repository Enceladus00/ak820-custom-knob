//! Minimal HID report descriptor parser. It only works out which reports a
//! collection declares and how long they are, which is what we need to know
//! before asking a vendor interface for a report. Usages and value ranges
//! are ignored.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Input,
    Output,
    Feature,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Input => "input",
            Kind::Output => "output",
            Kind::Feature => "feature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub kind: Kind,
    /// 0 when the collection doesn't use report IDs.
    pub id: u8,
    pub bits: u32,
}

impl Report {
    /// Length on the wire as Windows sees it: the report ID byte (0 when
    /// unnumbered) followed by the data.
    pub fn wire_len(&self) -> usize {
        1 + self.bits.div_ceil(8) as usize
    }
}

/// Lists the reports declared in `desc`, in the order they first appear.
pub fn parse(desc: &[u8]) -> Result<Vec<Report>, String> {
    let mut reports: Vec<Report> = Vec::new();
    let (mut size, mut count, mut id) = (0u32, 0u32, 0u8);
    let mut stack = Vec::new();
    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            // Long item: [FE][data length][tag][data...]. Never used for reports.
            let len = *desc.get(i + 1).ok_or("truncated long item")? as usize;
            i += 3 + len;
            continue;
        }
        let n = match prefix & 0x03 {
            3 => 4,
            s => s as usize,
        };
        let data = desc
            .get(i + 1..i + 1 + n)
            .ok_or_else(|| format!("truncated item at offset {i}"))?;
        let value = data.iter().rev().fold(0u32, |acc, &b| acc << 8 | b as u32);
        // Tag and type, without the size bits.
        match prefix & 0xFC {
            0x74 => size = value,
            0x94 => count = value,
            0x84 => id = value as u8,
            0xA4 => stack.push((size, count, id)),
            0xB4 => (size, count, id) = stack.pop().ok_or("Pop without Push")?,
            tag @ (0x80 | 0x90 | 0xB0) => {
                let kind = match tag {
                    0x80 => Kind::Input,
                    0x90 => Kind::Output,
                    _ => Kind::Feature,
                };
                let bits = size * count;
                match reports.iter_mut().find(|r| r.kind == kind && r.id == id) {
                    Some(r) => r.bits += bits,
                    None => reports.push(Report { kind, id, bits }),
                }
            }
            _ => {}
        }
        i += 1 + n;
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_control_with_report_id() {
        // Usage Page (Consumer), Usage (Consumer Control), Collection (Application),
        // Report ID 3, Report Size 16, Report Count 1, Input, End Collection.
        let desc = [
            0x05, 0x0C, 0x09, 0x01, 0xA1, 0x01, 0x85, 0x03, 0x75, 0x10, 0x95, 0x01, 0x81, 0x00,
            0xC0,
        ];
        let reports = parse(&desc).unwrap();
        assert_eq!(
            reports,
            vec![Report {
                kind: Kind::Input,
                id: 3,
                bits: 16
            }]
        );
        assert_eq!(reports[0].wire_len(), 3);
    }

    #[test]
    fn vendor_page_unnumbered_input_output_feature() {
        // Usage Page (0xFF80) as a 2-byte item, 64-byte input, output and feature.
        let desc = [
            0x06, 0x80, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x75, 0x08, 0x96, 0x40, 0x00, 0x09, 0x01,
            0x81, 0x02, 0x09, 0x01, 0x91, 0x02, 0x09, 0x01, 0xB1, 0x02, 0xC0,
        ];
        let reports = parse(&desc).unwrap();
        assert_eq!(reports.len(), 3);
        for (r, kind) in reports
            .iter()
            .zip([Kind::Input, Kind::Output, Kind::Feature])
        {
            assert_eq!((r.kind, r.id, r.bits, r.wire_len()), (kind, 0, 512, 65));
        }
    }

    #[test]
    fn fields_add_up_and_push_pop_restore_state() {
        let desc = [
            0x85, 0x01, 0x75, 0x01, 0x95, 0x05, 0x81, 0x02, // 5 bits
            0x75, 0x03, 0x95, 0x01, 0x81, 0x01, // + 3 padding bits
            0xA4, 0x85, 0x02, 0x75, 0x08, 0x95, 0x07, 0xB1, 0x02, // feature id 2, 7 bytes
            0xB4, 0x81, 0x02, // back to id 1, size 3, count 1
        ];
        let reports = parse(&desc).unwrap();
        assert_eq!(
            reports,
            vec![
                Report {
                    kind: Kind::Input,
                    id: 1,
                    bits: 11
                },
                Report {
                    kind: Kind::Feature,
                    id: 2,
                    bits: 56
                },
            ]
        );
    }

    #[test]
    fn truncated_item_is_an_error() {
        assert!(parse(&[0x06, 0x80]).is_err());
    }
}
