use std::io::{Read, Result, Write};

/// Represents anything that can have USB packets written to it
/// 
/// For this application, it will be the USB serial connection on the master DS
pub trait ProtoWriteable {
    fn write_packet(&mut self, data: Vec<u8>) -> Result<()>;
}

impl ProtoWriteable for dyn Write {
    fn write_packet(&mut self, data: Vec<u8>) -> Result<()> {
        // Assert that u32 takes up 4 bytes (to make sure that encoding and decoding are consistent)
        assert_eq!(u32::BITS / 8, 4);

        self.write_all(&(data.len() as u32).to_le_bytes())?;
        self.write_all(&data)?;

        Ok(())
    }
}

/// Represents anything that can have USB packets read from it
/// 
/// For this application, it will be the USB serial connection on the slave rpi
pub trait ProtoReadable {
    fn read_packet(&mut self) -> Result<Vec<u8>>;
}

impl ProtoReadable for dyn Read {
    fn read_packet(&mut self) -> Result<Vec<u8>> {
        // Assert that u32 takes up 4 bytes (to make sure that encoding and decoding are consistent)
        assert_eq!(u32::BITS / 8, 4);

        // Read the first four bytes (the data length)
        let mut len_buf = [0u8; 4];
        self.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf);

        // Read the rest of the packet (`len` bytes)
        let data = vec![0; len as usize];
        self.read_exact(&mut len_buf)?;

        Ok(data)
    }
}
