use crate::idl;
use crate::idl_helpers::{create_event, current_thread_uuid, DebugAnnotations};
use crate::idl_helpers;
use crate::perfetto_visitor::{PerfettoVisitor, TrackNameVisitor};
use prost::Message;
use tracing::{Event, Id, span};
use tracing_subscriber::layer::Context;

pub(crate) struct SequenceId(u64);

impl SequenceId {
    pub(crate) fn new(n: u64) -> Self {
        Self(n)
    }

    pub(crate) fn get(&self) -> u64 {
        self.0
    }
}

pub(crate) struct TrackUuid(u64);
    
impl TrackUuid {
    pub(crate) fn new(n: u64) -> Self {
        Self(n)
    }

    pub(crate) fn get(&self) -> u64 {
        self.0
    }
}

#[derive(Default)]
pub(crate) struct Config {
    pub(crate) debug_annotations: bool,
    pub(crate) filter: Option<fn(&str) -> bool>,
}

struct PerfettoSpanState {
    track_descriptor: Option<idl::TrackDescriptor>, // optional track descriptor for this span, defaults to thread if not found
    trace: idl::Trace, // The Protobuf trace messages that we accumulate for this span.
}


/// A `Layer` that records span as perfetto's
/// `TYPE_SLICE_BEGIN`/`TYPE_SLICE_END`, and event as `TYPE_INSTANT`.
///
/// `PerfettoLayer` will output the records as encoded [protobuf messages](https://github.com/google/perfetto).
pub struct PerfettoLayer<W = fn() -> std::io::Stdout> {
    pub(crate) sequence_id: SequenceId,
    pub(crate) process_track_uuid: TrackUuid,
    pub(crate) writer: W,
    pub(crate) config: Config,
}

impl<W, S: tracing::Subscriber> tracing_subscriber::Layer<S> for PerfettoLayer<W>
where
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + 'static,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };

        let enabled = self
            .config
            .filter
            .map(|f| {
                let mut visitor = crate::perfetto_visitor::PerfettoVisitor::new(f);
                attrs.record(&mut visitor);
                visitor.perfetto
            })
            .unwrap_or(true);

        if !enabled {
            return;
        }

    let mut debug_annotations = DebugAnnotations::default();
        if self.config.debug_annotations {
            attrs.record(&mut debug_annotations);
        }

        let mut packet = idl::TracePacket::default();

        // check if parent span has a non default track descriptor
        let inherited_track_descriptor = span
            .parent()
            // If the span has a parent, try retrieving the track descriptor from the parent's state
            .and_then(|parent_span| {
                parent_span
                    .extensions()
                    .get::<PerfettoSpanState>()
                    .map(|state| state.track_descriptor.clone())
            })
            .flatten();

        // retrieve the user set track name (via `perfetto.track_name` field)
    let mut user_track_name: Option<String> = None;
    let mut visitor = TrackNameVisitor {
            user_track_name: &mut user_track_name,
        };
        attrs.record(&mut visitor);

        // resolve the optional track descriptor for this span (either inherited from parent or user set, or None)
        let span_track_descriptor = user_track_name
            .map(|name| idl::TrackDescriptor::named_child_for(&name, self.process_track_uuid.get()))
            .or(inherited_track_descriptor);

        let final_uuid = span_track_descriptor
            .as_ref()
            .map(|desc| desc.uuid())
            .unwrap_or_else(current_thread_uuid);

        let event = create_event(
            final_uuid, // span track id if exists, otherwise thread track id
            Some(span.name()),
            span.metadata().file().zip(span.metadata().line()),
            debug_annotations,
            Some(idl::track_event::Type::SliceBegin),
        );
        packet.data = Some(idl::trace_packet::Data::TrackEvent(event));
        packet.timestamp = chrono::Local::now().timestamp_nanos_opt().map(|t| t as _);
        packet.trusted_pid = Some(std::process::id() as _);
        packet.optional_trusted_packet_sequence_id = Some(
            idl::trace_packet::OptionalTrustedPacketSequenceId::TrustedPacketSequenceId(
                self.sequence_id.get() as _,
            ),
        );

        let span_state = PerfettoSpanState {
            track_descriptor: span_track_descriptor,
            trace: idl::Trace {
                packet: vec![packet],
            },
        };
        span.extensions_mut().insert(span_state);
    }

    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(span) else {
            return;
        };

        // We don't check the filter here -- we've already checked it when we handled the span on
        // `on_new_span`. Iff we successfully attached a track packet to the span, then we'll also
        // update the trace packet with the debug data here.
        if let Some(extension) = span.extensions_mut().get_mut::<PerfettoSpanState>() {
            if let Some(idl::trace_packet::Data::TrackEvent(ref mut event)) =
                &mut extension.trace.packet[0].data
            {
                let mut debug_annotations = DebugAnnotations::default();
                values.record(&mut debug_annotations);
                event
                    .debug_annotations
                    .append(&mut debug_annotations.annotations);
            }
        };
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let enabled = self
            .config
            .filter
            .map(|f| {
                let mut visitor = PerfettoVisitor::new(f);
                event.record(&mut visitor);
                visitor.perfetto
            })
            .unwrap_or(true);

        if !enabled {
            return;
        }

        let metadata = event.metadata();
        let location = metadata.file().zip(metadata.line());

        let mut debug_annotations = DebugAnnotations::default();

        if self.config.debug_annotations {
            event.record(&mut debug_annotations);
        }

        let thread_track_uuid = current_thread_uuid();
        let mut track_event = create_event(
            0,
            Some(metadata.name()),
            location,
            debug_annotations,
            Some(idl::track_event::Type::Instant),
        );

        let mut packet = idl::TracePacket {
            trusted_pid: Some(std::process::id() as _),
            timestamp: chrono::Local::now().timestamp_nanos_opt().map(|t| t as _),
            optional_trusted_packet_sequence_id: Some(
                idl::trace_packet::OptionalTrustedPacketSequenceId::TrustedPacketSequenceId(
                    self.sequence_id.get() as _,
                ),
            ),
            ..Default::default()
        };

        if let Some(span) = ctx.event_span(event) {
            if let Some(span_state) = span.extensions_mut().get_mut::<PerfettoSpanState>() {
                track_event.track_uuid = span_state
                    .track_descriptor
                    .as_ref()
                    .map(|d| d.uuid())
                    .or(Some(current_thread_uuid()));
                packet.data = Some(idl::trace_packet::Data::TrackEvent(track_event));
                span_state.trace.packet.push(packet);
                return;
            }
        }

        // no span or no span state, just write the event
        track_event.track_uuid = Some(thread_track_uuid);
        packet.data = Some(idl::trace_packet::Data::TrackEvent(track_event));
        let trace = idl::Trace {
            packet: vec![packet],
        };
    self.write_log(trace, idl_helpers::current_thread_track_descriptor());
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };

        let Some(mut span_state) = span.extensions_mut().remove::<PerfettoSpanState>() else {
            return;
        };

        let debug_annotations = DebugAnnotations::default();

        let track_uuid = span_state
            .track_descriptor
            .as_ref()
            .map(|d| d.uuid())
            .unwrap_or_else(current_thread_uuid);

        let mut packet = idl::TracePacket::default();
        let meta = span.metadata();
        let event = create_event(
            track_uuid,
            Some(meta.name()),
            meta.file().zip(meta.line()),
            debug_annotations,
            Some(idl::track_event::Type::SliceEnd),
        );
        packet.data = Some(idl::trace_packet::Data::TrackEvent(event));
        packet.timestamp = chrono::Local::now().timestamp_nanos_opt().map(|t| t as _);
        packet.trusted_pid = Some(std::process::id() as _);
        packet.optional_trusted_packet_sequence_id = Some(
            idl::trace_packet::OptionalTrustedPacketSequenceId::TrustedPacketSequenceId(
                self.sequence_id.get() as _,
            ),
        );

        span_state.trace.packet.push(packet);

    self.write_log(
            span_state.trace,
            span_state
                .track_descriptor
                .unwrap_or_else(idl_helpers::current_thread_track_descriptor),
        );
    }
}

impl<W: crate::perfetto_writer::PerfettoWriter> crate::perfetto_layer::PerfettoLayer<W> {
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
        let mut buf = bytes::BytesMut::new();

        if let Some(p) = idl_helpers::process_descriptor(self.process_track_uuid.get()) {
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