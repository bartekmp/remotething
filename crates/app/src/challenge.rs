//! Keyboard-interactive sign-in: the questions the server asks (password,
//! one-time code) and the form that collects the answers.

use std::sync::{Arc, Mutex};

use iced::futures::stream;
use iced::widget::{button, column, row, text};
use iced::{Element, Task};
use remotething_remote::ssh::{Auth, Challenge, Prompt};
use remotething_secrets::Secret;
use tokio::sync::{mpsc, oneshot};

use crate::credentials::Hidden;
use crate::i18n::fl;
use crate::session::Message;
use crate::theme;

/// A question round on its way to the window. It is taken out once.
#[derive(Clone)]
pub struct Asked(Arc<Mutex<Option<Challenge>>>);

impl std::fmt::Debug for Asked {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Asked")
    }
}

impl Asked {
    pub fn new(challenge: Challenge) -> Asked {
        Asked(Arc::new(Mutex::new(Some(challenge))))
    }

    pub fn take(&self) -> Option<Challenge> {
        self.0.lock().ok()?.take()
    }
}

/// The way to sign in, and the questions it will send.
pub fn auth(username: String) -> (Auth, mpsc::Receiver<Challenge>) {
    let (challenges, questions) = mpsc::channel(1);
    let auth = Auth::Interactive {
        username,
        challenges,
    };
    (auth, questions)
}

/// Turns the questions into messages. The stream ends once the sign-in is over.
pub fn questions(questions: mpsc::Receiver<Challenge>) -> Task<Message> {
    let asked = stream::unfold(questions, |mut questions| async move {
        let challenge = questions.recv().await?;
        Some((Asked::new(challenge), questions))
    });
    Task::run(asked, Message::Challenged)
}

/// One round of questions with what has been typed so far.
pub struct Form {
    name: String,
    instructions: String,
    prompts: Vec<Prompt>,
    answers: Vec<String>,
    reply: oneshot::Sender<Vec<Secret>>,
}

impl Form {
    pub fn new(challenge: Challenge) -> Form {
        Form {
            answers: vec![String::new(); challenge.prompts.len()],
            name: challenge.name,
            instructions: challenge.instructions,
            prompts: challenge.prompts,
            reply: challenge.reply,
        }
    }

    pub fn set_answer(&mut self, index: usize, text: String) {
        if let Some(answer) = self.answers.get_mut(index) {
            *answer = text;
        }
    }

    /// Sends the answers to the sign-in. Dropping the form instead cancels it.
    pub fn submit(self) {
        let answers = self.answers.into_iter().map(Secret::new).collect();
        // A closed receiver means the sign-in is already over.
        let _ = self.reply.send(answers);
    }
}

pub fn view<'a>(title: &str, form: &'a Form) -> Element<'a, Message> {
    let mut fields = column![text(title.to_string()).size(theme::TEXT_TITLE)].spacing(10);
    for line in [&form.name, &form.instructions] {
        if !line.trim().is_empty() {
            fields = fields.push(text(line.as_str()).style(theme::secondary_text));
        }
    }
    for (index, prompt) in form.prompts.iter().enumerate() {
        let hint = prompt.text.trim().trim_end_matches(':').to_string();
        let input = theme::input(&hint, &form.answers[index])
            .secure(!prompt.echo)
            .on_input(move |typed| Message::AnswerChanged(index, Hidden(typed)))
            .on_submit(Message::AnswersSubmitted);
        fields = fields.push(input);
    }
    let buttons = row![
        button(text(fl!("session-connect")))
            .style(theme::primary)
            .on_press(Message::AnswersSubmitted),
        button(text(fl!("session-cancel")))
            .style(theme::pill)
            .on_press(Message::AnswersCancelled),
    ]
    .spacing(8);
    fields.push(buttons).into()
}
