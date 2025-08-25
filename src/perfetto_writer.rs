use std::io::Write;


/// Writes encoded records into provided instance.
///
/// This is implemented for types implements [`MakeWriter`].
pub trait PerfettoWriter {
    fn write_log(&self, buf: bytes::BytesMut) -> std::io::Result<()>;
}

impl<W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + 'static> PerfettoWriter for W {
    fn write_log(&self, buf: bytes::BytesMut) -> std::io::Result<()> {
        self.make_writer().write_all(&buf)
    }
}



