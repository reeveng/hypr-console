//! What an object on the bus says it is, for whoever calls `Introspect`.
//!
//! The answer is a small XML document, and both services on this desktop had
//! been writing theirs out by hand as one long string: the method list typed a
//! second time beside the code that answers the methods, where nothing checks
//! that the two still agree. What is here is the shape of that document said as
//! values -- an interface is its methods, its signals and its properties -- so
//! the description can be built from the same names the answering code uses.
//!
//! A signal's arguments carry no direction, because a signal only ever goes
//! out; a method's are split into what it takes and what it gives rather than
//! tagged, which is the same fact without a direction to get wrong.

use console_core_never::Never;

pub const INTROSPECTABLE: &str = "org.freedesktop.DBus.Introspectable";

pub const DOCTYPE: &str = "<!DOCTYPE node PUBLIC \"-//freedesktop//DTD D-BUS Object Introspection 1.0//EN\"\n \
                           \"http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd\">\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

impl Access {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Access::Read => "read",
            Access::Write => "write",
            Access::ReadWrite => "readwrite",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub shape: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub taking: Vec<Argument>,
    pub giving: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub name: String,
    pub carrying: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub shape: String,
    pub access: Access,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub methods: Vec<Method>,
    pub signals: Vec<Signal>,
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    In,
    Out,
}

fn argument(argument: &Argument, direction: Option<Direction>) -> Result<String, Never> {
    let direction = match direction {
        Some(Direction::In) => " direction=\"in\"",
        Some(Direction::Out) => " direction=\"out\"",
        None => "",
    };

    Ok(format!("      <arg name=\"{}\" type=\"{}\"{direction}/>\n", argument.name, argument.shape))
}

fn arguments(listed: &[Argument], direction: Option<Direction>) -> Result<String, Never> {
    Ok(listed
        .iter()
        .map(|one| {
            let Ok(line) = argument(one, direction);

            line
        })
        .collect())
}

fn method(method: &Method) -> Result<String, Never> {
    let Ok(taking) = arguments(&method.taking, Some(Direction::In));
    let Ok(giving) = arguments(&method.giving, Some(Direction::Out));

    Ok(format!("    <method name=\"{}\">\n{taking}{giving}    </method>\n", method.name))
}

fn signal(signal: &Signal) -> Result<String, Never> {
    let Ok(carrying) = arguments(&signal.carrying, None);

    Ok(format!("    <signal name=\"{}\">\n{carrying}    </signal>\n", signal.name))
}

fn property(property: &Property) -> Result<String, Never> {
    let Ok(access) = property.access.word();

    Ok(format!(
        "    <property name=\"{}\" type=\"{}\" access=\"{access}\"/>\n",
        property.name, property.shape
    ))
}

fn interface(interface: &Interface) -> Result<String, Never> {
    let methods: String = interface
        .methods
        .iter()
        .map(|one| {
            let Ok(said) = method(one);

            said
        })
        .collect();
    let signals: String = interface
        .signals
        .iter()
        .map(|one| {
            let Ok(said) = signal(one);

            said
        })
        .collect();
    let properties: String = interface
        .properties
        .iter()
        .map(|one| {
            let Ok(said) = property(one);

            said
        })
        .collect();

    Ok(format!(
        "  <interface name=\"{}\">\n{methods}{signals}{properties}  </interface>\n",
        interface.name
    ))
}

pub fn xml(interfaces: &[Interface]) -> Result<String, Never> {
    let inside: String = interfaces
        .iter()
        .map(|one| {
            let Ok(said) = interface(one);

            said
        })
        .collect();

    Ok(format!("{DOCTYPE}<node>\n{inside}</node>\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> Interface {
        Interface { name: name.to_string(), methods: Vec::new(), signals: Vec::new(), properties: Vec::new() }
    }

    fn arg(name: &str, shape: &str) -> Argument {
        Argument { name: name.to_string(), shape: shape.to_string() }
    }

    #[test]
    fn a_method_says_what_it_takes_and_what_it_gives() {
        let mut shown = named("org.example.Player");

        shown.methods.push(Method {
            name: "OpenUri".to_string(),
            taking: vec![arg("Uri", "s")],
            giving: vec![arg("Opened", "b")],
        });

        let Ok(said) = xml(&[shown]);

        assert!(said.contains("<interface name=\"org.example.Player\">"), "{said}");
        assert!(said.contains("<method name=\"OpenUri\">"), "{said}");
        assert!(said.contains("<arg name=\"Uri\" type=\"s\" direction=\"in\"/>"), "{said}");
        assert!(said.contains("<arg name=\"Opened\" type=\"b\" direction=\"out\"/>"), "{said}");
    }

    #[test]
    fn a_signal_has_no_direction_to_say() {
        let mut shown = named("org.example.Player");

        shown.signals.push(Signal { name: "Seeked".to_string(), carrying: vec![arg("Position", "x")] });

        let Ok(said) = xml(&[shown]);

        assert!(said.contains("<signal name=\"Seeked\">"), "{said}");
        assert!(said.contains("<arg name=\"Position\" type=\"x\"/>"), "{said}");
        assert!(!said.contains("direction"), "{said}");
    }

    #[test]
    fn a_property_says_who_may_touch_it() {
        let mut shown = named("org.example.Player");

        shown.properties.push(Property {
            name: "Volume".to_string(),
            shape: "d".to_string(),
            access: Access::ReadWrite,
        });

        let Ok(said) = xml(&[shown]);

        assert!(said.contains("<property name=\"Volume\" type=\"d\" access=\"readwrite\"/>"), "{said}");
    }

    #[test]
    fn the_document_is_one_node_with_every_interface_in_it() {
        let Ok(said) = xml(&[named("org.example.One"), named("org.example.Two")]);

        assert!(said.starts_with("<!DOCTYPE node"), "{said}");
        assert_eq!(said.matches("<node>").count(), 1, "{said}");
        assert!(said.ends_with("</node>\n"), "{said}");
        assert_eq!(said.matches("<interface ").count(), 2, "{said}");
    }
}
