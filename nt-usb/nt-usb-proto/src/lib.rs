use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};

use serialport::SerialPort;

pub enum ProxyPacket {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

impl ProxyPacket {
    fn id(&self) -> u8 {
        match self {
            ProxyPacket::Text(_) => 0,
            ProxyPacket::Binary(_) => 1,
            ProxyPacket::Close => 2,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut res = Vec::new();

        // Write the packet id
        res.write(&[self.id()])?;

        match self {
            ProxyPacket::Text(string) => {
                res.write_all(&string.as_bytes())?;
            }
            ProxyPacket::Binary(buf) => {
                res.write_all(&buf)?;
            }
            ProxyPacket::Close => {}
        };

        Ok(res)
    }

    pub fn decode(buf: Vec<u8>) -> Result<ProxyPacket> {
        let mut cursor = Cursor::new(buf);

        let mut id = [0u8; 1];
        cursor.read_exact(&mut id)?;
        let id = u8::from_le_bytes(id);

        let mut bytes = Vec::new();
        cursor.read_to_end(&mut bytes)?;

        match id {
            // Text Packet
            0 => {
                let string = String::from_utf8(bytes).map_err(|_| {
                    Error::new(ErrorKind::Other, "Could not create utf8 string from bytes")
                })?;

                Ok(ProxyPacket::Text(string))
            }
            // Binary Packet
            1 => Ok(ProxyPacket::Binary(bytes)),
            // Close Packet
            2 => Ok(ProxyPacket::Close),
            // Unknown packet ID
            _ => Err(Error::new(
                ErrorKind::Other,
                "Invalid packet ID found when decoding packet buffer",
            )),
        }
    }
}

/// Represents anything that can have USB packets written to it
///
/// For this application, it will be the USB serial connection on the master DS
pub trait ProtoWriteable: Write {
    fn write_packet(&mut self, packet: ProxyPacket) -> Result<()>;
}

impl ProtoWriteable for dyn SerialPort {
    fn write_packet(&mut self, packet: ProxyPacket) -> Result<()> {
        // Assert that u32 takes up 4 bytes (to make sure that encoding and decoding are consistent)
        assert_eq!(u32::BITS / 8, 4);

        // Encode the payload
        let payload = packet.encode()?;

        // Encode the payload length in LE
        let len = (payload.len() as u32).to_le_bytes();

        // Write the length first
        self.write_all(&len)?;

        // Write the full payload
        self.write_all(&payload)?;

        Ok(())
    }
}

/// Represents anything that can have USB packets read from it
///
/// For this application, it will be the USB serial connection on the slave rpi
pub trait ProtoReadable: Read {
    fn read_packet(&mut self) -> Result<ProxyPacket>;
}

impl ProtoReadable for dyn SerialPort {
    fn read_packet(&mut self) -> Result<ProxyPacket> {
        // Assert that u32 takes up 4 bytes (to make sure that encoding and decoding are consistent)
        assert_eq!(u32::BITS / 8, 4);

        // Read the first four bytes (the data length in LE)
        let mut len = [0u8; 4];
        self.read_exact(&mut len)?;
        let len = u32::from_le_bytes(len);

        // Read the rest of the packet (`len` bytes)
        let mut data = vec![0; len as usize];
        self.read_exact(&mut data)?;

        // Decode the packet buffer
        let packet = ProxyPacket::decode(data)?;

        Ok(packet)
    }
}
