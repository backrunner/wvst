use serde::Serialize;

use crate::{
    Vst3OutputEvent, Vst3OutputEventStats, Vst3OutputParameterChangeStats, Vst3ParameterChange,
};

#[derive(Debug, Default)]
pub struct Vst3ProcessOutput {
    pub events: Vec<Vst3OutputEvent>,
    pub parameter_changes: Vec<Vst3ParameterChange>,
    pub diagnostics: Vst3ProcessOutputDiagnostics,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ProcessOutputDiagnostics {
    pub output_events: Vst3OutputEventStats,
    pub output_parameter_changes: Vst3OutputParameterChangeStats,
}

impl Vst3ProcessOutput {
    pub fn with_capacities(max_events: usize, max_parameter_changes: usize) -> Self {
        Self {
            events: Vec::with_capacity(max_events),
            parameter_changes: Vec::with_capacity(max_parameter_changes),
            diagnostics: Vst3ProcessOutputDiagnostics::default(),
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.parameter_changes.clear();
        self.diagnostics = Vst3ProcessOutputDiagnostics::default();
    }
}
