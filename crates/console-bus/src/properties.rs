//! `org.freedesktop.DBus.Properties`: a service's state, asked for by name.
//!
//! Three calls and a signal. `Get` and `GetAll` read, `Set` writes, and
//! `PropertiesChanged` is what a service says when something moved, so that a
//! bar or a lock screen watching it does not have to ask again. What a property
//! means belongs to the service; what is here is the part every service would
//! otherwise write again -- where the values are held, which faults the
//! specification names for a call that cannot be answered, and the order the
//! signal's arguments come in.
//!
//! That order is the fault this was written against. The signal says the
//! *interface* first and the object nowhere, because the object is already the
//! message's path; the first draft of this file put the path where the
//! interface goes, and every listener would have filed the change under an
//! interface nobody has.
//!
//! A `Set` that is answered changes the value held here and hands back what
//! changed, rather than saying the signal itself: a service usually has
//! something to do about a write before anybody should hear about it.

use std::collections::BTreeMap;

use console_core_never::Never;

use crate::introspection::{self, Access};
use crate::messages::{Message, Signal, ValidationError, Value};

pub const INTERFACE: &str = "org.freedesktop.DBus.Properties";

pub const GET: &str = "Get";

pub const GET_ALL: &str = "GetAll";

pub const SET: &str = "Set";

pub const CHANGED: &str = "PropertiesChanged";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectPath<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceName<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyName<'a>(pub &'a str);

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub declared: introspection::Property,
    pub value: Value,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Properties {
    held: BTreeMap<String, BTreeMap<String, Property>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyChange {
    pub interface: String,
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub reply: Message,
    pub changed: Option<PropertyChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum May {
    Yes,
    No,
}

fn readable(access: Access) -> Result<May, Never> {
    Ok(match access {
        Access::Read | Access::ReadWrite => May::Yes,
        Access::Write => May::No,
    })
}

fn writable(access: Access) -> Result<May, Never> {
    Ok(match access {
        Access::Write | Access::ReadWrite => May::Yes,
        Access::Read => May::No,
    })
}

impl Properties {
    pub fn add(&mut self, interface: &str, property: Property) -> Result<(), Never> {
        let _replaced = self
            .held
            .entry(interface.to_string())
            .or_default()
            .insert(property.declared.name.clone(), property);

        Ok(())
    }

    pub fn value(&self, interface: InterfaceName<'_>, name: PropertyName<'_>) -> Result<Option<&Value>, Never> {
        Ok(self
            .held
            .get(interface.0)
            .and_then(|named| named.get(name.0))
            .map(|property| &property.value))
    }

    pub fn declared(&self, interface: &str) -> Result<Vec<introspection::Property>, Never> {
        Ok(match self.held.get(interface) {
            Some(named) => named.values().map(|property| property.declared.clone()).collect(),
            None => Vec::new(),
        })
    }

    fn found(&self, interface: InterfaceName<'_>, name: PropertyName<'_>) -> Result<Option<(&str, &Property)>, Never> {
        let (interface, name) = (interface.0, name.0);

        Ok(match interface.is_empty() {
            true => self.held.iter().find_map(|(on, named)| {
                named.get(name).map(|property| (on.as_str(), property))
            }),
            false => self
                .held
                .get_key_value(interface)
                .and_then(|(on, named)| named.get(name).map(|property| (on.as_str(), property))),
        })
    }
}

fn wrapped(property: &Property) -> Result<Value, Never> {
    Value::held(&property.declared.shape, property.value.clone())
}

fn complaint(message: &Message, fault: ValidationError, why: &str) -> Result<Response, Never> {
    let Ok(reply) = message.complaining(fault, why);

    Ok(Response { reply, changed: None })
}

fn words(message: &Message) -> Result<(Option<&str>, Option<&str>), Never> {
    let Ok(interface) = match message.values.first() {
        Some(value) => value.text(),
        None => Ok(None),
    };
    let Ok(name) = match message.values.get(1) {
        Some(value) => value.text(),
        None => Ok(None),
    };

    Ok((interface, name))
}

fn get(message: &Message, properties: &Properties) -> Result<Response, Never> {
    let Ok(said) = words(message);

    let (interface, name) = match said {
        (Some(interface), Some(name)) => (interface, name),
        (Some(_), None) | (None, Some(_)) | (None, None) => {
            return complaint(message, ValidationError::InvalidArgs, "Get takes an interface and a name");
        }
    };

    let Ok(found) = properties.found(InterfaceName(interface), PropertyName(name));

    let (_on, property) = match found {
        Some(found) => found,
        None => return complaint(message, ValidationError::UnknownProperty, name),
    };

    let Ok(may) = readable(property.declared.access);

    match may {
        May::Yes => {}
        May::No => return complaint(message, ValidationError::InvalidArgs, "that property is written, not read"),
    }

    let Ok(value) = wrapped(property);
    let Ok(reply) = message.answering();
    let Ok(reply) = reply.carrying("v", vec![value]);

    Ok(Response { reply, changed: None })
}

fn get_all(message: &Message, properties: &Properties) -> Result<Response, Never> {
    let Ok(said) = words(message);

    let interface = match said {
        (Some(interface), _) => interface,
        (None, _) => return complaint(message, ValidationError::InvalidArgs, "GetAll takes an interface"),
    };

    let named = match properties.held.get(interface) {
        Some(named) => named,
        None => return complaint(message, ValidationError::UnknownInterface, interface),
    };

    let listed: Vec<Value> = named
        .values()
        .filter(|property| {
            let Ok(may) = readable(property.declared.access);

            may == May::Yes
        })
        .map(|property| {
            let Ok(value) = wrapped(property);

            Value::Group(vec![Value::Word(property.declared.name.clone()), value])
        })
        .collect();

    let Ok(reply) = message.answering();
    let Ok(reply) = reply.carrying("a{sv}", vec![Value::List(listed)]);

    Ok(Response { reply, changed: None })
}

fn set(message: &Message, properties: &mut Properties) -> Result<Response, Never> {
    let Ok(said) = words(message);

    let (interface, name) = match said {
        (Some(interface), Some(name)) => (interface.to_string(), name.to_string()),
        (Some(_), None) | (None, Some(_)) | (None, None) => {
            return complaint(message, ValidationError::InvalidArgs, "Set takes an interface, a name and a value");
        }
    };

    let (shape, value) = match message.values.get(2) {
        Some(Value::Variant { shape, value }) => (shape.clone(), (**value).clone()),
        Some(_) | None => return complaint(message, ValidationError::InvalidArgs, "Set takes a variant"),
    };

    let Ok(found) = properties.found(InterfaceName(&interface), PropertyName(&name));

    let (on, declared) = match found {
        Some((on, property)) => (on.to_string(), property.declared.clone()),
        None => return complaint(message, ValidationError::UnknownProperty, &name),
    };

    let Ok(may) = writable(declared.access);

    match may {
        May::Yes => {}
        May::No => return complaint(message, ValidationError::PropertyReadOnly, &name),
    }

    match shape == declared.shape {
        true => {}
        false => {
            return complaint(
                message,
                ValidationError::InvalidArgs,
                &format!("{name} is {}, and {shape} was sent", declared.shape),
            );
        }
    }

    let Ok(()) = properties.add(&on, Property { declared, value: value.clone() });
    let Ok(reply) = message.answering();

    Ok(Response { reply, changed: Some(PropertyChange { interface: on, name, value }) })
}

pub fn answer(message: &Message, properties: &mut Properties) -> Result<Response, Never> {
    match message.member.as_deref() {
        Some(GET) => get(message, properties),
        Some(GET_ALL) => get_all(message, properties),
        Some(SET) => set(message, properties),
        Some(_) | None => complaint(message, ValidationError::UnknownMethod, "Properties answers Get, GetAll and Set"),
    }
}

pub fn changed(at: ObjectPath<'_>, interface: InterfaceName<'_>, changes: &[&Property]) -> Result<Message, Never> {
    let Ok(signal) = Message::signal(&Signal { at: at.0, on: INTERFACE, name: CHANGED });

    let listed: Vec<Value> = changes
        .iter()
        .map(|property| {
            let Ok(value) = wrapped(property);

            Value::Group(vec![Value::Word(property.declared.name.clone()), value])
        })
        .collect();

    signal.carrying(
        "sa{sv}as",
        vec![Value::Word(interface.0.to_string()), Value::List(listed), Value::List(Vec::new())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{Kind, Whom};

    const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

    fn property(name: &str, shape: &str, access: Access, value: Value) -> Property {
        Property {
            declared: introspection::Property { name: name.to_string(), shape: shape.to_string(), access },
            value,
        }
    }

    fn held() -> Properties {
        let mut properties = Properties::default();
        let Ok(()) = properties.add(PLAYER, property("PlaybackStatus", "s", Access::Read, Value::Word("Playing".to_string())));
        let Ok(()) = properties.add(PLAYER, property("Volume", "d", Access::ReadWrite, Value::Fraction(0.5)));
        let Ok(()) = properties.add(PLAYER, property("Secret", "s", Access::Write, Value::Word("hidden".to_string())));

        properties
    }

    fn calling(member: &str, values: Vec<Value>) -> Message {
        let Ok(call) = Message::call(&Whom { to: "org.example", at: "/org/example", on: INTERFACE, calling: member });

        Message { serial: 7, sender: Some(":1.9".to_string()), values, ..call }
    }

    fn word(said: &str) -> Value {
        Value::Word(said.to_string())
    }

    fn fault(answered: &Response) -> Option<&str> {
        answered.reply.fault.as_deref()
    }

    #[test]
    fn get_answers_the_value_in_its_declared_shape() {
        let mut properties = held();
        let Ok(answered) = answer(&calling(GET, vec![word(PLAYER), word("Volume")]), &mut properties);

        assert_eq!(answered.reply.kind, Kind::Answer);
        assert_eq!(answered.reply.reply_to, Some(7));
        assert_eq!(answered.reply.shape, "v");
        assert_eq!(
            answered.reply.values,
            vec![Value::Variant { shape: "d".to_string(), value: Box::new(Value::Fraction(0.5)) }]
        );
    }

    #[test]
    fn a_property_nobody_has_is_the_fault_the_specification_names() {
        let mut properties = held();
        let Ok(answered) = answer(&calling(GET, vec![word(PLAYER), word("Nothing")]), &mut properties);

        assert_eq!(fault(&answered), Some("org.freedesktop.DBus.Error.UnknownProperty"));
    }

    #[test]
    fn an_empty_interface_finds_the_name_wherever_it_is() {
        let mut properties = held();
        let Ok(answered) = answer(&calling(GET, vec![word(""), word("PlaybackStatus")]), &mut properties);

        assert_eq!(fault(&answered), None);
    }

    #[test]
    fn get_all_leaves_out_what_may_only_be_written() {
        let mut properties = held();
        let Ok(answered) = answer(&calling(GET_ALL, vec![word(PLAYER)]), &mut properties);

        let listed = match answered.reply.values.first() {
            Some(Value::List(listed)) => listed.clone(),
            Some(_) | None => Vec::new(),
        };
        let names: Vec<&str> = listed
            .iter()
            .filter_map(|entry| {
                let Ok(pair) = entry.pair();

                pair.and_then(|(name, _)| {
                    let Ok(text) = name.text();

                    text
                })
            })
            .collect();

        assert_eq!(answered.reply.shape, "a{sv}");
        assert_eq!(names, vec!["PlaybackStatus", "Volume"]);
    }

    #[test]
    fn get_all_of_an_interface_nobody_has_says_so() {
        let mut properties = held();
        let Ok(answered) = answer(&calling(GET_ALL, vec![word("org.example.Nothing")]), &mut properties);

        assert_eq!(fault(&answered), Some("org.freedesktop.DBus.Error.UnknownInterface"));
    }

    #[test]
    fn set_writes_the_value_and_says_what_changed() {
        let mut properties = held();
        let sent = Value::Variant { shape: "d".to_string(), value: Box::new(Value::Fraction(0.8)) };
        let Ok(answered) = answer(&calling(SET, vec![word(PLAYER), word("Volume"), sent]), &mut properties);

        assert_eq!(fault(&answered), None);
        assert_eq!(
            answered.changed,
            Some(PropertyChange { interface: PLAYER.to_string(), name: "Volume".to_string(), value: Value::Fraction(0.8) })
        );
        assert_eq!(properties.value(InterfaceName(PLAYER), PropertyName("Volume")), Ok(Some(&Value::Fraction(0.8))));
    }

    #[test]
    fn set_on_a_property_that_is_only_read_is_refused_and_changes_nothing() {
        let mut properties = held();
        let sent = Value::Variant { shape: "s".to_string(), value: Box::new(word("Stopped")) };
        let Ok(answered) = answer(&calling(SET, vec![word(PLAYER), word("PlaybackStatus"), sent]), &mut properties);

        assert_eq!(fault(&answered), Some("org.freedesktop.DBus.Error.PropertyReadOnly"));
        assert_eq!(answered.changed, None);
        assert_eq!(properties.value(InterfaceName(PLAYER), PropertyName("PlaybackStatus")), Ok(Some(&word("Playing"))));
    }

    #[test]
    fn set_in_the_wrong_shape_is_refused_and_changes_nothing() {
        let mut properties = held();
        let sent = Value::Variant { shape: "s".to_string(), value: Box::new(word("loud")) };
        let Ok(answered) = answer(&calling(SET, vec![word(PLAYER), word("Volume"), sent]), &mut properties);

        assert_eq!(fault(&answered), Some("org.freedesktop.DBus.Error.InvalidArgs"));
        assert_eq!(properties.value(InterfaceName(PLAYER), PropertyName("Volume")), Ok(Some(&Value::Fraction(0.5))));
    }

    #[test]
    fn the_signal_names_the_interface_first_and_not_the_object() {
        let volume = property("Volume", "d", Access::ReadWrite, Value::Fraction(0.8));
        let Ok(said) = changed(ObjectPath("/org/mpris/MediaPlayer2"), InterfaceName(PLAYER), &[&volume]);

        assert_eq!(said.kind, Kind::Signal);
        assert_eq!(said.path.as_deref(), Some("/org/mpris/MediaPlayer2"));
        assert_eq!(said.interface.as_deref(), Some(INTERFACE));
        assert_eq!(said.member.as_deref(), Some(CHANGED));
        assert_eq!(said.shape, "sa{sv}as");
        assert_eq!(said.values.first(), Some(&word(PLAYER)));
    }

    #[test]
    fn the_signal_is_bytes_a_bus_would_take() {
        let volume = property("Volume", "d", Access::ReadWrite, Value::Fraction(0.8));
        let Ok(said) = changed(ObjectPath("/org/mpris/MediaPlayer2"), InterfaceName(PLAYER), &[&volume]);

        assert!(said.bytes(1).is_ok(), "{:?}", said.bytes(1));
    }
}
