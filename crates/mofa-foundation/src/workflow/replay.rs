use super::recorder::{ExecutionStep, WorkflowRecording};

pub struct WorkflowReplayer {
    recording: WorkflowRecording,
    current_step: usize,
}

impl WorkflowReplayer {
    pub fn new(recording: WorkflowRecording) -> Self {
        Self {
            recording,
            current_step: 0,
        }
    }

    pub fn step_forward(&mut self) -> Option<&ExecutionStep> {
        if self.current_step < self.recording.steps.len() {
            let step = &self.recording.steps[self.current_step];
            self.current_step += 1;
            Some(step)
        } else {
            None
        }
    }

    pub fn step_backward(&mut self) -> Option<&ExecutionStep> {
        if self.current_step > 0 {
            self.current_step -= 1;
            Some(&self.recording.steps[self.current_step])
        } else {
            None
        }
    }

    pub fn jump_to_step(&mut self, step: usize) -> Option<&ExecutionStep> {
        if step < self.recording.steps.len() {
            self.current_step = step;
            Some(&self.recording.steps[self.current_step])
        } else {
            None
        }
    }

    pub fn inspect_state(&self) -> Option<&ExecutionStep> {
        if self.current_step < self.recording.steps.len() {
            Some(&self.recording.steps[self.current_step])
        } else if !self.recording.steps.is_empty() {
             Some(&self.recording.steps[self.recording.steps.len() - 1])
        } else {
            None
        }
    }

    pub fn get_recording(&self) -> &WorkflowRecording {
        &self.recording
    }
    
    pub fn current_step_index(&self) -> usize {
        self.current_step
    }
    
    pub fn total_steps(&self) -> usize {
        self.recording.steps.len()
    }
}
