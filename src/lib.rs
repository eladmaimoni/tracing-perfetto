#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![forbid(unsafe_code)]



#[path = "perfetto.protos.rs"]
#[allow(clippy::all)]
#[rustfmt::skip]
mod idl;

mod idl_helpers;
mod perfetto_layer;
mod perfetto_visitor;
mod perfetto_writer;    

pub use perfetto_layer::PerfettoLayer;










// macro_rules! impl_record {
//     ($method:ident, $type:ty, $value_variant:ident) => {
//         fn $method(&mut self, field: &Field, value: $type) {
//             let mut annotation = idl::DebugAnnotation::default();
//             annotation.name_field = Some(idl::debug_annotation::NameField::Name(
//                 field.name().to_string(),
//             ));
//             annotation.value = Some(idl::debug_annotation::Value::$value_variant(value.into()));
//             self.annotations.push(annotation);
//         }
//     };
//     ($method:ident, $type:ty, $value_variant:ident, $conversion:expr) => {
//         fn $method(&mut self, field: &Field, value: $type) {
//             let mut annotation = idl::DebugAnnotation::default();
//             annotation.name_field = Some(idl::debug_annotation::NameField::Name(
//                 field.name().to_string(),
//             ));
//             annotation.value = Some(idl::debug_annotation::Value::$value_variant($conversion(
//                 value,
//             )));
//             self.annotations.push(annotation);
//         }
//     };
// }

// impl Visit for DebugAnnotations {
//     impl_record!(record_bool, bool, BoolValue);
//     impl_record!(record_str, &str, StringValue, String::from);
//     impl_record!(record_f64, f64, DoubleValue);
//     impl_record!(record_i64, i64, IntValue);
//     impl_record!(record_i128, i128, StringValue, |v: i128| v.to_string());
//     impl_record!(record_u128, u128, StringValue, |v: u128| v.to_string());
//     impl_record!(record_u64, u64, IntValue, |v: u64| v as i64);

//     fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
//         let annotation = idl::DebugAnnotation {
//             name_field: Some(idl::debug_annotation::NameField::Name(
//                 field.name().to_string(),
//             )),
//             value: Some(idl::debug_annotation::Value::StringValue(format!(
//                 "{value:?}"
//             ))),
//             ..Default::default()
//         };
//         // let mut annotation = idl::DebugAnnotation::default();
//         // annotation.name_field = Some(idl::debug_annotation::NameField::Name(
//         //     field.name().to_string(),
//         // ));
//         // annotation.value = Some(idl::debug_annotation::Value::StringValue(format!(
//         //     "{value:?}"
//         // )));
//         self.annotations.push(annotation);
//     }

//     fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
//         let annotation = idl::DebugAnnotation {
//             name_field: Some(idl::debug_annotation::NameField::Name(
//                 field.name().to_string(),
//             )),
//             value: Some(idl::debug_annotation::Value::StringValue(format!(
//                 "{value}"
//             ))),
//             ..Default::default()
//         };

//         self.annotations.push(annotation);
//     }
// }

// #[cfg(test)]
// mod tests {
//     use std::sync::Arc;
//     use std::sync::Mutex;

//     use tracing::{field, trace_span};
//     use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt};

//     use crate::idl;
//     use crate::idl::track_event;
//     use crate::PerfettoLayer;
//     use prost::Message;

//     /// A Sink for testing that can be passed to PerfettoLayer::new to write trace data to. The
//     /// sink just accumulates the trace data into a buffer in memory. The data will be
//     /// `idl::Trace` protobufs which can be `.decode`'ed.
//     struct TestWriter {
//         buf: Arc<Mutex<Vec<u8>>>,
//     }

//     impl TestWriter {
//         fn new() -> Self {
//             Self {
//                 buf: Arc::new(Mutex::new(Vec::new())),
//             }
//         }
//     }

//     impl<'a> MakeWriter<'a> for TestWriter {
//         type Writer = TestWriter;
//         fn make_writer(&'a self) -> Self::Writer {
//             TestWriter {
//                 buf: self.buf.clone(),
//             }
//         }
//     }

//     impl std::io::Write for TestWriter {
//         fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//             self.buf.lock().unwrap().extend_from_slice(buf);
//             std::io::Result::Ok(buf.len())
//         }

//         fn flush(&mut self) -> std::io::Result<()> {
//             // There's nothing to flush, we always immediately append to the buffer.
//             std::io::Result::Ok(())
//         }
//     }

//     // Check that we are able to write a span and confirm that it's written as protobuf data to the
//     // output
//     #[test]
//     fn test_simple_span() {
//         let writer = TestWriter::new();
//         let extra_writer = writer.make_writer();
//         let perfetto_layer = PerfettoLayer::new(writer).with_debug_annotations(true);
//         let subscriber = tracing_subscriber::registry().with(perfetto_layer);
//         let _guard = tracing::subscriber::set_default(subscriber);
//         {
//             let demo_span = trace_span!("simple_span",);
//             let _enter = demo_span.enter();
//         }
//         assert!(extra_writer.buf.lock().unwrap().len() > 0);
//         let trace = idl::Trace::decode(extra_writer.buf.lock().unwrap().as_slice()).unwrap();

//         let mut track_events_seen = 0;
//         let mut saw_slice_begin = false;
//         let mut saw_slice_end = false;
//         // Depending on test ordering, we may or may not see a process descriptor
//         for packet in trace.packet {
//             let Some(idl::trace_packet::Data::TrackEvent(ref event)) = packet.data else {
//                 continue;
//             };
//             track_events_seen += 1;
//             let expected = Some(track_event::NameField::Name(String::from("simple_span")));
//             assert_eq!(event.name_field, expected);

//             match event.r#type() {
//                 track_event::Type::SliceBegin => saw_slice_begin = true,
//                 track_event::Type::SliceEnd => saw_slice_end = true,
//                 _ => unreachable!("Unexpected track event"),
//             }
//         }
//         assert_eq!(track_events_seen, 2);
//         assert!(saw_slice_begin);
//         assert!(saw_slice_end);
//     }

//     // Check that we are able to write arguments to a span correctly
//     #[test]
//     fn test_span_arguments() {
//         let writer = TestWriter::new();
//         let extra_writer = writer.make_writer();
//         let perfetto_layer = PerfettoLayer::new(writer)
//             .with_debug_annotations(true)
//             .with_filter_by_marker(|s| s == "regular_arg");

//         let subscriber = tracing_subscriber::registry().with(perfetto_layer);
//         let _guard = tracing::subscriber::set_default(subscriber);
//         {
//             let demo_span = trace_span!(
//                 "span_with_args",
//                 regular_arg = "Arg data",
//                 extra_arg = field::Empty
//             );
//             let _enter = demo_span.enter();
//             demo_span.record("extra_arg", "Some Extra Data");
//         }
//         assert!(extra_writer.buf.lock().unwrap().len() > 0);
//         let trace = idl::Trace::decode(extra_writer.buf.lock().unwrap().as_slice()).unwrap();

//         let mut track_events_seen = 0;
//         let mut saw_slice_begin = false;
//         let mut saw_slice_end = false;
//         // Depending on test ordering, we may or may not see a process descriptor
//         for packet in trace.packet {
//             let Some(idl::trace_packet::Data::TrackEvent(ref event)) = packet.data else {
//                 continue;
//             };
//             track_events_seen += 1;
//             let expected = Some(track_event::NameField::Name(String::from("span_with_args")));
//             assert_eq!(event.name_field, expected);

//             match event.r#type() {
//                 track_event::Type::SliceBegin => {
//                     saw_slice_begin = true;

//                     // The SliceBegin isn't recorded until it's dropped, so both the args are added to the
//                     // SliceBegin record.
//                     assert_eq!(event.debug_annotations.len(), 2);
//                     assert_eq!(
//                         event.debug_annotations[0].name_field,
//                         Some(idl::debug_annotation::NameField::Name(
//                             "regular_arg".to_string(),
//                         ))
//                     );
//                     assert_eq!(
//                         event.debug_annotations[0].value,
//                         Some(idl::debug_annotation::Value::StringValue(
//                             "Arg data".to_string(),
//                         ))
//                     );
//                     assert_eq!(
//                         event.debug_annotations[1].name_field,
//                         Some(idl::debug_annotation::NameField::Name(
//                             "extra_arg".to_string(),
//                         ))
//                     );
//                     assert_eq!(
//                         event.debug_annotations[1].value,
//                         Some(idl::debug_annotation::Value::StringValue(
//                             "Some Extra Data".to_string(),
//                         ))
//                     );
//                 }
//                 track_event::Type::SliceEnd => {
//                     saw_slice_end = true;
//                     // The SliceEnd won't have any arguments
//                     assert_eq!(event.debug_annotations.len(), 0);
//                 }
//                 _ => unreachable!("Unexpected track event"),
//             }
//         }
//         assert_eq!(track_events_seen, 2);
//         assert!(saw_slice_begin);
//         assert!(saw_slice_end);
//     }

//     // If all our spans are filtered, we shouldn't get any trace data at all. Doing a `.record` on
//     // a span should also "fail successfully".
//     #[test]
//     fn test_span_arguments_filtered() {
//         let writer = TestWriter::new();
//         let extra_writer = writer.make_writer();
//         let perfetto_layer = PerfettoLayer::new(writer)
//             .with_debug_annotations(true)
//             .with_filter_by_marker(|s| s == "NO SUCH ARG");

//         let subscriber = tracing_subscriber::registry().with(perfetto_layer);
//         let _guard = tracing::subscriber::set_default(subscriber);
//         {
//             let demo_span = trace_span!(
//                 "span_with_args",
//                 regular_arg = "Arg data",
//                 extra_arg = field::Empty
//             );
//             let _enter = demo_span.enter();
//             demo_span.record("extra_arg", "Some Extra Data");
//         }
//         assert_eq!(extra_writer.buf.lock().unwrap().len(), 0);
//     }
// }
