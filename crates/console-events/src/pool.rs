//! Who is listening to what, and what the last word on each topic was.
//!
//! All of it, and nothing else. There is no socket in this file and no thread:
//! the pool is arithmetic over two maps, so what happens when two programs
//! want the same topic and one of them goes away can be asked on a laptop
//! instead of by opening twenty-five subscriptions and counting.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;
use console_program_contract::{Changed, Topic};

pub type Who = u64;

#[derive(Debug, Default)]
pub struct Pool {
    listening: BTreeMap<Who, BTreeSet<Topic>>,
    last: BTreeMap<Topic, String>,
    next: Who,
}

impl Pool {
    pub fn joined(&mut self) -> Result<Who, Never> {
        let who = self.next;

        self.next = who.saturating_add(1);
        self.listening.insert(who, BTreeSet::new());

        Ok(who)
    }

    pub fn gone(&mut self, who: Who) -> Result<(), Never> {
        self.listening.remove(&who);

        Ok(())
    }

    pub fn listens(&mut self, who: Who, topic: Topic) -> Result<Option<Changed>, Never> {
        let said = self.last.get(&topic).cloned();

        let Some(topics) = self.listening.get_mut(&who) else { return Ok(None) };

        let _ = topics.insert(topic.clone());

        Ok(said.map(|said| Changed { about: topic, said }))
    }

    pub fn deafen(&mut self, who: Who, topic: &Topic) -> Result<(), Never> {
        match self.listening.get_mut(&who) {
            Some(topics) => {
                let _ = topics.remove(topic);
            }
            None => {},
        }

        Ok(())
    }

    pub fn said(&mut self, changed: &Changed) -> Result<Vec<Who>, Never> {
        self.last.insert(changed.about.clone(), changed.said.clone());

        Ok(self
            .listening
            .iter()
            .filter(|(_, topics)| topics.contains(&changed.about))
            .map(|(who, _)| *who)
            .collect())
    }

    pub fn wanted(&self) -> Result<BTreeSet<Topic>, Never> {
        Ok(self.listening.values().flatten().cloned().collect())
    }

    pub fn last(&self, topic: &Topic) -> Result<Option<&str>, Never> {
        Ok(self.last.get(topic).map(String::as_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn about(topic: Topic, said: &str) -> Changed {
        Changed { about: topic, said: said.to_string() }
    }

    #[test]
    fn a_word_reaches_everybody_who_asked_for_that_topic_and_nobody_else() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();
        let Ok(two) = pool.joined();
        let Ok(three) = pool.joined();

        let Ok(first) = pool.listens(one, Topic::Sound);
        let Ok(second) = pool.listens(two, Topic::Sound);
        let Ok(third) = pool.listens(three, Topic::Network);

        assert_eq!(first, None);
        assert_eq!(second, None);
        assert_eq!(third, None);

        let Ok(answered) = pool.said(&about(Topic::Sound, "sink 1"));

        assert_eq!(answered, vec![one, two]);
    }

    #[test]
    fn whoever_arrives_late_is_told_the_last_word_first() {
        let mut pool = Pool::default();
        let Ok(early) = pool.joined();

        let Ok(answered) = pool.listens(early, Topic::Sound);

        assert_eq!(answered, None);

        let Ok(_said) = pool.said(&about(Topic::Sound, "sink 1 at 40%"));
        let Ok(late) = pool.joined();

        let Ok(answered) = pool.listens(late, Topic::Sound);

        assert_eq!(answered, Some(about(Topic::Sound, "sink 1 at 40%")));
    }

    #[test]
    fn the_last_word_is_the_last_one_and_not_all_of_them() {
        let mut pool = Pool::default();
        let Ok(_said) = pool.said(&about(Topic::Sound, "40%"));
        let Ok(_said) = pool.said(&about(Topic::Sound, "45%"));
        let Ok(late) = pool.joined();

        let Ok(answered) = pool.listens(late, Topic::Sound);

        assert_eq!(answered, Some(about(Topic::Sound, "45%")));
    }

    #[test]
    fn a_program_that_has_gone_is_told_nothing_and_holds_no_source_open() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();
        let Ok(two) = pool.joined();

        let Ok(_said) = pool.listens(one, Topic::Compositor);
        let Ok(_said) = pool.listens(two, Topic::Compositor);
        let Ok(()) = pool.gone(one);

        let Ok(answered) = pool.said(&about(Topic::Compositor, "openwindow"));

        assert_eq!(answered, vec![two]);
        let Ok(answered) = pool.wanted();

        assert_eq!(answered, [Topic::Compositor].into());

        let Ok(()) = pool.gone(two);

        let Ok(answered) = pool.said(&about(Topic::Compositor, "closewindow"));

        assert!(answered.is_empty());
        let Ok(answered) = pool.wanted();

        assert!(answered.is_empty());
    }

    #[test]
    fn a_program_that_stopped_caring_about_one_topic_still_hears_the_others() {
        let mut pool = Pool::default();
        let Ok(who) = pool.joined();

        let Ok(_said) = pool.listens(who, Topic::Sound);
        let Ok(_said) = pool.listens(who, Topic::Network);
        let Ok(()) = pool.deafen(who, &Topic::Sound);

        let Ok(answered) = pool.said(&about(Topic::Sound, "40%"));

        assert!(answered.is_empty());
        let Ok(answered) = pool.said(&about(Topic::Network, "up"));

        assert_eq!(answered, vec![who]);
    }

    #[test]
    fn nobody_is_ever_given_a_name_somebody_else_had() {
        let mut pool = Pool::default();
        let Ok(one) = pool.joined();

        let Ok(()) = pool.gone(one);

        let Ok(two) = pool.joined();

        assert_ne!(one, two);
    }

    #[test]
    fn a_word_on_a_topic_nobody_wants_is_still_remembered_for_whoever_comes() {
        let mut pool = Pool::default();

        let Ok(answered) = pool.said(&about(Topic::Player, "paused"));

        assert!(answered.is_empty());
        let Ok(answered) = pool.last(&Topic::Player);

        assert_eq!(answered, Some("paused"));
    }
}
