//! The list's structure in T3's default sidebar order: pinned, working, active, then the
//! snoozed and settled shelves (collapsed by default). Each thread's section resolves like T3's
//! `resolveSidebarSection`: snoozed → settled → pinned → working → active. Order inside a section
//! is `Core::threads()` order, which is stable. Pure data, rebuilt only when threads, channels, the
//! search query or a collapse toggle change.

use std::collections::{HashMap, HashSet};

use t3_panel_core::{ChannelView, ThreadView};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Pinned,
    Working,
    Active,
    Snoozed,
    Settled,
}

impl Section {
    pub const ORDER: [Section; 5] =
        [Section::Pinned, Section::Working, Section::Active, Section::Snoozed, Section::Settled];

    pub fn label(self) -> &'static str {
        match self {
            Section::Pinned => "Sabitlenmiş",
            Section::Working => "Çalışıyor",
            Section::Active => "Aktif",
            Section::Snoozed => "Ertelendi",
            Section::Settled => "Bitti",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Section::Pinned => "pinned",
            Section::Working => "working",
            Section::Active => "active",
            Section::Snoozed => "snoozed",
            Section::Settled => "settled",
        }
    }

    /// T3 starts its shelves collapsed.
    pub fn collapsed_by_default(self) -> bool {
        matches!(self, Section::Snoozed | Section::Settled)
    }

    pub fn of(t: &ThreadView) -> Section {
        if t.snoozed {
            Section::Snoozed
        } else if t.settled {
            Section::Settled
        } else if t.pinned {
            Section::Pinned
        } else if t.status == "running" {
            Section::Working
        } else {
            Section::Active
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Header {
        section: Section,
        count: usize,
        collapsed: bool,
        /// A collapsed section hides a thread that waits for the user or is listened to.
        signal: bool,
    },
    Row {
        /// Index into the thread snapshot.
        thread: usize,
        /// Second line: the utterance being spoken, or what the thread waits for.
        state_line: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListModel {
    pub items: Vec<Item>,
    /// Thread ids of visible rows, in visual order (keyboard navigation).
    pub row_ids: Vec<String>,
    /// Threads matching the query (visible or inside a collapsed section).
    pub matched: usize,
}

pub struct Filter<'a> {
    /// Lowercased search query (empty = no filter).
    pub query: &'a str,
    /// Sections whose collapsed state differs from `collapsed_by_default`.
    pub toggled: &'a HashSet<Section>,
}

impl Filter<'_> {
    pub fn collapsed(&self, s: Section) -> bool {
        Filter::collapsed_in(self.toggled, s)
    }

    pub fn collapsed_in(toggled: &HashSet<Section>, s: Section) -> bool {
        s.collapsed_by_default() != toggled.contains(&s)
    }
}

pub fn matches(t: &ThreadView, query: &str) -> bool {
    query.is_empty() || t.title.to_lowercase().contains(query) || t.project.to_lowercase().contains(query)
}

pub fn has_state_line(ch: Option<&ChannelView>) -> bool {
    ch.is_some_and(|c| c.waiting_for_user || (c.listening && c.now_text.is_some()))
}

pub fn build(threads: &[ThreadView], channels: &HashMap<String, ChannelView>, f: &Filter) -> ListModel {
    let searching = !f.query.is_empty();
    let mut model = ListModel::default();
    let mut sections: [Vec<usize>; 5] = Default::default();
    for (ix, t) in threads.iter().enumerate() {
        if matches(t, f.query) {
            model.matched += 1;
            sections[Section::ORDER.iter().position(|s| *s == Section::of(t)).unwrap_or(2)].push(ix);
        }
    }
    for (section, rows) in Section::ORDER.into_iter().zip(&sections) {
        if rows.is_empty() {
            continue;
        }
        let collapsed = !searching && f.collapsed(section);
        let signal = collapsed
            && rows.iter().any(|&ix| channels.get(&threads[ix].id).is_some_and(|c| c.waiting_for_user || c.listening));
        model.items.push(Item::Header { section, count: rows.len(), collapsed, signal });
        if collapsed {
            continue;
        }
        for &ix in rows {
            let t = &threads[ix];
            model.items.push(Item::Row { thread: ix, state_line: has_state_line(channels.get(&t.id)) });
            model.row_ids.push(t.id.clone());
        }
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(id: &str, status: &str, pinned: bool, settled: bool, snoozed: bool) -> ThreadView {
        ThreadView {
            id: id.into(),
            project: "Kazan".into(),
            project_id: "p".into(),
            project_icon: None,
            project_pinned: false,
            title: format!("title {id}"),
            status: status.into(),
            settled,
            snoozed,
            pinned,
            created_at: String::new(),
            updated_at: String::new(),
            link: String::new(),
            provider: "omp".into(),
            model: String::new(),
            running_since: None,
        }
    }

    fn ids(model: &ListModel) -> Vec<&str> {
        model.row_ids.iter().map(String::as_str).collect()
    }

    fn headers(model: &ListModel) -> Vec<(Section, usize, bool)> {
        model
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Header { section, count, collapsed, .. } => Some((*section, *count, *collapsed)),
                Item::Row { .. } => None,
            })
            .collect()
    }

    #[test]
    fn sections_follow_t3_resolution_and_order() {
        let threads = vec![
            thread("a", "idle", false, false, false),
            thread("b", "running", false, false, false),
            thread("c", "running", true, false, false),
            thread("d", "completed", true, true, false),
            thread("e", "idle", false, false, true),
            thread("f", "completed", false, false, false),
        ];
        let toggled = HashSet::new();
        let model = build(&threads, &HashMap::new(), &Filter { query: "", toggled: &toggled });
        // Settled wins over pinned; pinned over working; shelves start collapsed.
        assert_eq!(ids(&model), ["c", "b", "a", "f"]);
        assert_eq!(
            headers(&model),
            [
                (Section::Pinned, 1, false),
                (Section::Working, 1, false),
                (Section::Active, 2, false),
                (Section::Snoozed, 1, true),
                (Section::Settled, 1, true),
            ]
        );
        assert_eq!(model.matched, 6);
    }

    #[test]
    fn collapse_signal_and_search() {
        let threads = vec![thread("a", "idle", false, false, false), thread("b", "idle", false, true, false)];
        let toggled: HashSet<Section> = [Section::Active].into();
        let mut channels = HashMap::new();
        channels.insert("a".to_owned(), ChannelView { waiting_for_user: true, ..Default::default() });
        let model = build(&threads, &channels, &Filter { query: "", toggled: &toggled });
        assert!(ids(&model).is_empty());
        assert!(matches!(&model.items[0], Item::Header { collapsed: true, signal: true, count: 1, .. }));

        // Searching opens every section.
        let model = build(&threads, &channels, &Filter { query: "title", toggled: &toggled });
        assert_eq!(ids(&model), ["a", "b"]);
        assert!(matches!(&model.items[1], Item::Row { state_line: true, .. }));

        let model = build(&threads, &channels, &Filter { query: "zzz", toggled: &toggled });
        assert!(model.items.is_empty());
        assert_eq!(model.matched, 0);
    }
}
