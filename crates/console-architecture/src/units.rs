//! What the unit files say about which program runs and what it runs beside.
//!
//! A unit is read for the four things the map draws: the program `ExecStart`
//! names, the units it is ordered after, the one it is part of, and, for a
//! timer, the service it starts. Drop-ins are not read: none of them names a
//! program or an ordering today, and a map that read them would be answering
//! a question nobody has asked it.

use console_core_ini_files::{Key, Under, field};
use console_core_never::Never;

pub const SERVICE: Under<'static> = Under("Service");

pub const UNIT: Under<'static> = Under("Unit");

pub const TIMER: Under<'static> = Under("Timer");

pub const EXEC_START: Key<'static> = Key("ExecStart");

pub const TIMER_UNIT: Key<'static> = Key("Unit");

pub const ORDERINGS: [(Key<'static>, &str); 5] = [
    (Key("PartOf"), "part of"),
    (Key("BindsTo"), "bound to"),
    (Key("Requires"), "requires"),
    (Key("Wants"), "wants"),
    (Key("After"), "after"),
];

pub const OURS: &str = "/usr/local/bin/";

pub const TIMER_SUFFIX: &str = ".timer";

pub const SERVICE_SUFFIX: &str = ".service";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unit {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Started {
    InternalProgram(String),
    ExternalProgram(String),
}

impl Unit {
    pub fn started(&self) -> Result<Option<Started>, Never> {
        let Ok(said) = field(&self.text, SERVICE, EXEC_START);
        let first = said
            .and_then(|command| command.split_whitespace().next())
            .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']));

        Ok(first.map(|path| match path.strip_prefix(OURS) {
            Some(ours) => Started::InternalProgram(String::from(ours)),
            None => Started::ExternalProgram(match path.rsplit_once('/') {
                Some((_, name)) => String::from(name),
                None => String::from(path),
            }),
        }))
    }

    pub fn ordered(&self) -> Result<Vec<(String, &'static str)>, Never> {
        let mut found = Vec::new();

        for (key, said) in ORDERINGS {
            let Ok(named) = field(&self.text, UNIT, key);

            for other in named.into_iter().flat_map(str::split_whitespace) {
                found.push((String::from(other), said));
            }
        }

        Ok(found)
    }

    pub fn triggers(&self) -> Result<Option<String>, Never> {
        let Ok(named) = field(&self.text, TIMER, TIMER_UNIT);

        Ok(match (named, self.name.strip_suffix(TIMER_SUFFIX)) {
            (Some(named), Some(_)) => Some(String::from(named)),
            (None, Some(stem)) => Some(format!("{stem}{SERVICE_SUFFIX}")),
            (Some(_) | None, None) => None,
        })
    }
}
