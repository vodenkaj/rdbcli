use std::time::SystemTime;

use ratatui::widgets::{Paragraph, StatefulWidget, Widget};

pub struct Throbber {
    steps: Vec<String>,
    message: Option<String>,
}

pub struct ThrobberState {
    current_frame: usize,
    max_frames: usize,
    frame_rate: usize,
    time_elapsed: SystemTime,
}

impl Throbber {
    pub fn new(steps: Vec<String>, message: Option<String>) -> Self {
        Self { steps, message }
    }
}

impl StatefulWidget for Throbber {
    type State = ThrobberState;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        let step_index = state.progress();

        Paragraph::new(format!(
            "{} {}",
            self.message.unwrap_or(String::new()),
            self.steps[step_index].clone()
        ))
        .render(area, buf);
    }
}

impl ThrobberState {
    pub fn new(max_steps: usize, frame_rate: usize) -> Self {
        Self {
            current_frame: 0,
            max_frames: max_steps,
            frame_rate,
            time_elapsed: SystemTime::now(),
        }
    }

    pub fn progress(&mut self) -> usize {
        let current_step = self.current_frame;

        let now = SystemTime::now();
        let updated_at = self.time_elapsed;

        if let Ok(diff) = now.duration_since(updated_at) {
            let diff_secs = (diff.as_millis() / 1000) as usize;

            let prev_frame = self.current_frame;
            self.current_frame =
                ((diff_secs * self.frame_rate) / self.max_frames) % self.max_frames;

            if prev_frame != 0 && self.current_frame == 0 {
                self.time_elapsed = now;
            }
        }

        current_step
    }
}

pub fn get_throbber_data() -> (Vec<String>, ThrobberState) {
    let loader_steps: Vec<String> = vec!["⠧", "⠏", "⠛", "⠹", "⠼", "⠶"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let state = ThrobberState::new(loader_steps.len(), 10);

    (loader_steps, state)
}
