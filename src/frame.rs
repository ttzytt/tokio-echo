use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::common::BoxError;
pub struct Frame {
    pub id: u32, 
    pub payload : Vec<u8>
}

pub async fn read_frame<R>(reader: &mut R) -> Result<Frame, BoxError>
where R: AsyncReadExt + Unpin{
    let mut header = [0u8; 8]; 
    reader.read_exact(&mut header).await?; 
    let id = u32::from_be_bytes(header[..4].try_into()?);
    let len = u32::from_be_bytes(header[4..].try_into()?) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    Ok(Frame{id, payload})
}

pub async fn write_frame<W>(writer: &mut W, frame: &Frame) -> Result<(), BoxError>
where W: AsyncWriteExt + Unpin {
    let mut buf = Vec::with_capacity(8 + frame.payload.len());
    buf.extend_from_slice(&frame.id.to_be_bytes()); 
    buf.extend_from_slice(&(frame.payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&frame.payload);
    writer.write_all(&buf).await?;
    Ok(())
}