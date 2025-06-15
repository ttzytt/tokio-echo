use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::common::{BoxError, Id_t, ByteSeq};


#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Data = 0,
    Terminate = 1,
    AssignId = 2,
}

impl TryFrom<u8> for FrameKind {
    type Error = ();
    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            0 => Ok(FrameKind::Data),
            1 => Ok(FrameKind::Terminate),
            2 => Ok(FrameKind::AssignId),
            _ => Err(()),
        }
    }
}

impl TryInto<u8> for FrameKind {
    type Error = ();
    fn try_into(self) -> Result<u8, Self::Error> {
        match self {
            FrameKind::Data => Ok(0),
            FrameKind::Terminate => Ok(1),
            FrameKind::AssignId => Ok(2), // AssignId is not a valid frame kind for this conversion
            _ => Err(()),
        }
    }
}


pub struct Frame {
    pub kind: FrameKind,
    pub id: Id_t, 
    pub payload : ByteSeq,
    
}

impl Frame{
    pub const HEADER_BYTES: usize =  1 + 4 + 4; // kind(1) + id(4) + len(4)
    pub const TERMINATE_FRAME: Frame = Frame {
        kind: FrameKind::Terminate,
        id: 0,
        payload: Vec::new(),
    };
}

pub async fn read_frame<R>(reader: &mut R) -> Result<Frame, BoxError>
where R: AsyncReadExt + Unpin{
    // TODO: use some automatic serialization tools to make this clearer
    let mut header = [0u8; Frame::HEADER_BYTES]; 
    reader.read_exact(&mut header).await?;
    let kind = FrameKind::try_from(header[0]).unwrap();
    let id = Id_t::from_be_bytes(header[1..5].try_into()?) as Id_t;
    let len = Id_t::from_be_bytes(header[5..9].try_into()?) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    Ok(Frame{kind, id, payload})
}

pub async fn write_frame<W>(writer: &mut W, frame: &Frame) -> Result<(), BoxError>
where W: AsyncWriteExt + Unpin {
    let mut buf = Vec::with_capacity(Frame::HEADER_BYTES + frame.payload.len());
    buf.push(frame.kind as u8);
    buf.extend_from_slice(&frame.id.to_be_bytes()); 
    buf.extend_from_slice(&(frame.payload.len() as Id_t).to_be_bytes());
    buf.extend_from_slice(&frame.payload);
    writer.write_all(&buf).await?;
    Ok(())
}

pub async fn write_frames<W>(writer: &mut W, frames: &[Frame]) -> Result<(), BoxError>
where W: AsyncWriteExt + Unpin {
    let mut buf = Vec::with_capacity(frames.iter().map(|f| Frame::HEADER_BYTES + f.payload.len()).sum());
    for frame in frames {
        buf.push(frame.kind as u8);
        buf.extend_from_slice(&frame.id.to_be_bytes());
        buf.extend_from_slice(&(frame.payload.len() as Id_t).to_be_bytes());
        buf.extend_from_slice(&frame.payload);
    }
    writer.write_all(&buf).await?;
    Ok(())
}