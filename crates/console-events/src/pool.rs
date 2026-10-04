//! Who is listening to what, and what the last word on each event group was.
//!
//! All of it, and nothing else. There is no socket in this file and no thread:
//! the pool is arithmetic over two maps, so what happens when two programs
//! want the same event group and one of them goes away can be asked on a laptop
//! instead of by opening twenty-five subscriptions and counting.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;
use console_program_contract::{Change, EventGroup};

pub type Who = u64;

#[derive(Debug, Default)]
pub struct Pool {
    subscribers: BTreeMap<Who, BTreeSet<EventGroup>>,
    last: BTreeMap<EventGroup, String>,
    next: Who,
}

impl Pool {
    pub fn joined(&mut self) -> Result<Who, Never> {
        let who = self.next;

        self.next = who.saturating_add(1);
        self.subscribers.insert(who, BTreeSet::new());

        Ok(who)
    }

    pub fn remove(&mut self, who: Who) -> Result<(), Never> {
        self.subscribers.remove(&who);

        Ok(())
    }

    pub fn subscribe(&mut self, who: Who, event_group: EventGroup) -> Result<Option<Change>, Never> {
        let last = self.last.get(&event_group).cloned();

        let event_groups = match self.subscribers.get_mut(&who) {
            Some(event_groups) => event_groups,
            None => return Ok(None),
        };

        let _ = event_groups.insert(event_group.clone());

        Ok(last.map(|text| Change { event_group, text }))
    }

    pub fn unsubscribe(&mut self, who: Who, event_group: &EventGroup) -> Result<(), Never> {
        match self.subscribers.get_mut(&who) {
            Some(event_groups) => {
                let _ = event_groups.remove(event_group);
            }
            None => {},
        }

        Ok(())
    }

    pub fn publish(&mut self, change: &Change) -> Result<Vec<Who>, Never> {
        self.last.insert(change.event_group.clone(), change.text.clone());

        Ok(self
            .subscribers
            .iter()
            .filter(|(_, event_groups)| event_groups.contains(&change.event_group))
            .map(|(who, _)| *who)
            .collect())
    }

    pub fn event_groups(&self) -> Result<BTreeSet<EventGroup>, Never> {
        Ok(self.subscribers.values().flatten().cloned().collect())
    }

    pub fn last(&self, event_group: &EventGroup) -> Result<Option<&str>, Never> {
        Ok(self.last.get(event_group).map(String::as_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_reaches_everyone_who_asked_for_that_event_group_and_no_one_else() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();
        let Ok(two) = pool.joined();
        let Ok(three) = pool.joined();

        let Ok(first) = pool.subscribe(one, EventGroup::Sound);
        let Ok(second) = pool.subscribe(two, EventGroup::Sound);
        let Ok(third) = pool.subscribe(three, EventGroup::Network);

        assert_eq!(first, None);
        assert_eq!(second, None);
        assert_eq!(third, None);

        let Ok(answered) = pool.publish(&Change { event_group: EventGroup::Sound, text: "sink 1".to_string() });

        assert_eq!(answered, vec![one, two]);
    }

    #[test]
    fn whoever_arrives_late_is_told_the_last_word_first() {
        let mut pool = Pool::default();
        let Ok(early) = pool.joined();

        let Ok(answered) = pool.subscribe(early, EventGroup::Sound);

        assert_eq!(answered, None);

        let Ok(_) = pool.publish(&Change { event_group: EventGroup::Sound, text: "sink 1 at 40%".to_string() });
        let Ok(late) = pool.joined();

        let Ok(answered) = pool.subscribe(late, EventGroup::Sound);

        assert_eq!(answered, Some(Change { event_group: EventGroup::Sound, text: "sink 1 at 40%".to_string() }));
    }

    #[test]
    fn the_last_word_is_the_last_one_and_not_all_of_them() {
        let mut pool = Pool::default();
        let Ok(_) = pool.publish(&Change { event_group: EventGroup::Sound, text: "40%".to_string() });
        let Ok(_) = pool.publish(&Change { event_group: EventGroup::Sound, text: "45%".to_string() });
        let Ok(late) = pool.joined();

        let Ok(answered) = pool.subscribe(late, EventGroup::Sound);

        assert_eq!(answered, Some(Change { event_group: EventGroup::Sound, text: "45%".to_string() }));
    }

    #[test]
    fn a_program_that_has_gone_is_told_nothing_and_holds_no_source_open() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();
        let Ok(two) = pool.joined();

        let Ok(_) = pool.subscribe(one, EventGroup::Compositor);
        let Ok(_) = pool.subscribe(two, EventGroup::Compositor);
        let Ok(()) = pool.remove(one);

        let Ok(answered) = pool.publish(&Change { event_group: EventGroup::Compositor, text: "openwindow".to_string() });

        assert_eq!(answered, vec![two]);
        let Ok(answered) = pool.event_groups();

        assert_eq!(answered, BTreeSet::from([EventGroup::Compositor]));

        let Ok(()) = pool.remove(two);

        let Ok(answered) = pool.publish(&Change { event_group: EventGroup::Compositor, text: "closewindow".to_string() });

        assert!(answered.is_empty());
        let Ok(answered) = pool.event_groups();

        assert!(answered.is_empty());
    }

    #[test]
    fn a_program_that_stopped_caring_about_one_event_group_still_hears_the_others() {
        let mut pool = Pool::default();
        let Ok(who) = pool.joined();

        let Ok(_) = pool.subscribe(who, EventGroup::Sound);
        let Ok(_) = pool.subscribe(who, EventGroup::Network);
        let Ok(()) = pool.unsubscribe(who, &EventGroup::Sound);

        let Ok(answered) = pool.publish(&Change { event_group: EventGroup::Sound, text: "40%".to_string() });

        assert!(answered.is_empty());
        let Ok(answered) = pool.publish(&Change { event_group: EventGroup::Network, text: "up".to_string() });

        assert_eq!(answered, vec![who]);
    }

    #[test]
    fn no_one_is_ever_given_a_name_someone_else_had() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();

        let Ok(()) = pool.remove(one);

        let Ok(two) = pool.joined();

        assert_ne!(one, two);
    }

    #[test]
    fn a_word_on_a_event_group_no_one_wants_is_still_remembered_for_whoever_comes() {
        let mut pool = Pool::default();

        let Ok(answered) = pool.publish(&Change { event_group: EventGroup::Player, text: "paused".to_string() });

        assert!(answered.is_empty());
        let Ok(answered) = pool.last(&EventGroup::Player);

        assert_eq!(answered, Some("paused"));
    }
}
