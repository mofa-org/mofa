use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    StreamingText = {{StreamingText}} {
        width: Fill,
        height: Fit,
        draw_text: {
            color: #x fff
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct StreamingText {
    #[deref] label: Label,
    #[live] full_text: String,
    #[live] displayed_length: usize,
    #[rust] last_anim_time: f64,
}

impl Widget for StreamingText {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let Event::Signal = event {
            if self.displayed_length < self.full_text.len() {
                self.displayed_length += 1;
                let text = &self.full_text[..self.displayed_length];
                self.label.set_text(text);
                self.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.label.draw_walk(cx, scope, walk)
    }
}

impl StreamingText {
    pub fn start_streaming(&mut self, cx: &mut Cx, text: &str) {
        self.full_text = text.to_string();
        self.displayed_length = self.full_text.len();
        self.label.set_text(&self.full_text);
        self.redraw(cx);
    }

    pub fn add_token(&mut self, cx: &mut Cx, token: &str) {
        self.full_text.push_str(token);
        self.displayed_length = self.full_text.len();
        self.label.set_text(&self.full_text);
        self.redraw(cx);
    }
}
