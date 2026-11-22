use crate::xkb::diagnostic::Diagnostic;
use crate::xkb::diagnostic::DiagnosticHandler;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::Fatality;
use crate::xkb::diagnostic::Relevance;
use crate::xkb::diagnostic::Severity;
use console_core_never::Never;

impl DiagnosticHandler for Vec<Diagnostic> {
  fn filter(&self, _kind: DiagnosticKind, _fatality: Fatality) -> Result<Relevance, Never> {
    Ok(Relevance::Relevant)
  }

  fn handle(&mut self, diag: Diagnostic) {
    self.push(diag);
  }
}

impl<T> DiagnosticHandler for &'_ mut T
where
  T: ?Sized + DiagnosticHandler,
{
  fn filter(&self, kind: DiagnosticKind, fatality: Fatality) -> Result<Relevance, Never> {
    T::filter(*self, kind, fatality)
  }

  fn handle(&mut self, diag: Diagnostic) {
    T::handle(self, diag);
  }
}

impl<T, F> DiagnosticHandler for (T, F)
where
  T: DiagnosticHandler,
  F: Fn(DiagnosticKind, Fatality) -> Relevance,
{
  fn filter(&self, kind: DiagnosticKind, fatality: Fatality) -> Result<Relevance, Never> {
    match self.1(kind, fatality) {
      Relevance::Relevant => self.0.filter(kind, fatality),
      Relevance::Irrelevant => Ok(Relevance::Irrelevant),
    }
  }

  fn handle(&mut self, diag: Diagnostic) {
    self.0.handle(diag);
  }
}

impl<T> DiagnosticHandler for (T, Severity)
where
  T: DiagnosticHandler,
{
  fn filter(&self, kind: DiagnosticKind, fatality: Fatality) -> Result<Relevance, Never> {
    let Ok(severity) = kind.severity();

    match ::core::primitive::bool::then_some(severity >= self.1, ()) {
      ::core::option::Option::Some(()) => self.0.filter(kind, fatality),
      ::core::option::Option::None => Ok(Relevance::Irrelevant),
    }
  }

  fn handle(&mut self, diag: Diagnostic) {
    self.0.handle(diag);
  }
}

pub(crate) mod stderr {}
