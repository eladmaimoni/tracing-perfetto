pub(crate) struct TrackNameVisitor<'a> {
    pub(crate) user_track_name: &'a mut Option<String>,
}

impl tracing::field::Visit for TrackNameVisitor<'_> {
    // fn record_u64(&mut self, field: &Field, value: u64) {
    //     if field.name() == "perfetto_track_id" {
    //         *self.user_track_id = Some(value);
    //     }
    // }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "perfetto.track_name" {
            *self.user_track_name = Some(value.to_string());
        }
    }
    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
        // If you want to parse `perfetto_track_id` from a non-u64 typed field,
        // you could do that here, e.g. if user sets `perfetto_track_id = "0xABCD"`.
        // For now, we'll ignore it.
    }
    // Optionally implement record_* for other numeric types if needed
}
pub(crate) struct PerfettoVisitor {
    pub(crate) perfetto: bool,
    filter: fn(&str) -> bool,
}

impl PerfettoVisitor {
    pub(crate) fn new(filter: fn(&str) -> bool) -> PerfettoVisitor {
        Self {
            filter,
            perfetto: false,
        }
    }
}

impl tracing::field::Visit for PerfettoVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
        if (self.filter)(field.name()) {
            self.perfetto = true;
        }
    }
}

