use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: usize,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub body: String,
    pub sent_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email {
    pub id: usize,
    pub subject: String,
    pub sender: String,
    pub preview: String,
    pub folders: HashSet<Folder>,
    pub is_read: bool,
    pub is_flagged: bool,
    pub labels: Vec<String>,
    pub messages: Vec<EmailMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Folder {
    Inbox,
    Starred,
    Sent,
    Drafts,
    Trash,
    Custom(String),
}

impl Default for Folder {
    fn default() -> Self {
        Folder::Inbox
    }
}

impl ToString for Folder {
    fn to_string(&self) -> String {
        match self {
            Folder::Inbox => "Inbox".into(),
            Folder::Starred => "Starred".into(),
            Folder::Sent => "Sent".into(),
            Folder::Drafts => "Drafts".into(),
            Folder::Trash => "Trash".into(),
            Folder::Custom(s) => s.clone(),
        }
    }
}

impl Email {
    pub fn last_message(&self) -> &EmailMessage {
        self.messages
            .last()
            .unwrap_or_else(|| self.messages.first().expect("email thread has no messages"))
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let subject = self.subject.to_lowercase();
        let sender = self.sender.to_lowercase();
        let preview = self.preview.to_lowercase();
        if subject.contains(&q) || sender.contains(&q) || preview.contains(&q) {
            return true;
        }
        self.messages
            .iter()
            .any(|m| m.body.to_lowercase().contains(&q))
    }

    pub fn refresh_preview(&mut self) {
        if let Some(last) = self.messages.last() {
            self.preview = summarize(&last.body, 80);
        }
    }
}

pub struct SeededMailbox {
    pub emails: Vec<Email>,
    pub next_email_id: usize,
    pub next_message_id: usize,
}

pub fn seed_mailbox() -> SeededMailbox {
    let mut next_email_id = 1usize;
    let mut next_message_id = 1usize;
    let mut emails = Vec::new();

    let mut add_thread = |subject: &str,
                          sender: &str,
                          folders: &[Folder],
                          is_read: bool,
                          is_flagged: bool,
                          labels: &[&str],
                          bodies: &[&str]| {
        let id = next_email_id;
        next_email_id += 1;
        let mut messages = Vec::new();
        for (idx, body) in bodies.iter().enumerate() {
            let msg_id = next_message_id;
            next_message_id += 1;
            let sent_at = NaiveDate::from_ymd_opt(2025, 1, 10 + idx as u32)
                .unwrap()
                .and_time(NaiveTime::from_hms_opt(9 + idx as u32, 15, 0).unwrap());
            messages.push(EmailMessage {
                id: msg_id,
                from: sender.to_string(),
                to: vec!["me@fission.rs".into()],
                cc: Vec::new(),
                body: body.to_string(),
                sent_at,
            });
        }
        let mut folders_set = HashSet::new();
        for f in folders {
            folders_set.insert(f.clone());
        }
        let mut email = Email {
            id,
            subject: subject.to_string(),
            sender: sender.to_string(),
            preview: String::new(),
            folders: folders_set,
            is_read,
            is_flagged,
            labels: labels.iter().map(|s| (*s).to_string()).collect(),
            messages,
        };
        email.refresh_preview();
        emails.push(email);
    };

    add_thread(
        "Quarterly planning sync",
        "Dana Wu",
        &[Folder::Inbox],
        false,
        true,
        &["Work", "Planning"],
        &[
            "Hey team — can we align on the Q1 goals? I pulled the draft OKRs into the doc.",
            "Following up with an updated deck. Please leave comments by Friday.",
        ],
    );
    add_thread(
        "Design review: Inbox refresh",
        "Alex Rivera",
        &[Folder::Inbox, Folder::Starred],
        true,
        true,
        &["Design"],
        &[
            "Sharing the latest design pass. The header area needs more breathing room.",
            "Thanks! I like the new hierarchy. Let's ship it next sprint.",
        ],
    );
    add_thread(
        "Receipt — Fission Pro renewal",
        "Billing",
        &[Folder::Inbox],
        true,
        false,
        &["Receipts"],
        &["Your subscription renewed on Jan 12. Total: $249.00. Invoice attached."],
    );
    add_thread(
        "Draft: Partnership proposal",
        "You",
        &[Folder::Drafts],
        true,
        false,
        &["Drafts"],
        &["Hi Jordan,\n\nI wanted to follow up on the partnership outline we discussed..."],
    );
    add_thread(
        "Meeting follow-up",
        "You",
        &[Folder::Sent],
        true,
        false,
        &["Sent"],
        &["Thanks for the time today. Here are the notes and the next steps we agreed on."],
    );
    add_thread(
        "Travel details: NYC",
        "Ops",
        &[Folder::Inbox],
        false,
        false,
        &["Travel"],
        &[
            "Attached are your flight details and hotel confirmation. Reach out if anything changes.",
        ],
    );
    add_thread(
        "Weekly product update",
        "Product Team",
        &[Folder::Inbox],
        true,
        false,
        &["Updates"],
        &["Here is your weekly product roundup. Highlights include new themes and analytics."],
    );
    add_thread(
        "Security review notes",
        "Security",
        &[Folder::Inbox],
        false,
        false,
        &["Security"],
        &[
            "Attached are the review notes from this week's dependency audit. Two medium findings need follow-up.",
        ],
    );
    add_thread(
        "Customer interview highlights",
        "Research",
        &[Folder::Inbox],
        true,
        false,
        &["Research"],
        &[
            "Three customers asked for faster search, better bulk triage, and clearer notification controls.",
        ],
    );
    add_thread(
        "Hiring loop feedback",
        "People Ops",
        &[Folder::Inbox],
        false,
        false,
        &["Hiring"],
        &[
            "The panel submitted final feedback for the design systems candidate. Hiring committee is tomorrow morning.",
        ],
    );

    SeededMailbox {
        emails,
        next_email_id,
        next_message_id,
    }
}

fn summarize(body: &str, limit: usize) -> String {
    let mut out = String::new();
    for ch in body.chars() {
        if ch.is_ascii_control() {
            out.push(' ');
        } else {
            out.push(ch);
        }
        if out.len() >= limit {
            break;
        }
    }
    if body.len() > out.len() {
        out.push('…');
    }
    out.trim().to_string()
}
