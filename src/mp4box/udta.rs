use std::io::{Read, Seek};

use serde::Serialize;

use crate::mp4box::meta::MetaBox;
use crate::mp4box::{
    box_start, skip_box, skip_bytes_to, BoxHeader, BoxType, Error, Mp4Box, ReadBox, Result,
    HEADER_SIZE,
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UdtaBox {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<MetaBox>,
}

impl UdtaBox {
    pub fn get_type() -> BoxType {
        BoxType::UdtaBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE;
        if let Some(meta) = &self.meta {
            size += meta.box_size();
        }
        size
    }
}

impl Mp4Box for UdtaBox {
    fn box_type(&self) -> BoxType {
        Self::get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).expect("Failed to convert to JSON"))
    }

    fn summary(&self) -> Result<String> {
        Ok(String::new())
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for UdtaBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let mut meta = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        // QuickTime files may end the udta box with a 32-bit zero terminator,
        // which is too short to be a box header.
        while current + HEADER_SIZE <= end {
            // Get box header.
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "udta box contains a box with a larger size than it",
                ));
            }
            if s < HEADER_SIZE {
                break;
            }

            match name {
                BoxType::MetaBox => {
                    meta = Some(MetaBox::read_box(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    skip_box(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        skip_bytes_to(reader, start + size)?;

        Ok(Self { meta })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(udta: &[u8]) -> Result<UdtaBox> {
        let mut reader = Cursor::new(udta);
        let header = BoxHeader::read(&mut reader)?;
        UdtaBox::read_box(&mut reader, header.size)
    }

    #[test]
    fn quicktime_zero_terminator() {
        let udta = parse(&[0, 0, 0, 12, b'u', b'd', b't', b'a', 0, 0, 0, 0]).expect("parse failed");
        assert!(udta.meta.is_none());
    }

    #[test]
    fn quicktime_boxes_then_zero_terminator() {
        let mut bytes = vec![0, 0, 0, 42, b'u', b'd', b't', b'a'];
        bytes.extend_from_slice(&[0, 0, 0, 12, b'W', b'L', b'O', b'C', 0, 0x41, 0x01, 0x6b]);
        bytes.extend_from_slice(&[0, 0, 0, 9, b'S', b'e', b'l', b'O', 0]);
        bytes.extend_from_slice(&[0, 0, 0, 9, b'A', b'l', b'l', b'F', 0]);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        let udta = parse(&bytes).expect("parse failed");
        assert!(udta.meta.is_none());
    }
}
