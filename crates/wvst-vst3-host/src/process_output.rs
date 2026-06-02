use crate::{Vst3OutputEvent, Vst3ParameterChange};

#[derive(Debug, Default)]
pub struct Vst3ProcessOutput {
    pub events: Vec<Vst3OutputEvent>,
    pub parameter_changes: Vec<Vst3ParameterChange>,
}

impl Vst3ProcessOutput {
    pub fn with_capacities(max_events: usize, max_parameter_changes: usize) -> Self {
        Self {
            events: Vec::with_capacity(max_events),
            parameter_changes: Vec::with_capacity(max_parameter_changes),
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.parameter_changes.clear();
    }
}
