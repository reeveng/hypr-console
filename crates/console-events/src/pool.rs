//! Who is listening to what, and what the last word on each topic was.
//!
//! All of it, and nothing else. There is no socket in this file and no thread:
//! the pool is arithmetic over two maps, so what happens when two programs
//! want the same topic and one of them goes away can be asked on a laptop
//! instead of by opening twenty-five subscriptions and counting.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;
use console_program_contract::{Change, Topic};

pub type Who = u64;

#[derive(Debug, Default)]
pub struct Pool {
    subscribers: BTreeMap<Who, BTreeSet<Topic>>,
    last: BTreeMap<Topic, String>,
    next: Who,
}

impl Pool {
    pub fn joined(&mut self) -> Result<Who, Never> {
        let who = self.next;

        self.next = who.saturating_add(1);
        self.subscribers.insert(who, BTreeSet::new());

        Ok(who)
    }

    pub fn left(&mut self, who: Who) -> Result<(), Never> {
        self.subscribers.remove(&who);

        Ok(())
    }

    pub fn subscribe(&mut self, who: Who, topic: Topic) -> Result<Option<Change>, Never> {
        let last = self.last.get(&topic).cloned();

        let topics = match self.subscribers.get_mut(&who) {
            Some(topics) => topics,
            None => return Ok(None),
        };

        let _ = topics.insert(topic.clone());

        Ok(last.map(|text| Change { topic, text }))
    }

    pub fn unsubscribe(&mut self, who: Who, topic: &Topic) -> Result<(), Never> {
        match self.subscribers.get_mut(&who) {
            Some(topics) => {
                let _ = topics.remove(topic);
            }
            None => {},
        }

        Ok(())
    }

    pub fn publish(&mut self, change: &Change) -> Result<Vec<Who>, Never> {
        self.last.insert(change.topic.clone(), change.text.clone());

        Ok(self
            .subscribers
            .iter()
            .filter(|(_, topics)| topics.contains(&change.topic))
            .map(|(who, _)| *who)
            .collect())
    }

    pub fn wanted(&self) -> Result<BTreeSet<Topic>, Never> {
        Ok(self.subscribers.values().flatten().cloned().collect())
    }

    pub fn last(&self, topic: &Topic) -> Result<Option<&str>, Never> {
        Ok(self.last.get(topic).map(String::as_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(topic: Topic, text: &str) -> Change {
        Change { topic, text: text.to_string() }
    }

    #[test]
    fn a_word_reaches_everyone_who_asked_for_that_topic_and_no_one_else() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();
        let Ok(two) = pool.joined();
        let Ok(three) = pool.joined();

        let Ok(first) = pool.subscribe(one, Topic::Sound);
        let Ok(second) = pool.subscribe(two, Topic::Sound);
        let Ok(third) = pool.subscribe(three, Topic::Network);

        assert_eq!(first, None);
        assert_eq!(second, None);
        assert_eq!(third, None);

        let Ok(answered) = pool.publish(&change(Topic::Sound, "sink 1"));

        assert_eq!(answered, vec![one, two]);
    }

    #[test]
    fn whoever_arrives_late_is_told_the_last_word_first() {
        let mut pool = Pool::default();
        let Ok(early) = pool.joined();

        let Ok(answered) = pool.subscribe(early, Topic::Sound);

        assert_eq!(answered, None);

        let Ok(_) = pool.publish(&change(Topic::Sound, "sink 1 at 40%"));
        let Ok(late) = pool.joined();

        let Ok(answered) = pool.subscribe(late, Topic::Sound);

        assert_eq!(answered, Some(change(Topic::Sound, "sink 1 at 40%")));
    }

    #[test]
    fn the_last_word_is_the_last_one_and_not_all_of_them() {
        let mut pool = Pool::default();
        let Ok(_) = pool.publish(&change(Topic::Sound, "40%"));
        let Ok(_) = pool.publish(&change(Topic::Sound, "45%"));
        let Ok(late) = pool.joined();

        let Ok(answered) = pool.subscribe(late, Topic::Sound);

        assert_eq!(answered, Some(change(Topic::Sound, "45%")));
    }

    #[test]
    fn a_program_that_has_gone_is_told_nothing_and_holds_no_source_open() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();
        let Ok(two) = pool.joined();

        let Ok(_) = pool.subscribe(one, Topic::Compositor);
        let Ok(_) = pool.subscribe(two, Topic::Compositor);
        let Ok(()) = pool.left(one);

        let Ok(answered) = pool.publish(&change(Topic::Compositor, "openwindow"));

        assert_eq!(answered, vec![two]);
        let Ok(answered) = pool.wanted();

        assert_eq!(answered, [Topic::Compositor].into());

        let Ok(()) = pool.left(two);

        let Ok(answered) = pool.publish(&change(Topic::Compositor, "closewindow"));

        assert!(answered.is_empty());
        let Ok(answered) = pool.wanted();

        assert!(answered.is_empty());
    }

    #[test]
    fn a_program_that_stopped_caring_about_one_topic_still_hears_the_others() {
        let mut pool = Pool::default();
        let Ok(who) = pool.joined();

        let Ok(_) = pool.subscribe(who, Topic::Sound);
        let Ok(_) = pool.subscribe(who, Topic::Network);
        let Ok(()) = pool.unsubscribe(who, &Topic::Sound);

        let Ok(answered) = pool.publish(&change(Topic::Sound, "40%"));

        assert!(answered.is_empty());
        let Ok(answered) = pool.publish(&change(Topic::Network, "up"));

        assert_eq!(answered, vec![who]);
    }

    #[test]
    fn no_one_is_ever_given_a_name_someone_else_had() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();

        let Ok(()) = pool.left(one);

        let Ok(two) = pool.joined();

        assert_ne!(one, two);
    }

    #[test]
    fn a_word_on_a_topic_no_one_wants_is_still_remembered_for_whoever_comes() {
        let mut pool = Pool::default();

        let Ok(answered) = pool.publish(&change(Topic::Player, "paused"));

        assert!(answered.is_empty());
        let Ok(answered) = pool.last(&Topic::Player);

        assert_eq!(answered, Some("paused"));
    }
}
