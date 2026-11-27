use cursive::theme::{BaseColor, Color};

#[derive(Debug, Clone, Copy)]
pub enum MessageKind {
    Error,
    Info,
    Hint,
}

impl From<MessageKind> for Color {
    fn from(item: MessageKind) -> Self {
        match item {
            MessageKind::Error => Color::Dark(BaseColor::Red),
            MessageKind::Info => Color::Dark(BaseColor::Yellow),
            MessageKind::Hint => Color::Dark(BaseColor::White),
        }
    }
}

impl<T> From<T> for Message
where
    T: AsRef<str>,
{
    fn from(item: T) -> Self {
        Message {
            msg: item.as_ref().to_string(),
            kind: MessageKind::Info,
        }
    }
}

pub struct Message {
    msg: String,
    kind: MessageKind,
}

impl Message {
    pub fn startup() -> Self {
        "Welcome to your habit tracker".into()
    }
    pub fn contents(&self) -> &str {
        &self.msg
    }
    pub fn kind(&self) -> MessageKind {
        self.kind
    }
    pub fn set_kind(&mut self, k: MessageKind) {
        self.kind = k;
    }
    pub fn set_message<S: AsRef<str>>(&mut self, m: S) {
        self.msg = m.as_ref().into();
    }
    /// Empty the message and go back to the info kind, so the next message
    /// does not take the color of an earlier error.
    pub fn clear(&mut self) {
        self.msg.clear();
        self.kind = MessageKind::Info;
    }
}

impl std::default::Default for Message {
    fn default() -> Self {
        Message {
            msg: String::new(),
            kind: MessageKind::Info,
        }
    }
}
