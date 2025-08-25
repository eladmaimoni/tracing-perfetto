use bytes::BytesMut;
use prost::Message;
use std::io::Write;
use crate::idl;
use crate::idl_helpers::process_descriptor;
use crate::perfetto_layer::{Config, SequenceId, TrackUuid};


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



impl<W: PerfettoWriter> crate::perfetto_layer::PerfettoLayer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            sequence_id: SequenceId::new(rand::random()),
            process_track_uuid: TrackUuid::new(rand::random()),
            writer,
            config: Config::default(),
        }
    }

    /// Configures whether or not spans/events should be recorded with their metadata and fields.
    pub fn with_debug_annotations(mut self, value: bool) -> Self {
        self.config.debug_annotations = value;
        self
    }

    /// Configures whether or not spans/events be recorded based on the occurrence of a field name.
    ///
    /// Sometimes, not all the events/spans should be treated as perfetto trace, you can append a
    /// field to indicate that this even/span should be captured into trace:
    ///
    /// ```rust
    /// use tracing_perfetto::PerfettoLayer;
    /// use tracing_subscriber::{layer::SubscriberExt, Registry, prelude::*};
    ///
    /// let layer = PerfettoLayer::new(std::fs::File::open("/tmp/test.pftrace").unwrap())
    ///                 .with_filter_by_marker(|field_name| field_name == "perfetto");
    /// tracing_subscriber::registry().with(layer).init();
    ///
    /// // this event will be record, as it contains a `perfetto` field
    /// tracing::info!(perfetto = true, my_bool = true);
    ///
    /// // this span will be record, as it contains a `perfetto` field
    /// #[tracing::instrument(fields(perfetto = true))]
    /// fn to_instr() {
    ///
    ///   // this event will be ignored
    ///   tracing::info!(my_bool = true);
    /// }
    /// ```
    pub fn with_filter_by_marker(mut self, filter: fn(&str) -> bool) -> Self {
        self.config.filter = Some(filter);
        self
    }

    pub(crate) fn write_log(&self, mut log: idl::Trace, track_descriptor: idl::TrackDescriptor) {
        let mut buf = BytesMut::new();

        if let Some(p) = process_descriptor(self.process_track_uuid.get()) {
            log.packet.insert(0, p);
        }

        let packet = idl::TracePacket {
            data: Some(idl::trace_packet::Data::TrackDescriptor(track_descriptor)),
            ..Default::default()
        };
        // let mut packet = idl::TracePacket::default();
        // packet.data = Some(idl::trace_packet::Data::TrackDescriptor(track_descriptor));
        log.packet.insert(1, packet);

        // if let Some(t) = track_descriptor {
        //     let mut packet = idl::TracePacket::default();
        //     packet.data = Some(idl::trace_packet::Data::TrackDescriptor(t));
        //     log.packet.insert(1, packet);
        // } else if let Some(t) = self.thread_descriptor() {
        //     log.packet.insert(1, t);
        // }

    let Ok(_) = log.encode(&mut buf) else {
            return;
        };
    _ = self.writer.write_log(buf);
    }
}