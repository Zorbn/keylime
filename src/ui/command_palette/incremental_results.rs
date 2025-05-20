use std::{time::Instant, usize};

use super::{CommandPalette, CommandPaletteResult, MAX_VISIBLE_RESULTS};

#[derive(Debug, PartialEq, Eq)]
enum IncrementalResultsState {
    None,
    Partial,
    All,
}

const TARGET_STEP_TIME: f32 = 0.005;

#[derive(Debug, PartialEq, Eq)]
pub enum IncrementalStepState {
    InProgress,
    DoneWithStep,
    DoneWithAllSteps,
}

pub struct IncrementalResults {
    max_result_len: usize,
    results_state: IncrementalResultsState,
    pending_results: Vec<CommandPaletteResult>,
}

impl IncrementalResults {
    pub fn new(max_results_len: Option<usize>) -> Self {
        Self {
            max_result_len: max_results_len.unwrap_or(usize::MAX),
            results_state: IncrementalResultsState::None,
            pending_results: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        self.pending_results.clear();
        self.results_state = IncrementalResultsState::None;
    }

    pub fn push(&mut self, result: CommandPaletteResult) {
        self.pending_results.push(result);
    }

    pub fn finish(&mut self, command_palette: &mut CommandPalette) {
        self.flush_pending_results(command_palette);
        self.results_state = IncrementalResultsState::All;
    }

    pub fn try_finish(
        &mut self,
        start_time: Instant,
        command_palette: &mut CommandPalette,
    ) -> IncrementalStepState {
        if self.pending_results.len() >= self.max_result_len {
            self.finish(command_palette);

            return IncrementalStepState::DoneWithAllSteps;
        }

        if self.pending_results.len() >= MAX_VISIBLE_RESULTS {
            self.flush_pending_results(command_palette);
        }

        if start_time.elapsed().as_secs_f32() > TARGET_STEP_TIME {
            IncrementalStepState::DoneWithStep
        } else {
            IncrementalStepState::InProgress
        }
    }

    fn flush_pending_results(&mut self, command_palette: &mut CommandPalette) {
        if self.results_state == IncrementalResultsState::None {
            self.results_state = IncrementalResultsState::Partial;
            command_palette.result_list.drain();
        }

        command_palette
            .result_list
            .results
            .append(&mut self.pending_results);
    }

    pub fn is_finished(&self) -> bool {
        self.results_state == IncrementalResultsState::All
    }
}
