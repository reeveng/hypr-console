//! Stopping being told, and starting again.
//!
//! A panel behind a picker wants nothing until it is uncovered, and what it
//! wants the moment it is uncovered is *what is true now* rather than the next
//! thing to change. Those are the two halves of the same question and neither
//! can be asked of `pool` alone: the first is a line down a socket that the
//! serving loop has to act on, and the second is the replay arriving because
//! someone subscribed again rather than because a source said anything. So
//! this is the real serving loop, the real wire and a source the test speaks
//! through, and the source is watched to prove it was opened once.
//!
//! Getting in is asserted here as well, in the one place its order can be
//! relied on: it comes before any word a source says. `bar-door` is the watch
//! with no tick underneath it, and that word is the whole of how it recovers
//! from a gap.

mod pool;

use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use console_events::subscription::{Desired, Received, connect_at};
use console_program_contract::EventGroup;
use pool::{Failure, BEFORE_LONG, before_long, change, serve_at, socket};

#[test]
fn a_program_that_asks_again_is_told_what_is_true_now_rather_than_waiting_for_a_change() -> Result<(), Failure> {
    let at = socket("stopped-listening")?;
    let handed = serve_at(&at)?;

    let Ok(subscriber) = connect_at(&at, &[EventGroup::Sound]);
    let Ok(heard) = subscriber.received();
    let saying = handed.recv_timeout(BEFORE_LONG).map_err(|_| "the source was never opened")?;

    assert_eq!(
        heard.recv_timeout(BEFORE_LONG),
        Ok(Received::Connected),
        "getting in was not said, so a watch whose words mean *ask again* has no way to know \
         it was away"
    );

    let Ok(forty) = change("40%");

    saying.send(forty.clone()).map_err(|_| "the pool stopped listening to its own source")?;

    assert_eq!(before_long(heard), Ok(Some(forty.clone())));

    let Ok(()) = subscriber.unsubscribe(&EventGroup::Sound);
    let Ok(wanting) = subscriber.desired();

    assert_eq!(wanting, Desired::None, "it still wants what it just gave up");

    let Ok(()) = subscriber.subscribe(&EventGroup::Sound);

    assert_eq!(
        before_long(heard),
        Ok(Some(forty)),
        "a program that started listening again was told nothing until the next change, which \
         is a panel coming back with a reading it cannot have"
    );

    let opened_again = handed.recv_timeout(Duration::from_millis(200));

    assert!(
        matches!(opened_again, Err(RecvTimeoutError::Timeout)),
        "the pool opened the source a second time for a program that had never left it"
    );

    let _ = std::fs::remove_file(&at);

    Ok(())
}
