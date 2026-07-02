//! Session matchmaking logic and queue handlers.

use super::{AccountId, ROSTER_SIZE, Region, SKILL_WINDOW, Session, SessionId};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueTicket {
    pub account: AccountId,
    pub rating: u16,
    pub region: Region,
    pub build: u32,
    pub queued_at: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueError {
    DuplicateAccount,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Matchmaker {
    pub queue: Vec<QueueTicket>,
    pub next_session: u32,
    pub formed_sessions: u32,
    pub last_event: String,
}

impl Default for Matchmaker {
    fn default() -> Self {
        Self::new()
    }
}

impl Matchmaker {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            next_session: 1,
            formed_sessions: 0,
            last_event: "Queue open.".to_string(),
        }
    }

    pub fn enqueue(&mut self, ticket: QueueTicket) -> Result<(), QueueError> {
        if self
            .queue
            .iter()
            .any(|queued| queued.account == ticket.account)
        {
            return Err(QueueError::DuplicateAccount);
        }
        self.queue.push(ticket);
        self.sort_queue();
        self.last_event = format!(
            "{} queued in {} at rating {}.",
            ticket.account.label(),
            ticket.region.label(),
            ticket.rating
        );
        Ok(())
    }

    pub fn form_next(&mut self) -> Option<Session> {
        self.sort_queue();
        let selected = find_compatible_group(&self.queue)?;
        let selected_accounts: BTreeSet<AccountId> =
            selected.iter().map(|ticket| ticket.account).collect();
        self.queue
            .retain(|ticket| !selected_accounts.contains(&ticket.account));

        let session_id = SessionId(self.next_session);
        self.next_session += 1;
        self.formed_sessions += 1;
        self.last_event = format!(
            "{} formed from {} compatible tickets.",
            session_id.label(),
            selected.len()
        );
        Some(Session::formed(session_id, selected))
    }

    fn sort_queue(&mut self) {
        self.queue
            .sort_by_key(|ticket| (ticket.queued_at, ticket.account));
    }
}

fn find_compatible_group(queue: &[QueueTicket]) -> Option<Vec<QueueTicket>> {
    for (anchor_index, anchor) in queue.iter().enumerate() {
        let mut group = vec![*anchor];
        let mut min_rating = anchor.rating;
        let mut max_rating = anchor.rating;
        for candidate in queue.iter().skip(anchor_index + 1) {
            if candidate.region != anchor.region || candidate.build != anchor.build {
                continue;
            }
            let next_min = min_rating.min(candidate.rating);
            let next_max = max_rating.max(candidate.rating);
            if next_max - next_min > SKILL_WINDOW {
                continue;
            }
            group.push(*candidate);
            min_rating = next_min;
            max_rating = next_max;
            if group.len() == ROSTER_SIZE {
                return Some(group);
            }
        }
    }
    None
}
