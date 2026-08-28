use imap_next::{
    imap_types::{
        auth::AuthMechanism,
        command::{Command, CommandBody},
        core::{Tag, Vec1},
        fetch::{Macro, MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName},
        flag::{Flag, FlagFetch, FlagPerm, StoreResponse, StoreType},
        mailbox::{ListMailbox, Mailbox},
        response::{Capability, Code, Data, Status},
        search::SearchKey,
        sequence::{SeqOrUid, Sequence, SequenceSet},
        status::{StatusDataItem, StatusDataItemName},
    },
    server::{ResponseHandle, Server},
};
use mailcrab::{MailMessage, MessageId};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

use crate::AppState;

use super::fetch::fetch_items;

/// Assigns a stable, strictly increasing IMAP UID to every message the IMAP
/// server has seen; shared between all IMAP connections
pub(super) struct UidRegistry {
    uid_validity: NonZeroU32,
    next_uid: u32,
    uids: HashMap<MessageId, NonZeroU32>,
}

impl UidRegistry {
    pub(super) fn new() -> Self {
        // messages only live in memory, a UIDVALIDITY based on the startup
        // time makes clients discard cached state from a previous run
        let uid_validity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .ok()
            .and_then(NonZeroU32::new)
            .unwrap_or(NonZeroU32::MIN);

        Self {
            uid_validity,
            next_uid: 1,
            uids: HashMap::new(),
        }
    }
}

/// The currently selected mailbox (always INBOX): an ordered view on the
/// message storage, where the sequence number of a message is its index + 1
struct SelectedMailbox {
    read_only: bool,
    /// messages ordered by UID
    messages: Vec<(NonZeroU32, MessageId)>,
    /// messages marked \Deleted in this session
    deleted: HashSet<MessageId>,
}

pub(super) enum CommandOutcome {
    Continue,
    Logout(ResponseHandle),
}

pub(super) struct Session {
    state: Arc<AppState>,
    uids: Arc<Mutex<UidRegistry>>,
    authenticated: bool,
    /// tag of an AUTHENTICATE command awaiting its continuation data
    pub(super) pending_auth: Option<Tag<'static>>,
    selected: Option<SelectedMailbox>,
}

impl Session {
    pub(super) fn new(state: Arc<AppState>, uids: Arc<Mutex<UidRegistry>>) -> Self {
        Self {
            state,
            uids,
            authenticated: false,
            pending_auth: None,
            selected: None,
        }
    }

    /// conclude an AUTHENTICATE exchange, any credentials are accepted
    pub(super) fn authenticate(&mut self, server: &mut Server, tag: Tag<'static>) {
        self.authenticated = true;
        let status = Status::ok(Some(tag), None, "authenticated").expect("static status");
        let _ = server.authenticate_finish(status);
    }

    pub(super) fn handle_command(
        &mut self,
        server: &mut Server,
        command: Command<'static>,
    ) -> CommandOutcome {
        let Command { tag, body } = command;

        match body {
            CommandBody::Capability => {
                server.enqueue_data(Data::Capability(capabilities()));
                ok(server, tag, "CAPABILITY completed");
            }
            CommandBody::Noop | CommandBody::Check => {
                self.notify_changes(server);
                ok(server, tag, "completed");
            }
            CommandBody::Logout => {
                let bye = Status::bye(None, "logging out").expect("static status");
                server.enqueue_status(bye);
                let status =
                    Status::ok(Some(tag), None, "LOGOUT completed").expect("static status");
                let handle = server.enqueue_status(status);

                return CommandOutcome::Logout(handle);
            }
            CommandBody::Login { .. } => {
                // this is a development tool: any credentials are accepted
                self.authenticated = true;
                ok(server, tag, "LOGIN completed");
            }
            CommandBody::Id { .. } => {
                server.enqueue_data(Data::Id { parameters: None });
                ok(server, tag, "ID completed");
            }
            body if !self.authenticated => {
                no(server, tag, "please login first");

                let _ = body;
            }
            CommandBody::Select { mailbox, .. } => self.select(server, tag, mailbox, false),
            CommandBody::Examine { mailbox, .. } => self.select(server, tag, mailbox, true),
            CommandBody::List {
                mailbox_wildcard, ..
            } => {
                self.list(server, &mailbox_wildcard, false);
                ok(server, tag, "LIST completed");
            }
            CommandBody::Lsub {
                mailbox_wildcard, ..
            } => {
                self.list(server, &mailbox_wildcard, true);
                ok(server, tag, "LSUB completed");
            }
            CommandBody::Status {
                mailbox,
                item_names,
            } => self.status(server, tag, mailbox, item_names),
            CommandBody::Subscribe { .. } | CommandBody::Unsubscribe { .. } => {
                ok(server, tag, "completed");
            }
            CommandBody::Close => {
                if self.selected.is_some() {
                    self.expunge(server, None, false);
                    self.selected = None;
                    ok(server, tag, "CLOSE completed");
                } else {
                    no(server, tag, "no mailbox selected");
                }
            }
            CommandBody::Unselect => {
                self.selected = None;
                ok(server, tag, "UNSELECT completed");
            }
            CommandBody::Expunge => {
                if self.selected.is_some() {
                    self.expunge(server, None, true);
                    ok(server, tag, "EXPUNGE completed");
                } else {
                    no(server, tag, "no mailbox selected");
                }
            }
            CommandBody::ExpungeUid { sequence_set } => {
                if self.selected.is_some() {
                    self.expunge(server, Some(&sequence_set), true);
                    ok(server, tag, "EXPUNGE completed");
                } else {
                    no(server, tag, "no mailbox selected");
                }
            }
            CommandBody::Fetch {
                sequence_set,
                macro_or_item_names,
                uid,
                ..
            } => self.fetch(server, tag, sequence_set, macro_or_item_names, uid),
            CommandBody::Store {
                sequence_set,
                kind,
                response,
                flags,
                uid,
                ..
            } => self.store(server, tag, sequence_set, kind, response, flags, uid),
            CommandBody::Search { criteria, uid, .. } => self.search(server, tag, criteria, uid),
            CommandBody::Copy { .. } | CommandBody::Move { .. } => {
                no(server, tag, "MailCrab has a single INBOX");
            }
            CommandBody::Create { .. }
            | CommandBody::Delete { .. }
            | CommandBody::Rename { .. }
            | CommandBody::Append { .. } => {
                no(server, tag, "MailCrab IMAP access is read-only");
            }
            _ => {
                no(server, tag, "command not supported");
            }
        }

        CommandOutcome::Continue
    }

    /// build an up-to-date view on the message storage: assign UIDs to new
    /// messages (in order of arrival) and drop removed messages
    fn snapshot(&self) -> Vec<(NonZeroU32, MessageId)> {
        let Ok(storage) = self.state.storage.read() else {
            return Vec::new();
        };

        let mut present = storage
            .values()
            .map(|message| (message.time, message.id))
            .collect::<Vec<_>>();
        present.sort_unstable();

        let Ok(mut registry) = self.uids.lock() else {
            return Vec::new();
        };
        registry.uids.retain(|id, _| storage.contains_key(id));

        let mut messages = Vec::with_capacity(present.len());
        for (_, id) in present {
            let uid = match registry.uids.get(&id) {
                Some(uid) => *uid,
                None => {
                    let uid = NonZeroU32::new(registry.next_uid).expect("UIDs start at 1");
                    registry.next_uid += 1;
                    registry.uids.insert(id, uid);
                    uid
                }
            };
            messages.push((uid, id));
        }

        messages.sort_unstable_by_key(|(uid, _)| *uid);

        messages
    }

    fn uid_validity(&self) -> NonZeroU32 {
        self.uids
            .lock()
            .map(|registry| registry.uid_validity)
            .unwrap_or(NonZeroU32::MIN)
    }

    fn next_uid(&self) -> NonZeroU32 {
        self.uids
            .lock()
            .map(|registry| NonZeroU32::new(registry.next_uid).unwrap_or(NonZeroU32::MIN))
            .unwrap_or(NonZeroU32::MIN)
    }

    /// announce mailbox changes (new and removed messages) on the selected
    /// mailbox, as requested by NOOP and CHECK
    fn notify_changes(&mut self, server: &mut Server) {
        if self.selected.is_none() {
            return;
        }

        let messages = self.snapshot();
        let Some(selected) = self.selected.as_mut() else {
            return;
        };

        let current = messages.iter().map(|(_, id)| *id).collect::<HashSet<_>>();

        // report removed messages, highest sequence number first
        for (index, (_, id)) in selected.messages.iter().enumerate().rev() {
            if !current.contains(id)
                && let Some(seq) = NonZeroU32::new(index as u32 + 1)
            {
                server.enqueue_data(Data::Expunge(seq));
            }
        }

        let known = selected
            .messages
            .iter()
            .map(|(_, id)| *id)
            .collect::<HashSet<_>>();
        let has_new = messages.iter().any(|(_, id)| !known.contains(id));

        selected.messages = messages;
        selected.deleted.retain(|id| current.contains(id));

        if has_new {
            server.enqueue_data(Data::Exists(selected.messages.len() as u32));
            server.enqueue_data(Data::Recent(0));
        }
    }

    fn select(
        &mut self,
        server: &mut Server,
        tag: Tag<'static>,
        mailbox: Mailbox<'static>,
        read_only: bool,
    ) {
        if mailbox != Mailbox::Inbox {
            self.selected = None;
            no(server, tag, "MailCrab has a single INBOX");
            return;
        }

        let messages = self.snapshot();
        let count = messages.len() as u32;

        self.selected = Some(SelectedMailbox {
            read_only,
            messages,
            deleted: HashSet::new(),
        });

        server.enqueue_data(Data::Flags(vec![Flag::Seen, Flag::Deleted]));
        server.enqueue_data(Data::Exists(count));
        server.enqueue_data(Data::Recent(0));
        server.enqueue_status(
            Status::ok(
                None,
                Some(Code::PermanentFlags(vec![
                    FlagPerm::Flag(Flag::Seen),
                    FlagPerm::Flag(Flag::Deleted),
                ])),
                "flags",
            )
            .expect("static status"),
        );
        server.enqueue_status(
            Status::ok(
                None,
                Some(Code::UidValidity(self.uid_validity())),
                "UIDs valid",
            )
            .expect("static status"),
        );
        server.enqueue_status(
            Status::ok(
                None,
                Some(Code::UidNext(self.next_uid())),
                "predicted next UID",
            )
            .expect("static status"),
        );

        let (code, text) = if read_only {
            (Code::ReadOnly, "EXAMINE completed")
        } else {
            (Code::ReadWrite, "SELECT completed")
        };
        server.enqueue_status(Status::ok(Some(tag), Some(code), text).expect("static status"));
    }

    fn list(&self, server: &mut Server, wildcard: &ListMailbox<'static>, lsub: bool) {
        let pattern = match wildcard {
            ListMailbox::Token(token) => String::from_utf8_lossy(token.as_ref()).to_string(),
            ListMailbox::String(string) => String::from_utf8_lossy(string.as_ref()).to_string(),
        };

        // an empty pattern requests the hierarchy delimiter and root
        if pattern.is_empty() {
            if let Ok(mailbox) = Mailbox::try_from("") {
                server.enqueue_data(Data::List {
                    items: vec![],
                    delimiter: None,
                    mailbox,
                });
            }
            return;
        }

        let matches_inbox =
            pattern.contains('*') || pattern.contains('%') || pattern.eq_ignore_ascii_case("inbox");

        if matches_inbox {
            let data = if lsub {
                Data::Lsub {
                    items: vec![],
                    delimiter: None,
                    mailbox: Mailbox::Inbox,
                }
            } else {
                Data::List {
                    items: vec![],
                    delimiter: None,
                    mailbox: Mailbox::Inbox,
                }
            };
            server.enqueue_data(data);
        }
    }

    fn status(
        &mut self,
        server: &mut Server,
        tag: Tag<'static>,
        mailbox: Mailbox<'static>,
        item_names: Cow<'static, [StatusDataItemName]>,
    ) {
        if mailbox != Mailbox::Inbox {
            no(server, tag, "MailCrab has a single INBOX");
            return;
        }

        let messages = self.snapshot();
        let unseen = {
            match self.state.storage.read() {
                Ok(storage) => storage.values().filter(|m| !m.is_opened()).count() as u32,
                Err(_) => 0,
            }
        };

        let items = item_names
            .iter()
            .filter_map(|name| match name {
                StatusDataItemName::Messages => {
                    Some(StatusDataItem::Messages(messages.len() as u32))
                }
                StatusDataItemName::Recent => Some(StatusDataItem::Recent(0)),
                StatusDataItemName::UidNext => Some(StatusDataItem::UidNext(self.next_uid())),
                StatusDataItemName::UidValidity => {
                    Some(StatusDataItem::UidValidity(self.uid_validity()))
                }
                StatusDataItemName::Unseen => Some(StatusDataItem::Unseen(unseen)),
                _ => None,
            })
            .collect::<Vec<_>>();

        server.enqueue_data(Data::Status {
            mailbox: Mailbox::Inbox,
            items: Cow::Owned(items),
        });
        ok(server, tag, "STATUS completed");
    }

    /// collect the (sequence number, UID, message id) of all messages in the
    /// selected mailbox matched by a sequence set of message numbers or UIDs
    fn matched(
        &self,
        sequence_set: &SequenceSet,
        uid_mode: bool,
    ) -> Vec<(NonZeroU32, NonZeroU32, MessageId)> {
        let Some(selected) = self.selected.as_ref() else {
            return Vec::new();
        };

        let count = selected.messages.len() as u32;
        let max_uid = selected
            .messages
            .last()
            .map(|(uid, _)| uid.get())
            .unwrap_or(0);

        selected
            .messages
            .iter()
            .enumerate()
            .filter_map(|(index, (uid, id))| {
                let seq = NonZeroU32::new(index as u32 + 1)?;
                let matches = if uid_mode {
                    sequence_set_contains(sequence_set, *uid, max_uid)
                } else {
                    sequence_set_contains(sequence_set, seq, count)
                };

                matches.then_some((seq, *uid, *id))
            })
            .collect()
    }

    fn fetch(
        &mut self,
        server: &mut Server,
        tag: Tag<'static>,
        sequence_set: SequenceSet,
        macro_or_item_names: MacroOrMessageDataItemNames<'static>,
        uid_mode: bool,
    ) {
        if self.selected.is_none() {
            no(server, tag, "no mailbox selected");
            return;
        }

        let names = expand_macro(macro_or_item_names);

        // BODY[...] without .PEEK and the RFC822 full/text fetches implicitly
        // mark a message as seen
        let marks_seen = names.iter().any(|name| {
            matches!(
                name,
                MessageDataItemName::BodyExt { peek: false, .. }
                    | MessageDataItemName::Rfc822
                    | MessageDataItemName::Rfc822Text
            )
        });

        for (seq, uid, id) in self.matched(&sequence_set, uid_mode) {
            let Some(mut message) = self
                .state
                .storage
                .read()
                .ok()
                .and_then(|storage| storage.get(&id).cloned())
            else {
                continue;
            };

            if marks_seen && !message.is_opened() {
                if let Ok(mut storage) = self.state.storage.write()
                    && let Some(stored) = storage.get_mut(&id)
                {
                    stored.open();
                }
                message.open();
            }

            let deleted = self
                .selected
                .as_ref()
                .is_some_and(|selected| selected.deleted.contains(&id));
            let items = fetch_items(&names, &message, uid, deleted, uid_mode);

            server.enqueue_data(Data::Fetch { seq, items });
        }

        ok(server, tag, "FETCH completed");
    }

    #[allow(clippy::too_many_arguments)]
    fn store(
        &mut self,
        server: &mut Server,
        tag: Tag<'static>,
        sequence_set: SequenceSet,
        kind: StoreType,
        response: StoreResponse,
        flags: Vec<Flag<'static>>,
        uid_mode: bool,
    ) {
        let read_only = match self.selected.as_ref() {
            Some(selected) => selected.read_only,
            None => {
                no(server, tag, "no mailbox selected");
                return;
            }
        };

        if read_only {
            no(server, tag, "mailbox is read-only");
            return;
        }

        let has_seen = flags.contains(&Flag::Seen);
        let has_deleted = flags.contains(&Flag::Deleted);

        for (seq, uid, id) in self.matched(&sequence_set, uid_mode) {
            let mut opened = false;

            if let Ok(mut storage) = self.state.storage.write()
                && let Some(message) = storage.get_mut(&id)
            {
                match kind {
                    StoreType::Add if has_seen => message.set_opened(true),
                    StoreType::Remove if has_seen => message.set_opened(false),
                    StoreType::Replace => message.set_opened(has_seen),
                    _ => {}
                }
                opened = message.is_opened();
            }

            let deleted = {
                let Some(selected) = self.selected.as_mut() else {
                    continue;
                };
                match kind {
                    StoreType::Add if has_deleted => {
                        selected.deleted.insert(id);
                    }
                    StoreType::Remove if has_deleted => {
                        selected.deleted.remove(&id);
                    }
                    StoreType::Replace => {
                        if has_deleted {
                            selected.deleted.insert(id);
                        } else {
                            selected.deleted.remove(&id);
                        }
                    }
                    _ => {}
                }
                selected.deleted.contains(&id)
            };

            if response == StoreResponse::Answer {
                let mut items = vec![MessageDataItem::Flags(flag_fetch(opened, deleted))];
                if uid_mode {
                    items.push(MessageDataItem::Uid(uid));
                }
                server.enqueue_data(Data::Fetch {
                    seq,
                    items: Vec1::unvalidated(items),
                });
            }
        }

        ok(server, tag, "STORE completed");
    }

    /// remove all messages marked \Deleted (optionally limited to a UID set)
    /// from the storage, and report them when `announce` is set
    fn expunge(&mut self, server: &mut Server, uids: Option<&SequenceSet>, announce: bool) {
        let Some(selected) = self.selected.as_mut() else {
            return;
        };

        let max_uid = selected
            .messages
            .last()
            .map(|(uid, _)| uid.get())
            .unwrap_or(0);

        // walk from the highest sequence number down, so earlier expunges do
        // not shift the sequence numbers still to be reported
        for index in (0..selected.messages.len()).rev() {
            let (uid, id) = selected.messages[index];

            if !selected.deleted.contains(&id) {
                continue;
            }

            if let Some(uids) = uids
                && !sequence_set_contains(uids, uid, max_uid)
            {
                continue;
            }

            if let Ok(mut storage) = self.state.storage.write() {
                storage.remove(&id);
                info!("message {} removed over IMAP", id);
            }

            selected.messages.remove(index);
            selected.deleted.remove(&id);

            if announce && let Some(seq) = NonZeroU32::new(index as u32 + 1) {
                server.enqueue_data(Data::Expunge(seq));
            }
        }
    }

    fn search(
        &mut self,
        server: &mut Server,
        tag: Tag<'static>,
        criteria: Vec1<SearchKey<'static>>,
        uid_mode: bool,
    ) {
        let Some(selected) = self.selected.as_ref() else {
            no(server, tag, "no mailbox selected");
            return;
        };

        let count = selected.messages.len() as u32;
        let max_uid = selected
            .messages
            .last()
            .map(|(uid, _)| uid.get())
            .unwrap_or(0);

        let mut hits = Vec::new();

        if let Ok(storage) = self.state.storage.read() {
            for (index, (uid, id)) in selected.messages.iter().enumerate() {
                let Some(message) = storage.get(id) else {
                    continue;
                };
                let Some(seq) = NonZeroU32::new(index as u32 + 1) else {
                    continue;
                };
                let context = SearchContext {
                    message,
                    uid: *uid,
                    seq,
                    count,
                    max_uid,
                    deleted: selected.deleted.contains(id),
                };

                if criteria
                    .as_ref()
                    .iter()
                    .all(|key| matches_search(key, &context))
                {
                    hits.push(if uid_mode { *uid } else { seq });
                }
            }
        }

        server.enqueue_data(Data::Search(hits));
        ok(server, tag, "SEARCH completed");
    }
}

fn capabilities() -> Vec1<Capability<'static>> {
    Vec1::unvalidated(vec![
        Capability::Imap4Rev1,
        Capability::Auth(AuthMechanism::Plain),
    ])
}

fn ok(server: &mut Server, tag: Tag<'static>, text: &'static str) {
    server.enqueue_status(Status::ok(Some(tag), None, text).expect("static status"));
}

fn no(server: &mut Server, tag: Tag<'static>, text: &'static str) {
    server.enqueue_status(Status::no(Some(tag), None, text).expect("static status"));
}

pub(super) fn flag_fetch(opened: bool, deleted: bool) -> Vec<FlagFetch<'static>> {
    let mut flags = Vec::new();
    if opened {
        flags.push(FlagFetch::Flag(Flag::Seen));
    }
    if deleted {
        flags.push(FlagFetch::Flag(Flag::Deleted));
    }
    flags
}

fn expand_macro(names: MacroOrMessageDataItemNames<'static>) -> Vec<MessageDataItemName<'static>> {
    match names {
        MacroOrMessageDataItemNames::MessageDataItemNames(names) => names,
        MacroOrMessageDataItemNames::Macro(Macro::Fast) => vec![
            MessageDataItemName::Flags,
            MessageDataItemName::InternalDate,
            MessageDataItemName::Rfc822Size,
        ],
        MacroOrMessageDataItemNames::Macro(Macro::All) => vec![
            MessageDataItemName::Flags,
            MessageDataItemName::InternalDate,
            MessageDataItemName::Rfc822Size,
            MessageDataItemName::Envelope,
        ],
        MacroOrMessageDataItemNames::Macro(Macro::Full) => vec![
            MessageDataItemName::Flags,
            MessageDataItemName::InternalDate,
            MessageDataItemName::Rfc822Size,
            MessageDataItemName::Envelope,
            MessageDataItemName::Body,
        ],
        // `Macro` is non-exhaustive: treat unknown macros as ALL
        MacroOrMessageDataItemNames::Macro(_) => vec![
            MessageDataItemName::Flags,
            MessageDataItemName::InternalDate,
            MessageDataItemName::Rfc822Size,
            MessageDataItemName::Envelope,
        ],
    }
}

/// whether a sequence set contains the given sequence number or UID; ranges
/// are checked without expanding them, `*` means the largest value in the
/// mailbox
fn sequence_set_contains(sequence_set: &SequenceSet, value: NonZeroU32, largest: u32) -> bool {
    let expand = |seq_or_uid: &SeqOrUid| match seq_or_uid {
        SeqOrUid::Value(value) => value.get(),
        SeqOrUid::Asterisk => largest,
    };

    sequence_set
        .0
        .as_ref()
        .iter()
        .any(|sequence| match sequence {
            Sequence::Single(single) => expand(single) == value.get(),
            Sequence::Range(from, to) => {
                let (from, to) = (expand(from), expand(to));
                let (low, high) = if from <= to { (from, to) } else { (to, from) };

                low <= value.get() && value.get() <= high
            }
        })
}

struct SearchContext<'a> {
    message: &'a MailMessage,
    uid: NonZeroU32,
    seq: NonZeroU32,
    count: u32,
    max_uid: u32,
    deleted: bool,
}

/// evaluate the common IMAP SEARCH criteria; criteria that cannot apply to
/// the MailCrab message store (drafts, answered or flagged messages, dates,
/// header and text matching) match everything
fn matches_search(key: &SearchKey, context: &SearchContext) -> bool {
    match key {
        SearchKey::All => true,
        SearchKey::And(keys) => keys.as_ref().iter().all(|key| matches_search(key, context)),
        SearchKey::Not(key) => !matches_search(key, context),
        SearchKey::Or(a, b) => matches_search(a, context) || matches_search(b, context),
        SearchKey::SequenceSet(set) => sequence_set_contains(set, context.seq, context.count),
        SearchKey::Uid(set) => sequence_set_contains(set, context.uid, context.max_uid),
        SearchKey::Seen => context.message.is_opened(),
        SearchKey::Unseen => !context.message.is_opened(),
        SearchKey::Deleted => context.deleted,
        SearchKey::Undeleted => !context.deleted,
        SearchKey::Answered | SearchKey::Draft | SearchKey::Flagged => false,
        SearchKey::Unanswered | SearchKey::Undraft | SearchKey::Unflagged => true,
        SearchKey::New | SearchKey::Recent => false,
        SearchKey::Old => true,
        SearchKey::Larger(size) => {
            context.message.raw_bytes().map(|b| b.len()).unwrap_or(0) > *size as usize
        }
        SearchKey::Smaller(size) => {
            context.message.raw_bytes().map(|b| b.len()).unwrap_or(0) < *size as usize
        }
        _ => true,
    }
}
