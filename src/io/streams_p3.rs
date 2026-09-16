use super::{AsyncRead, AsyncWrite};

use wasip3::wit_bindgen::{StreamReader, StreamResult, StreamWriter};

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A wrapper for WASI's `InputStream` resource that provides implementations of `AsyncRead` and
/// `AsyncPollable`.
#[derive(Debug)]
pub struct AsyncInputStream {
    /// TODO: Should this also contain an error future to get the errors?
    stream: StreamReader<u8>,
}

impl AsyncInputStream {
    /// Construct an `AsyncInputStream` from a WASI `InputStream` resource.
    pub fn new(stream: StreamReader<u8>) -> Self {
        Self { stream }
    }

    /// TODO: None of these can return error actually.
    pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let vec = Vec::with_capacity(buf.len());
        let (_result, vec) = self.stream.read(vec).await;
        buf[..vec.len()].copy_from_slice(&vec);
        Ok(vec.len())
    }

    /// TODO: This requires `stream.forward` to avoid copying.
    /// Move the entire contents of an input stream directly into an output
    /// stream, until the input stream has closed. This operation is optimized
    /// to avoid copying stream contents into and out of memory.
    pub async fn copy_to(&mut self, writer: &mut AsyncOutputStream) -> std::io::Result<u64> {
        const CHUNK_SIZE: usize = 1024;
        let mut vec = Vec::with_capacity(CHUNK_SIZE);
        let mut written = 0;
        loop {
            let (result, new_vec) = self.stream.read(vec).await;
            vec = new_vec;
            match result {
                StreamResult::Complete(r) => {
                    writer.write_all(&vec).await?;
                    written += r as u64;
                }
                StreamResult::Dropped | StreamResult::Cancelled => {
                    break;
                }
            }
            vec.clear();
        }
        Ok(written)
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
    /// will be at most 8k. If you want to control chunk size, use
    /// `Self::into_stream_of`.
    pub fn into_stream(self) -> AsyncInputChunkStream {
        AsyncInputChunkStream {
            stream: self,
            chunk_size: 8 * 1024,
        }
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<Vec<u8>, std::io::Error>`. The returned byte vectors
    /// will be at most the `chunk_size` argument specified.
    pub fn into_stream_of(self, chunk_size: usize) -> AsyncInputChunkStream {
        AsyncInputChunkStream {
            stream: self,
            chunk_size,
        }
    }

    /// Use this `AsyncInputStream` as a `futures_lite::stream::Stream` with
    /// items of `Result<u8, std::io::Error>`.
    pub fn into_bytestream(self) -> AsyncInputByteStream {
        AsyncInputByteStream {
            stream: self.into_stream(),
            buffer: std::io::Read::bytes(std::io::Cursor::new(Vec::new())),
        }
    }
}

impl AsyncRead for AsyncInputStream {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Self::read(self, buf).await
    }

    #[inline]
    fn as_async_input_stream(&mut self) -> Option<&mut AsyncInputStream> {
        Some(self)
    }
}

/// It needs to be lent out to read. We could probably do it otherwise, but need a
/// self referential pointer at least.
///
/// Wrapper of `AsyncInputStream` that impls `futures_lite::stream::Stream`
/// with an item of `Result<Vec<u8>, std::io::Error>`
pub struct AsyncInputChunkStream {
    stream: AsyncInputStream,
    chunk_size: usize,
}

impl AsyncInputChunkStream {
    /// Extract the `AsyncInputStream` which backs this stream.
    pub fn into_inner(self) -> AsyncInputStream {
        self.stream
    }
}

impl futures_lite::stream::Stream for AsyncInputChunkStream {
    type Item = Result<Vec<u8>, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // TODO: reuse the vec instead of making a new one each time.
        let vec = Vec::with_capacity(self.chunk_size);
        // TODO: Should we start with a 0 read to check for readiness first?
        let mut read = std::pin::pin!(self.stream.stream.read(vec));
        // TODO: Should I do something with this poll result?
        let _ = read.as_mut().poll(cx);
        let (result, vec) = read.cancel();
        if vec.is_empty() {
            match result {
                // TODO: double check these cases.
                StreamResult::Dropped => Poll::Ready(None),
                StreamResult::Cancelled | StreamResult::Complete(_) => Poll::Pending,
            }
        } else {
            Poll::Ready(Some(Ok(vec)))
        }
    }
}

pin_project_lite::pin_project! {
    /// Wrapper of `AsyncInputStream` that impls
    /// `futures_lite::stream::Stream` with item `Result<u8, std::io::Error>`.
    pub struct AsyncInputByteStream {
        #[pin]
        stream: AsyncInputChunkStream,
        buffer: std::io::Bytes<std::io::Cursor<Vec<u8>>>,
    }
}

impl AsyncInputByteStream {
    /// Extract the `AsyncInputStream` which backs this stream, and any bytes
    /// read from the `AsyncInputStream` which have not yet been yielded by
    /// the byte stream.
    pub fn into_inner(self) -> (AsyncInputStream, Vec<u8>) {
        (
            self.stream.into_inner(),
            self.buffer
                .collect::<Result<Vec<u8>, std::io::Error>>()
                .expect("read of Cursor<Vec<u8>> is infallible"),
        )
    }
}

impl futures_lite::stream::Stream for AsyncInputByteStream {
    type Item = Result<u8, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match this.buffer.next() {
            Some(byte) => Poll::Ready(Some(Ok(byte.expect("cursor on Vec<u8> is infallible")))),
            None => match futures_lite::stream::Stream::poll_next(this.stream, cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let mut bytes = std::io::Read::bytes(std::io::Cursor::new(bytes));
                    match bytes.next() {
                        Some(Ok(byte)) => {
                            *this.buffer = bytes;
                            Poll::Ready(Some(Ok(byte)))
                        }
                        Some(Err(err)) => Poll::Ready(Some(Err(err))),
                        None => Poll::Ready(None),
                    }
                }
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

/// A wrapper for WASI's `output-stream` resource that provides implementations of `AsyncWrite` and
/// `AsyncPollable`.
#[derive(Debug)]
pub struct AsyncOutputStream {
    stream: StreamWriter<u8>,
}

impl AsyncOutputStream {
    /// Construct an `AsyncOutputStream` from a WASI `OutputStream` resource.
    pub fn new(stream: StreamWriter<u8>) -> Self {
        Self { stream }
    }
    /// Asynchronously write to the output stream. This method is the same as
    /// [`AsyncWrite::write`], but doesn't require a `&mut self`.
    ///
    /// Awaits for write readiness, and then performs at most one write to the
    /// output stream. Returns how much of the argument `buf` was written, or
    /// a `std::io::Error` indicating either an error returned by the stream write
    /// using the debug string provided by the WASI error, or else that the,
    /// indicated by `std::io::ErrorKind::ConnectionReset`.
    pub async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut vec = Vec::with_capacity(buf.len());
        vec.extend_from_slice(buf);
        let (_result, abi_buf) = self.stream.write(vec).await;
        let sent = buf.len() - abi_buf.remaining();
        Ok(sent)
    }

    /// Asynchronously write to the output stream. This method is the same as
    /// [`AsyncWrite::write_all`], but doesn't require a `&mut self`.
    pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let mut vec = Vec::with_capacity(buf.len());
        vec.extend_from_slice(buf);
        let (mut status, mut abi_buf) = self.stream.write(vec).await;
        loop {
            match status {
                StreamResult::Cancelled | StreamResult::Dropped => break,
                StreamResult::Complete(_) if abi_buf.remaining() == 0 => break,
                // TODO: Can we see a Complete 0 when we didn't send 0?
                StreamResult::Complete(_) => {}
            }
            let (result, new_buf) = self.stream.write_buf(abi_buf).await;
            status = result;
            abi_buf = new_buf;
        }
        if abi_buf.remaining() != 0 {
            Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
        } else {
            Ok(())
        }
    }

    /// Asyncronously flush the output stream. Initiates a flush, and then
    /// awaits until the flush is complete and the output stream is ready for
    /// writing again.
    ///
    /// This method is the same as [`AsyncWrite::flush`], but doesn't require
    /// a `&mut self`.
    ///
    /// Fails with a `std::io::Error` indicating either an error returned by
    /// the stream flush, using the debug string provided by the WASI error,
    /// or else that the stream is closed, indicated by
    /// `std::io::ErrorKind::ConnectionReset`.
    pub async fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

impl AsyncWrite for AsyncOutputStream {
    // Required methods
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Self::write(self, buf).await
    }
    async fn flush(&mut self) -> std::io::Result<()> {
        Self::flush(self).await
    }

    #[inline]
    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_to_works() {
        crate::runtime::block_on(async {
            let (mut source_writer, source_reader) = wasip3::wit_stream::new();
            let (destination_writer, destination_reader) = wasip3::wit_stream::new();

            let copy = crate::runtime::spawn(async move {
                let mut source = AsyncInputStream::new(source_reader);
                let mut destination = AsyncOutputStream::new(destination_writer);
                let copied = source.copy_to(&mut destination).await.unwrap();
                drop(destination);
                copied
            });
            let collect = crate::runtime::spawn(destination_reader.collect());

            assert!(source_writer.write_all(vec![1]).await.is_empty());
            assert!(source_writer.write_all(vec![2]).await.is_empty());
            assert!(source_writer.write_all(vec![3, 4, 5]).await.is_empty());
            drop(source_writer);

            let copied = copy.await;
            let actual = collect.await;

            assert_eq!(copied, 5);
            assert_eq!(actual, [1, 2, 3, 4, 5]);
        });
    }
}
