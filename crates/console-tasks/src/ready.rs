//! What must hold before a deploy, in the order it is asked.
//!
//! The cheap questions that can fail for a reason nobody meant go first: the
//! map drawn again and compared, the retired words, the words written out of
//! proportion. Then the build, because the tests of a panel open its program
//! and a stale one is a picture of yesterday. Then the tests, clippy, the
//! feature checks here and the rules, each over only the crates a change since
//! the last pass could reach.
//!
//! A pass is written down only for a tree that was committed and still: a pass
//! over files that were being edited is a pass over something that no longer
//! exists. And a tree that already passed is not asked again.

use console_core_never::Never;

use crate::reached::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Map,
    MapUnchanged,
    RetiredWords,
    WordsInProportion,
    Build,
    Tests(Scope),
    Clippy(Scope),
    Checks,
    Rules(Scope),
}

pub fn steps(scope: &Scope) -> Result<Vec<Step>, Never> {
    Ok(vec![
        Step::Map,
        Step::MapUnchanged,
        Step::RetiredWords,
        Step::WordsInProportion,
        Step::Build,
        Step::Tests(scope.clone()),
        Step::Clippy(scope.clone()),
        Step::Checks,
        Step::Rules(scope.clone()),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    Committed,
    Loose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Earlier {
    Passed,
    Ask { since: Option<String> },
}

pub fn earlier(tree: Tree, now: &str, recorded: Option<&str>) -> Result<Earlier, Never> {
    let since = recorded.map(|tree| tree.trim().to_string()).filter(|tree| !tree.is_empty());

    Ok(match (tree, since) {
        (Tree::Committed, Some(since)) => match since == now {
            true => Earlier::Passed,
            false => Earlier::Ask { since: Some(since) },
        },
        (Tree::Committed, None) | (Tree::Loose, None) => Earlier::Ask { since: None },
        (Tree::Loose, Some(since)) => Earlier::Ask { since: Some(since) },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    Write,
    Leave,
}

pub fn record(before: Tree, after: Tree, same_tree: Same) -> Result<Record, Never> {
    Ok(match (before, after, same_tree) {
        (Tree::Committed, Tree::Committed, Same::Yes) => Record::Write,
        (Tree::Committed, Tree::Committed, Same::No) | (Tree::Committed | Tree::Loose, Tree::Loose, Same::Yes | Same::No) | (Tree::Loose, Tree::Committed, Same::Yes | Same::No) => {
            Record::Leave
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
    Yes,
    No,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_committed_tree_that_passed_is_not_asked_again() {
        let Ok(answer) = earlier(Tree::Committed, "abc", Some("abc\n"));

        assert_eq!(answer, Earlier::Passed);
    }

    #[test]
    fn a_loose_tree_is_asked_even_when_its_commit_passed() {
        let Ok(answer) = earlier(Tree::Loose, "abc", Some("abc"));

        assert_eq!(answer, Earlier::Ask { since: Some("abc".to_string()) });
    }

    #[test]
    fn with_no_pass_written_down_there_is_nothing_to_measure_from() {
        let Ok(answer) = earlier(Tree::Committed, "abc", Some("  \n"));

        assert_eq!(answer, Earlier::Ask { since: None });
    }

    #[test]
    fn a_pass_is_written_only_for_a_tree_that_stayed_committed_and_still() {
        let Ok(written) = record(Tree::Committed, Tree::Committed, Same::Yes);
        let Ok(moved) = record(Tree::Committed, Tree::Committed, Same::No);
        let Ok(edited) = record(Tree::Committed, Tree::Loose, Same::Yes);
        let Ok(started_loose) = record(Tree::Loose, Tree::Committed, Same::Yes);

        assert_eq!((written, moved, edited, started_loose), (Record::Write, Record::Leave, Record::Leave, Record::Leave));
    }

    #[test]
    fn the_cheap_questions_are_asked_before_the_build_and_the_rules_last() {
        let Ok(asked) = steps(&Scope::Workspace);

        assert_eq!(asked.first(), Some(&Step::Map));
        assert_eq!(asked.iter().position(|step| *step == Step::Build), Some(4));
        assert_eq!(asked.last(), Some(&Step::Rules(Scope::Workspace)));
    }
}
