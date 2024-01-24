use ratatui::widgets::Paragraph;

pub struct Input<'a> {
    pub value: String,
    pub component: Paragraph<'a>,
}

impl<'a> Default for Input<'a> {
    fn default() -> Self {
        Self {
            value: String::new(),
            component: Paragraph::new(""),
        }
    }
}

impl<'a> Input<'a> {
    pub fn on_change(&mut self, value: &str) {
        self.value = String::from(value);
        self.component = Paragraph::new(self.value.clone());
    }
}
