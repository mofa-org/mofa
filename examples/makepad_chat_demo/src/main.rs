use makepad_chat::*;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_chat::chat_bubble::ChatBubble;

    App = {{App}} {
        ui: <Window> {
            body = <View> {
                flow: Down,
                width: Fill,
                height: Fill,

                chat_list = <ScrollYView> {
                    width: Fill,
                    height: Fill,
                    flow: Down,

                    user_msg = <ChatBubble> {
                        role: User,
                    }

                    assistant_msg = <ChatBubble> {
                        role: Assistant,
                    }
                }

                input_bar = <View> {
                    width: Fill,
                    height: Fit,
                    padding: 10,
                    spacing: 10,

                    button = <Button> {
                        text: "Simulate Assistant Response"
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    initialized: bool,
    #[rust]
    streaming_timer: Timer,
    #[rust]
    streaming_content: String,
    #[rust]
    current_token_index: usize,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);
        makepad_chat::live_design(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, _cx: &mut Cx) {}

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(id!(button)).clicked(actions) {
            self.start_streaming_simulation(cx);
        }
    }

    fn handle_timer(&mut self, cx: &mut Cx, event: &TimerEvent) {
        if self.streaming_timer.is_timer(event) {
            self.update_streaming_simulation(cx);
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

impl App {
    fn start_streaming_simulation(&mut self, cx: &mut Cx) {
        if self.streaming_timer != Timer::default() {
            return;
        }

        let chat_list = self.ui.view(id!(chat_list));
        
        // Reset user message
        if let Some(mut bubble) = chat_list.widget(id!(user_msg)).borrow_mut::<ChatBubble>() {
            bubble.set_text(cx, "Can you show me a Rust example with markdown?");
        }

        // Prepare assistant streaming
        self.streaming_content = "Sure! Here's a **Rust** example:\n\n```rust\nfn main() {\n    println!(\"Hello MoFA!\");\n}\n```\n\nHope this helps!".to_string();
        self.current_token_index = 0;
        
        if let Some(mut bubble) = chat_list.widget(id!(assistant_msg)).borrow_mut::<ChatBubble>() {
            bubble.start_streaming(cx, "");
        }

        self.streaming_timer = cx.start_timer(0.05, true);
    }

    fn update_streaming_simulation(&mut self, cx: &mut Cx) {
        if self.current_token_index < self.streaming_content.len() {
            // Simulate variable token sizes
            let end = (self.current_token_index + 3).min(self.streaming_content.len());
            let token = &self.streaming_content[self.current_token_index..end];
            
            let chat_list = self.ui.view(id!(chat_list));
            if let Some(mut bubble) = chat_list.widget(id!(assistant_msg)).borrow_mut::<ChatBubble>() {
                bubble.add_token(cx, token);
            }
            
            self.current_token_index = end;
        } else {
            // Once finished, convert to full markdown for final rendering
            let chat_list = self.ui.view(id!(chat_list));
            if let Some(mut bubble) = chat_list.widget(id!(assistant_msg)).borrow_mut::<ChatBubble>() {
                bubble.set_text(cx, &self.streaming_content);
            }
            cx.stop_timer(self.streaming_timer);
            self.streaming_timer = Timer::default();
        }
        cx.redraw_all();
    }
}

fn main() {
    app_main!(App);
}
