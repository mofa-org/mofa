use makepad_widgets::*;
use crate::markdown_renderer::MarkdownRenderer;
use crate::streaming_text::StreamingText;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::markdown_renderer::MarkdownRenderer;
    import crate::streaming_text::StreamingText;

    ChatBubble = {{ChatBubble}} {
        width: Fill,
        height: Fit,
        margin: {left: 10, right: 10, top: 5, bottom: 5},
        padding: 10,
        
        draw_bg: {
            fn pixel(self) -> vec4 {
                return self.color
            }
        }

        layout: {
            align: {x: 0.0, y: 0.0},
            flow: Down,
        }


        markdown = <MarkdownRenderer> { visible: true }
        streaming = <StreamingText> { visible: false }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ChatBubble {
    #[deref] view: View,
    #[live] pub role: ChatRole,
}

#[derive(Live, LiveHook, Clone, Copy, PartialEq)]
pub enum ChatRole {
    #[pick] User,
    Assistant,
    System,
}

impl Default for ChatRole {
    fn default() -> Self { Self::User }
}

impl Widget for ChatBubble {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        match self.role {
            ChatRole::User => {
                self.view.apply_over(cx, live! {
                    draw_bg: {color: #x4444ff},
                    layout: {align: {x: 1.0, y: 0.0}},
                });
            }
            ChatRole::Assistant => {
                self.view.apply_over(cx, live! {
                    draw_bg: {color: #x444444},
                    layout: {align: {x: 0.0, y: 0.0}},
                });
            }
            ChatRole::System => {
                self.view.apply_over(cx, live! {
                    draw_bg: {color: #x222222},
                    layout: {align: {x: 0.5, y: 0.0}},
                });
            }
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ChatBubble {
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.view.widget(id!(streaming)).set_visible(false);
        self.view.widget(id!(markdown)).set_visible(true);
        if let Some(mut markdown) = self.view.widget(id!(markdown)).borrow_mut::<MarkdownRenderer>() {
            markdown.render_markdown(cx, text);
        } else {
            self.label(id!(label)).set_text(text);
        }
    }

    pub fn start_streaming(&mut self, cx: &mut Cx, text: &str) {
        self.view.widget(id!(markdown)).set_visible(false);
        self.view.widget(id!(streaming)).set_visible(true);
        if let Some(mut streaming) = self.view.widget(id!(streaming)).borrow_mut::<StreamingText>() {
            streaming.start_streaming(cx, text);
        }
    }

    pub fn add_token(&mut self, cx: &mut Cx, token: &str) {
        if let Some(mut streaming) = self.view.widget(id!(streaming)).borrow_mut::<StreamingText>() {
            streaming.add_token(cx, token);
        }
    }
}
