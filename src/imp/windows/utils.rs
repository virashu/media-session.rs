use windows::Storage::Streams::{
    Buffer as WRT_Buffer, DataReader as WRT_DataReader,
    IRandomAccessStreamReference as WRT_IStreamRef,
    IRandomAccessStreamWithContentType as WRT_IStream, InputStreamOptions,
};

pub async fn stream_ref_to_bytes(stream_ref: WRT_IStreamRef) -> crate::Result<Vec<u8>> {
    let readable_stream: WRT_IStream = stream_ref.OpenReadAsync()?.await?;
    #[allow(clippy::cast_possible_truncation)]
    let read_size = readable_stream.Size()? as u32;
    let buffer: WRT_Buffer = WRT_Buffer::Create(read_size)?;

    let ib = readable_stream
        .ReadAsync(&buffer, read_size, InputStreamOptions::ReadAhead)?
        .await?;

    let reader: WRT_DataReader = WRT_DataReader::FromBuffer(&ib)?;
    let len = ib.Length()? as usize;
    let mut rv: Vec<u8> = vec![0; len];
    let res: &mut [u8] = rv.as_mut_slice();

    reader.ReadBytes(res)?;

    Ok(rv)
}
