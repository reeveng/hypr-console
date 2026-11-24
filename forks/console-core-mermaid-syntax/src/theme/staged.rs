//! Ordered execution for the classic Mermaid theme classes.
//!
//! Mermaid theme classes are mutable JavaScript objects, but their public behavior is a small,
//! deterministic staged protocol. This module keeps that protocol behind one interface and
//! records ownership from the same writes that produce values. It intentionally models only the
//! three assignment operators used by the pinned theme sources: unconditional assignment,
//! JavaScript `||`, and JavaScript `??`.

use super::{ThemeResolutionError, is_js_truthy, value_is_missing};
#[cfg(test)]
use crate::MermaidConfig;
use crate::json_value::number_value;
use crate::presentation::SourcePresentationConfig;
use crate::theme_color::{self, ColorAdjustment, ColorError};
use merman_model::presentation::ThemeEvaluationLimitExceeded;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::sync::OnceLock;

mod base;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StagedProgram {
    Base,
    Dark,
    Forest,
    Neutral,
}

impl StagedProgram {
    pub(super) fn resolved_without_overrides(
        self,
    ) -> Result<&'static Map<String, Value>, ThemeResolutionError> {
        use super::{MermaidThemeId, ThemeProgram};

        static BASE: OnceLock<Result<Map<String, Value>, ThemeResolutionError>> = OnceLock::new();
        static DARK: OnceLock<Result<Map<String, Value>, ThemeResolutionError>> = OnceLock::new();
        static FOREST: OnceLock<Result<Map<String, Value>, ThemeResolutionError>> = OnceLock::new();
        static NEUTRAL: OnceLock<Result<Map<String, Value>, ThemeResolutionError>> =
            OnceLock::new();

        let (resolved, theme) = match self {
            Self::Base => (&BASE, MermaidThemeId::Base),
            Self::Dark => (&DARK, MermaidThemeId::Dark),
            Self::Forest => (&FOREST, MermaidThemeId::Forest),
            Self::Neutral => (&NEUTRAL, MermaidThemeId::Neutral),
        };
        resolved
            .get_or_init(|| {


                Resolution::execute_impl(
                    self,
                    ThemeProgram::resolve(theme).prepared_constructor(),
                    &Map::new(),
                    None,
                )
                .map(|resolution| resolution.variables)
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

/// The interpreter always executes the same assignments. Authority is an optional, separate
/// data flow: authored leaves are borrowed and become owner identities only when actually read.
#[derive(Debug, Clone)]
pub(super) struct ThemeState<'a> {
    pub(super) variables: Map<String, Value>,
    authority: Option<AuthorityState<'a>>,
    theme_color_iterations: usize,
}

pub(super) struct Resolution {
    variables: Map<String, Value>,
    authority: Option<ResolvedAuthority>,
    #[cfg(test)]
    trace: ResolutionTrace,
    #[cfg(test)]
    work: AuthorityWork,
}

#[cfg(test)]
pub(super) struct ResolutionTrace {
    pub(super) constructor_prepared: StateSnapshot,
    pub(super) overrides_applied: StateSnapshot,
    pub(super) after_update: StateSnapshot,
    pub(super) explicit_replay: StateSnapshot,
    pub(super) final_resolved: StateSnapshot,
}

#[cfg(test)]
pub(super) struct StateSnapshot {
    pub(super) variables: Map<String, Value>,
    dependencies: BTreeMap<String, BTreeSet<String>>,
}

#[cfg(test)]
impl StateSnapshot {
    pub(super) fn depends_on(&self, target: &str, source: &str) -> bool {
        self.dependencies
            .get(target)
            .is_some_and(|sources| sources.contains(source))
    }
}

/// IDs refer to the operation-local source table, never to a value or a theme name.
/// Keeping the set sorted makes unions deterministic without allocating tree nodes per source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Owners(Vec<usize>);

impl Owners {
    fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
        self.0.sort_unstable();
        self.0.dedup();
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
enum OwnerSource<'a> {
    Config(&'a SourcePresentationConfig),
    #[cfg(test)]
    AllExplicit,
}

impl OwnerSource<'_> {
    #[must_use]
    fn owns(self, path: &str) -> bool {
        match self {
            Self::Config(config) => config.theme_variable_has_calculation_owner(path),
            #[cfg(test)]
            Self::AllExplicit => true,
        }
    }
}

#[derive(Debug, Clone)]
enum ValueAuthority<'a> {
    /// A replay write restored this authored subtree and blocks older dotted descendants.
    Authored(&'a Value),
    /// An empty set still blocks the original authored value after an assignment.
    Computed(Owners),
}

#[derive(Debug, Clone, Default)]
struct SourceTable {

    seeds: BTreeMap<String, Option<usize>>,
    paths: Vec<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AuthorityWork {
    reads: usize,
    source_queries: usize,
    replacement_probes: usize,
    removed_replacements: usize,
}

#[derive(Debug, Clone)]
struct AuthorityState<'a> {
    explicit: &'a Map<String, Value>,
    owner_source: OwnerSource<'a>,
    explicit_applied: bool,


    overlay_order: BTreeMap<&'a str, usize>,

    replacements: BTreeMap<String, ValueAuthority<'a>>,
    sources: std::cell::RefCell<SourceTable>,
    #[cfg(test)]
    work: std::cell::Cell<AuthorityWork>,
}

struct ResolvedAuthority {
    paths: Vec<String>,
    dependencies: BTreeMap<String, Owners>,
}

impl<'a> AuthorityState<'a> {
    #[must_use]
    fn new(explicit: &'a Map<String, Value>, owner_source: OwnerSource<'a>) -> Self {
        let mut overlay_order = BTreeMap::new();

        for key in explicit.keys() {
            let mut overlaps_ancestor = false;

            for (index, _) in key.match_indices('.') {
                let ancestor = &key[..index];

                if explicit.contains_key(ancestor) {
                    overlay_order.entry(ancestor).or_default();
                    overlaps_ancestor = true;
                }
            }

            if overlaps_ancestor {
                overlay_order.entry(key.as_str()).or_default();
            }
        }



        if !overlay_order.is_empty() {
            for (order, key) in explicit.keys().enumerate() {
                {
                    match overlay_order.get_mut(key.as_str()) {
                        Some(indexed_order) => {
                            *indexed_order = order;
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
        }

        Self {
            explicit,
            owner_source,
            explicit_applied: false,
            overlay_order,
            replacements: BTreeMap::new(),
            sources: std::cell::RefCell::new(SourceTable::default()),
            #[cfg(test)]
            work: std::cell::Cell::new(AuthorityWork::default()),
        }
    }

    /// Find the nearest replacement, including the path itself. A computed ancestor blocks
    /// authored fallback; replay can restore an authored child without disturbing siblings.
    #[must_use]
    fn origin(&self, path: &str) -> Option<(&str, &ValueAuthority<'a>)> {
        match self.replacements.get_key_value(path) {
            Some((root, origin)) => {
                return Some((root, origin));
            },
            ::core::option::Option::None => {},
        }

        path.rmatch_indices('.').find_map(|(index, _)| {
            self.replacements
                .get_key_value(&path[..index])
                .map(|(root, origin)| (root.as_str(), origin))
        })
    }

    #[must_use]
    fn read(&self, path: &str) -> Owners {
        #[cfg(test)]
        self.record_work(|work| work.reads += 1);

        if !self.explicit_applied {
            return Owners::default();
        }

        let authored_leaf = match self.origin(path) {
            Some((root, ValueAuthority::Computed(owners))) => {

                return if root == path {
                    owners.clone()
                } else {
                    Owners::default()
                };
            }
            Some((root, ValueAuthority::Authored(value))) => {
                authored_value_has_leaf(value, &path[root.len()..])
            }
            None => self.explicit_has_leaf_path(path),
        };

        if !authored_leaf {
            return Owners::default();
        }

        let mut sources = self.sources.borrow_mut();

        match sources.seeds.get(path) {
            Some(owner) => {
                return Owners(owner.iter().copied().collect());
            },
            ::core::option::Option::None => {},
        }

        #[cfg(test)]
        self.record_work(|work| work.source_queries += 1);
        let owner = self.owner_source.owns(path).then(|| {
            let owner = sources.paths.len();
            sources.paths.push(path.to_string());
            owner
        });
        sources.seeds.insert(path.to_string(), owner);
        Owners(owner.into_iter().collect())
    }

    /// Every matching literal root overwrites this path, even when it supplies no leaf there.
    /// Select the last such write in Map order, not the longest dotted prefix.
    #[must_use]
    fn explicit_has_leaf_path(&self, path: &str) -> bool {
        let mut latest: Option<(usize, usize, &Value)> = None;

        for root in
            std::iter::once(path).chain(path.rmatch_indices('.').map(|(index, _)| &path[..index]))
        {
            let value = {
                let optional_value = self.explicit.get(root);

                match optional_value {
                    Some(matched_value) => matched_value,
                    ::core::option::Option::None => continue,
                }
            };

            let order = {
                let optional_order = self.overlay_order.get(root);

                match optional_order {
                    Some(&matched_order) => matched_order,
                    ::core::option::Option::None => return authored_value_has_leaf(value, &path[root.len()..]),
                }
            };

            if latest.is_none_or(|(previous, _, _)| order > previous) {
                latest = Some((order, root.len(), value));
            }
        }

        latest.is_some_and(|(_, root_len, value)| authored_value_has_leaf(value, &path[root_len..]))
    }

    fn remove_subtree(&mut self, root: &str) {
        #[cfg(test)]
        self.record_work(|work| work.replacement_probes += 1);

        if self.replacements.remove(root).is_some() {
            #[cfg(test)]
            self.record_work(|work| work.removed_replacements += 1);
        }

        let prefix = format!("{root}.");

        loop {
            let descendant = self
                .replacements
                .range::<str, _>((
                    std::ops::Bound::Included(prefix.as_str()),
                    std::ops::Bound::Unbounded,
                ))
                .next()
                .filter(|(path, _)| path.starts_with(&prefix))
                .map(|(path, _)| path.clone());

            let descendant = match descendant {
                Some(matched_descendant) => matched_descendant,
                ::core::option::Option::None => break,
            };

            self.replacements.remove(&descendant);
            #[cfg(test)]
            self.record_work(|work| work.removed_replacements += 1);
        }
    }

    fn replay(&mut self, root: &str, value: &'a Value) {
        self.remove_subtree(root);




        let unchanged_root = self
            .explicit
            .get(root)
            .is_some_and(|original| std::ptr::eq(original, value));

        if !unchanged_root || self.origin(root).is_some() || self.overlay_order.contains_key(root) {
            self.replacements
                .insert(root.to_string(), ValueAuthority::Authored(value));
        }
    }

    fn commit(&mut self, target: &str, owners: Owners, nested: BTreeMap<String, Owners>) {
        self.remove_subtree(target);
        self.replacements
            .insert(target.to_string(), ValueAuthority::Computed(owners));

        for (path, owners_2) in nested {
            self.replacements
                .insert(format!("{target}.{path}"), ValueAuthority::Computed(owners_2));
        }
    }

    #[must_use]
    fn finish(self) -> ResolvedAuthority {
        ResolvedAuthority {
            paths: self.sources.into_inner().paths,
            dependencies: self
                .replacements
                .into_iter()
                .filter_map(|(target, origin)| match origin {
                    ValueAuthority::Computed(owners) if !owners.is_empty() => {
                        Some((target, owners))
                    }
                    ValueAuthority::Authored(_) | ValueAuthority::Computed(_) => None,
                })
                .collect(),
        }
    }

    #[cfg(test)]
    fn record_work(&self, record: impl FnOnce(&mut AuthorityWork)) {
        let mut work = self.work.get();
        record(&mut work);
        self.work.set(work);
    }
}

/// Inside one authored value, flattening does not perform intervening subtree replacements.
/// Try each object-key boundary so literal dots retain the original compatibility behavior.
#[must_use]
fn authored_value_has_leaf(value: &Value, suffix: &str) -> bool {
    if suffix.is_empty() {
        return value.as_object().is_none_or(Map::is_empty);
    }

    let object = {
        let optional_object = value.as_object();

        match optional_object {
            Some(matched_object) => matched_object,
            ::core::option::Option::None => return false,
        }
    };

    let path = &suffix[1..];

    if object
        .get(path)
        .is_some_and(|value_2| authored_value_has_leaf(value_2, ""))
    {
        return true;
    }

    path.match_indices('.').any(|(index, _)| {
        object
            .get(&path[..index])
            .is_some_and(|value_3| authored_value_has_leaf(value_3, &path[index..]))
    })
}

#[derive(Debug)]
struct ComputedValue {
    value: Value,
    owners: Owners,
    nested_owners: BTreeMap<String, Owners>,
}

impl ComputedValue {
    #[must_use]
    fn constant(value: impl Into<Value>) -> Self {
        Self {
            value: <_ as ::core::convert::Into<::serde_json::Value>>::into(value),
            owners: Owners::default(),
            nested_owners: BTreeMap::new(),
        }
    }

    #[must_use]
    fn string(value: impl Into<String>) -> Self {
        Self::constant(Value::String(<_ as ::core::convert::Into<::std::string::String>>::into(value)))
    }

    #[must_use]
    fn number(value: i64) -> Self {
        Self::constant(Value::Number(<::serde_json::Number as ::core::convert::From<::core::primitive::i64>>::from(value)))
    }

    #[must_use]
    fn with_owners(value: Value, owners: Owners) -> Self {
        Self {
            value,
            owners,
            nested_owners: BTreeMap::new(),
        }
    }

    fn inherit(&mut self, stage: &ThemeState<'_>, path: &str) {
        self.owners.extend(stage.owners(path));
    }

    #[must_use]
    fn object(value: Map<String, Value>, nested_owners: BTreeMap<String, Owners>) -> Self {
        Self {
            value: Value::Object(value),
            owners: Owners::default(),
            nested_owners,
        }
    }
}

impl<'a> ThemeState<'a> {
    #[must_use]
    fn constructor(
        variables: Map<String, Value>,
        explicit: &'a Map<String, Value>,
        owner_source: Option<OwnerSource<'a>>,
    ) -> Self {
        Self {
            variables,
            authority: owner_source.map(|source| AuthorityState::new(explicit, source)),
            theme_color_iterations: 0,
        }
    }

    fn overlay_explicit(&mut self, explicit: &Map<String, Value>) {
        self.variables.extend(
            explicit
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );

        match &mut self.authority {
            Some(authority) => {
                authority.explicit_applied = true;
            },
            ::core::option::Option::None => {},
        }
    }

    fn replay_explicit(&mut self, explicit: &'a Map<String, Value>) {
        for (key, value) in explicit {
            if self.variables.get(key).is_some_and(Value::is_object)
                && let Value::Object(children) = value
            {
                for (child, value_2) in children {
                    self.variables
                        .get_mut(key)
                        .and_then(Value::as_object_mut)
                        .expect("replay target remains an object")
                        .insert(child.clone(), value_2.clone());

                    match &mut self.authority {
                        Some(authority) => {
                            authority.replay(&format!("{key}.{child}"), value_2);
                        },
                        ::core::option::Option::None => {},
                    }
                }
            } else {
                self.variables.insert(key.clone(), value.clone());

                match &mut self.authority {
                    Some(authority) => {
                        authority.replay(key, value);
                    },
                    ::core::option::Option::None => {},
                }
            }
        }
    }

    #[must_use]
    fn owners(&self, key: &str) -> Owners {
        self.authority
            .as_ref()
            .map_or_else(Owners::default, |authority| authority.read(key))
    }

    #[must_use]
    fn computed(&self, value: Value, keys: &[&str]) -> ComputedValue {
        let mut owners = Owners::default();

        for key in keys {
            owners.extend(self.owners(key));
        }

        ComputedValue::with_owners(value, owners)
    }

    fn computed_from(&self, key: &str) -> Result<ComputedValue, ColorError> {
        let value = self
            .variables
            .get(key)
            .cloned()
            .ok_or_else(|| missing_value_error(key))?;
        Ok(self.computed(value, &[key]))
    }

    #[must_use]
    fn truthy_from(&self, key: &str) -> Option<ComputedValue> {
        self.variables
            .get(key)
            .filter(|value| is_js_truthy(value))
            .cloned()
            .map(|value| self.computed(value, &[key]))
    }

    fn color(&self, key: &str) -> Result<String, ColorError> {
        self.variables
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| missing_value_error(key))
    }

    /// Execute an unconditional JavaScript assignment.
    fn assign(
        &mut self,
        target: &str,
        compute: impl FnOnce(&Self) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        let computed = compute(self)?;
        self.commit_computed(target, computed);
        Ok(())
    }

    fn commit_computed(&mut self, target: &str, computed: ComputedValue) {
        self.variables.insert(target.to_string(), computed.value);

        match &mut self.authority {
            Some(authority) => {
                authority.commit(target, computed.owners, computed.nested_owners);
            },
            ::core::option::Option::None => {},
        }
    }

    /// Execute `target = target || fallback`, including short-circuit evaluation.
    fn assign_if_falsy(
        &mut self,
        target: &str,
        fallback: impl FnOnce(&Self) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        if value_is_missing(&self.variables, target) {
            self.assign(target, fallback)?;
        }

        Ok(())
    }

    /// Execute `target = target ?? fallback`, without treating other falsy values as absent.
    fn assign_if_nullish(
        &mut self,
        target: &str,
        fallback: impl FnOnce(&Self) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        if self.variables.get(target).is_none_or(Value::is_null) {
            self.assign(target, fallback)?;
        }

        Ok(())
    }

    #[cfg(test)]
    fn snapshot(&self) -> StateSnapshot {


        let dependencies = self
            .authority
            .clone()
            .map_or_else(BTreeMap::new, |authority| {
                let mut paths = authority
                    .replacements
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                for (key, value) in authority.explicit {
                    paths.extend(explicit_dependency_leaf_paths(key, value));
                }
                let mut dependencies = BTreeMap::new();
                for path in paths {
                    let owners = authority.read(&path);
                    let sources = authority.sources.borrow();
                    if !owners.is_empty() {
                        dependencies.insert(
                            path,
                            owners
                                .0
                                .iter()
                                .map(|owner| sources.paths[*owner].clone())
                                .collect(),
                        );
                    }
                }
                dependencies
            });
        StateSnapshot {
            variables: self.variables.clone(),
            dependencies,
        }
    }

    #[cfg(test)]
    pub(super) fn depends_on(&self, target: &str, source: &str) -> bool {
        self.snapshot().depends_on(target, source)
    }
}

#[cfg(test)]
fn explicit_dependency_leaf_paths(root: &str, value: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    let mut stack = vec![(root.to_string(), value)];
    while let Some((path, value)) = stack.pop() {
        match value {
            Value::Object(object) if !object.is_empty() => {
                for (key, child) in object.iter().rev() {
                    stack.push((format!("{path}.{key}"), child));
                }
            }
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Object(_) => paths.push(path),
        }
    }
    paths
}

impl Resolution {
    pub(super) fn execute_with_owners(
        program: StagedProgram,
        prepared_constructor: &Map<String, Value>,
        explicit: &Map<String, Value>,
        config: &SourcePresentationConfig,
    ) -> Result<Self, ThemeResolutionError> {
        Self::execute_impl(
            program,
            prepared_constructor,
            explicit,
            config
                .has_theme_variable_calculation_owners()
                .then_some(OwnerSource::Config(config)),
        )
    }

    #[cfg(test)]
    pub(super) fn execute(
        program: StagedProgram,
        prepared_constructor: &Map<String, Value>,
        explicit: &Map<String, Value>,
    ) -> Result<Self, ThemeResolutionError> {
        Self::execute_impl(
            program,
            prepared_constructor,
            explicit,
            Some(OwnerSource::AllExplicit),
        )
    }

    fn execute_impl(
        program: StagedProgram,
        prepared_constructor: &Map<String, Value>,
        explicit: &Map<String, Value>,
        owner_source: Option<OwnerSource<'_>>,
    ) -> Result<Self, ThemeResolutionError> {
        let mut state =
            ThemeState::constructor(prepared_constructor.clone(), explicit, owner_source);
        #[cfg(test)]
        let constructor_prepared = state.snapshot();

        state.overlay_explicit(explicit);
        #[cfg(test)]
        let overrides_applied = state.snapshot();
        state.theme_color_iterations = theme_color_iteration_count(&state)?;

        match program {
            StagedProgram::Base => base::update(&mut state)?,
            StagedProgram::Dark => update_dark(&mut state)?,
            StagedProgram::Forest => update_forest(&mut state)?,
            StagedProgram::Neutral => update_neutral(&mut state)?,
        }

        #[cfg(test)]
        let after_update = state.snapshot();

        state.replay_explicit(explicit);
        #[cfg(test)]
        let explicit_replay = state.snapshot();

        if program == StagedProgram::Base
            && explicit.contains_key("nodeBorder")
            && !explicit.contains_key("useGradient")
        {
            let computed = state.computed(Value::Bool(false), &["nodeBorder"]);
            state.commit_computed("useGradient", computed);
        }

        #[cfg(test)]
        let final_resolved = state.snapshot();
        #[cfg(test)]
        let work = state
            .authority
            .as_ref()
            .map_or_else(AuthorityWork::default, |authority| authority.work.get());

        Ok(Self {
            variables: state.variables,
            authority: state.authority.map(AuthorityState::finish),
            #[cfg(test)]
            trace: ResolutionTrace {
                constructor_prepared,
                overrides_applied,
                after_update,
                explicit_replay,
                final_resolved,
            },
            #[cfg(test)]
            work,
        })
    }

    pub(super) fn materialize_into(self, config: &mut SourcePresentationConfig) {
        config.set_value_preserving_theme_compatibility(
            "themeVariables",
            Value::Object(self.variables),
        );

        match self.authority {
            Some(authority) => {
                for (target, owners) in authority.dependencies {
                    for owner in owners.0 {
                        let source = &authority.paths[owner];

                        if source != &target {
                            config.propagate_theme_variable_ownership(source, &target);
                        }
                    }
                }
            },
            ::core::option::Option::None => {},
        }
    }

    #[cfg(test)]
    pub(super) const fn trace(&self) -> &ResolutionTrace {
        &self.trace
    }
}

#[must_use]
fn missing_value_error(key: &str) -> ColorError {
    ColorError::UnsupportedFormat {
        input: format!("missing or non-color theme value `{key}`"),
    }
}

fn transform_color(
    computed: ComputedValue,
    transform: impl FnOnce(&str) -> Result<String, ColorError>,
) -> Result<ComputedValue, ColorError> {
    let input = computed
        .value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ColorError::UnsupportedFormat {
            input: computed.value.to_string(),
        })?;
    Ok(ComputedValue::with_owners(
        Value::String({ let checked_value = transform(input)?; checked_value }),
        computed.owners,
    ))
}

fn copy(stage: &ThemeState, key: &str) -> Result<ComputedValue, ColorError> {
    stage.computed_from(key)
}

fn first_truthy(stage: &ThemeState, keys: &[&str]) -> Result<ComputedValue, ColorError> {
    let (last, candidates) = {
        let optional_last = keys.split_last();

        match optional_last {
            Some((matched_last, matched_candidates)) => (matched_last, matched_candidates),
            ::core::option::Option::None => return Err(missing_value_error("empty JavaScript OR chain")),
        }
    };

    if let Some(value) = candidates.iter().find_map(|key| stage.truthy_from(key)) {
        Ok(value)
    } else {
        stage.computed_from(last)
    }
}

fn for_theme_color_indices(
    stage: &mut ThemeState,
    mut execute: impl FnMut(&mut ThemeState, usize) -> Result<(), ColorError>,
) -> Result<(), ColorError> {
    for index in 0..stage.theme_color_iterations {
        execute(stage, index)?;
    }

    Ok(())
}

fn theme_color_iteration_count(stage: &ThemeState) -> Result<usize, ThemeEvaluationLimitExceeded> {
    merman_model::presentation::theme_color_iterations(stage.variables.get("THEME_COLOR_LIMIT"))
}

fn adjusted(
    stage: &ThemeState,
    key: &str,
    adjustment: ColorAdjustment,
) -> Result<ComputedValue, ColorError> {
    transform_color({ let checked_value = copy(stage, key)?; checked_value }, |color| {
        theme_color::adjust(color, adjustment)
    })
}

fn lightened(stage: &ThemeState, key: &str, amount: f64) -> Result<ComputedValue, ColorError> {
    transform_color({ let checked_value = copy(stage, key)?; checked_value }, |color| {
        theme_color::lighten(color, amount)
    })
}

fn darkened(stage: &ThemeState, key: &str, amount: f64) -> Result<ComputedValue, ColorError> {
    transform_color({ let checked_value = copy(stage, key)?; checked_value }, |color| {
        theme_color::darken(color, amount)
    })
}

fn inverted(stage: &ThemeState, key: &str) -> Result<ComputedValue, ColorError> {
    transform_color({ let checked_value = copy(stage, key)?; checked_value }, theme_color::invert)
}

struct ObjectBuilder<'a> {
    stage: &'a ThemeState<'a>,
    source_key: &'a str,
    object: Map<String, Value>,
    owners: BTreeMap<String, Owners>,
}

impl<'a> ObjectBuilder<'a> {
    #[must_use]
    fn new(stage: &'a ThemeState<'a>, source_key: &'a str) -> Self {
        Self {
            stage,
            source_key,
            object: Map::new(),
            owners: BTreeMap::new(),
        }
    }

    fn truthy_or(
        &mut self,
        field: &str,
        fallback: impl FnOnce(&ThemeState) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        let current = self
            .stage
            .variables
            .get(self.source_key)
            .and_then(Value::as_object)
            .and_then(|object| object.get(field))
            .filter(|value| is_js_truthy(value))
            .cloned();
        let computed = match current {
            Some(value) => {
                let owners = self
                    .stage
                    .authority
                    .as_ref()
                    .map_or_else(Owners::default, |authority| {
                        authority.read(&format!("{}.{}", self.source_key, field))
                    });
                ComputedValue::with_owners(value, owners)
            },
            ::core::option::Option::None => {
                fallback(self.stage)?
            },
        };
        self.object.insert(field.to_string(), computed.value);

        if !computed.owners.is_empty() {
            self.owners.insert(field.to_string(), computed.owners);
        }

        Ok(())
    }

    fn field(&mut self, field: &str, computed: ComputedValue) {
        self.object.insert(field.to_string(), computed.value);

        if !computed.owners.is_empty() {
            self.owners.insert(field.to_string(), computed.owners);
        }
    }

    #[must_use]
    fn finish(self) -> ComputedValue {
        ComputedValue::object(self.object, self.owners)
    }
}

fn update_journey(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, source, hue) in [
        ("fillType0", "primaryColor", 0.0),
        ("fillType1", "secondaryColor", 0.0),
        ("fillType2", "primaryColor", 64.0),
        ("fillType3", "secondaryColor", 64.0),
        ("fillType4", "primaryColor", -64.0),
        ("fillType5", "secondaryColor", -64.0),
        ("fillType6", "primaryColor", 128.0),
        ("fillType7", "secondaryColor", 128.0),
    ] {
        stage.assign(target, |stage_2| {
            if ::core::matches!(::core::primitive::f64::classify(hue), ::core::num::FpCategory::Zero) {
                copy(stage_2, source)
            } else {
                adjusted(stage_2, source, ColorAdjustment::hsl(hue, 0.0, 0.0))
            }
        })?;
    }

    Ok(())
}

fn update_quadrant(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("quadrant1Fill", |stage_2| copy(stage_2, "primaryColor"))?;

    for (target, amount) in [
        ("quadrant2Fill", 5.0),
        ("quadrant3Fill", 10.0),
        ("quadrant4Fill", 15.0),
    ] {
        stage.assign_if_falsy(target, |stage_3| {
            adjusted(
                stage_3,
                "primaryColor",
                ColorAdjustment::rgb(amount, amount, amount),
            )
        })?;
    }

    stage.assign_if_falsy("quadrant1TextFill", |stage_4| copy(stage_4, "primaryTextColor"))?;

    for (target, amount) in [
        ("quadrant2TextFill", -5.0),
        ("quadrant3TextFill", -10.0),
        ("quadrant4TextFill", -15.0),
    ] {
        stage.assign_if_falsy(target, |stage_5| {
            adjusted(
                stage_5,
                "primaryTextColor",
                ColorAdjustment::rgb(amount, amount, amount),
            )
        })?;
    }



    stage.assign("quadrantPointFill", |stage_6| {
        let existing_is_truthy = stage_6
            .variables
            .get("quadrantPointFill")
            .is_some_and(is_js_truthy);
        let quadrant = stage_6.color("quadrant1Fill")?;
        let lighten = existing_is_truthy || theme_color::is_dark(&quadrant)?;
        let value = if lighten {
            theme_color::lighten(&quadrant, f64::NAN)?
        } else {
            theme_color::darken(&quadrant, f64::NAN)?
        };
        let keys = if existing_is_truthy {
            &["quadrantPointFill", "quadrant1Fill"][..]
        } else {
            &["quadrant1Fill"][..]
        };
        Ok(stage_6.computed(Value::String(value), keys))
    })?;

    for target in [
        "quadrantPointTextFill",
        "quadrantXAxisTextFill",
        "quadrantYAxisTextFill",
        "quadrantTitleFill",
    ] {
        stage.assign_if_falsy(target, |stage_7| copy(stage_7, "primaryTextColor"))?;
    }

    for target in [
        "quadrantInternalBorderStrokeFill",
        "quadrantExternalBorderStrokeFill",
    ] {
        stage.assign_if_falsy(target, |stage_8| copy(stage_8, "primaryBorderColor"))?;
    }

    Ok(())
}

fn update_cynefin(
    stage: &mut ThemeState,
    cliff_color: &str,
    complex_background: &str,
    complicated_background: &str,
    chaotic_background: &str,
    clear_background: &str,
    confusion_background: &str,
) -> Result<(), ColorError> {
    stage.assign("cynefin", |stage_2| {
        let mut object = ObjectBuilder::new(stage_2, "cynefin");
        object.truthy_or("domainFontSize", |_| Ok(ComputedValue::number(16)))?;
        object.truthy_or("itemFontSize", |_| Ok(ComputedValue::number(12)))?;
        object.truthy_or("boundaryColor", |stage_3| copy(stage_3, "lineColor"))?;
        object.truthy_or("boundaryWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("cliffColor", |_| Ok(ComputedValue::string(cliff_color)))?;
        object.truthy_or("cliffWidth", |_| Ok(ComputedValue::number(4)))?;
        object.truthy_or("arrowColor", |stage_4| copy(stage_4, "lineColor"))?;
        object.truthy_or("arrowWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("complexBg", |_| {
            Ok(ComputedValue::string(complex_background))
        })?;
        object.truthy_or("complicatedBg", |_| {
            Ok(ComputedValue::string(complicated_background))
        })?;
        object.truthy_or("chaoticBg", |_| {
            Ok(ComputedValue::string(chaotic_background))
        })?;
        object.truthy_or("clearBg", |_| Ok(ComputedValue::string(clear_background)))?;
        object.truthy_or("confusionBg", |_| {
            Ok(ComputedValue::string(confusion_background))
        })?;
        object.truthy_or("textColor", |stage_5| copy(stage_5, "textColor"))?;
        object.truthy_or("labelColor", |stage_6| copy(stage_6, "primaryTextColor"))?;
        Ok(object.finish())
    })
}

fn update_xy_chart(stage: &mut ThemeState, palette: &str) -> Result<(), ColorError> {
    stage.assign("xyChart", |stage_2| {
        let mut object = ObjectBuilder::new(stage_2, "xyChart");
        object.truthy_or("backgroundColor", |stage_3| copy(stage_3, "background"))?;

        for field in [
            "titleColor",
            "dataLabelColor",
            "legendTextColor",
            "xAxisTitleColor",
            "xAxisLabelColor",
            "xAxisTickColor",
            "xAxisLineColor",
            "yAxisTitleColor",
            "yAxisLabelColor",
            "yAxisTickColor",
            "yAxisLineColor",
        ] {
            object.truthy_or(field, |stage_4| copy(stage_4, "primaryTextColor"))?;
        }

        object.truthy_or("plotColorPalette", |_| Ok(ComputedValue::string(palette)))?;
        Ok(object.finish())
    })
}

fn update_radar(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("radar", |stage_2| {
        let mut object = ObjectBuilder::new(stage_2, "radar");
        object.truthy_or("axisColor", |stage_3| copy(stage_3, "lineColor"))?;
        object.truthy_or("axisStrokeWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("axisLabelFontSize", |_| Ok(ComputedValue::number(12)))?;
        object.truthy_or("curveOpacity", |_| {
            Ok(ComputedValue::constant(number_value(0.5)))
        })?;
        object.truthy_or("curveStrokeWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("graticuleColor", |_| Ok(ComputedValue::string("#DEDEDE")))?;
        object.truthy_or("graticuleStrokeWidth", |_| Ok(ComputedValue::number(1)))?;
        object.truthy_or("graticuleOpacity", |_| {
            Ok(ComputedValue::constant(number_value(0.3)))
        })?;
        object.truthy_or("legendBoxSize", |_| Ok(ComputedValue::number(12)))?;
        object.truthy_or("legendFontSize", |_| Ok(ComputedValue::number(12)))?;
        Ok(object.finish())
    })
}

fn update_packet(stage: &mut ThemeState, fill_source: &str) -> Result<(), ColorError> {
    stage.assign("packet", |stage_2| {
        let mut object = ObjectBuilder::new(stage_2, "packet");

        for field in [
            "startByteColor",
            "endByteColor",
            "labelColor",
            "titleColor",
            "blockStrokeColor",
        ] {
            object.field(field, copy(stage_2, "primaryTextColor")?);
        }

        object.field("blockFillColor", { let checked_value_2 = copy(stage_2, fill_source)?; checked_value_2 });
        Ok(object.finish())
    })
}

fn update_wardley(
    stage: &mut ThemeState,
    evolution_color: &str,
    fill_source: &str,
) -> Result<(), ColorError> {
    stage.assign_if_falsy("wardleyEvolutionColor", |_| {
        Ok(ComputedValue::string(evolution_color))
    })?;
    stage.assign("wardley", |stage_2| {
        let mut object = ObjectBuilder::new(stage_2, "wardley");
        object.truthy_or("backgroundColor", |stage_3| copy(stage_3, "background"))?;
        object.truthy_or("axisColor", |stage_4| copy(stage_4, "lineColor"))?;
        object.truthy_or("axisTextColor", |stage_5| copy(stage_5, "primaryTextColor"))?;
        object.truthy_or("gridColor", |stage_6| copy(stage_6, "gridColor"))?;
        object.truthy_or("componentFill", |stage_7| copy(stage_7, fill_source))?;
        object.truthy_or("componentStroke", |stage_8| copy(stage_8, "lineColor"))?;
        object.truthy_or("componentLabelColor", |stage_9| {
            copy(stage_9, "primaryTextColor")
        })?;
        object.truthy_or("linkStroke", |stage_10| copy(stage_10, "lineColor"))?;
        object.truthy_or("evolutionStroke", |stage_11| {
            copy(stage_11, "wardleyEvolutionColor")
        })?;
        object.truthy_or("annotationStroke", |stage_12| copy(stage_12, "lineColor"))?;
        object.truthy_or("annotationTextColor", |stage_13| {
            copy(stage_13, "primaryTextColor")
        })?;
        object.truthy_or("annotationFill", |stage_14| copy(stage_14, fill_source))?;
        Ok(object.finish())
    })
}

fn update_requirement(
    stage: &mut ThemeState,
    relation_background: impl FnOnce(&ThemeState) -> Result<ComputedValue, ColorError>,
) -> Result<(), ColorError> {
    stage.assign_if_falsy("requirementBackground", |stage_2| copy(stage_2, "primaryColor"))?;
    stage.assign_if_falsy("requirementBorderColor", |stage_3| {
        copy(stage_3, "primaryBorderColor")
    })?;
    stage.assign_if_falsy("requirementBorderSize", |_| Ok(ComputedValue::string("1")))?;
    stage.assign_if_falsy("requirementTextColor", |stage_4| {
        copy(stage_4, "primaryTextColor")
    })?;
    stage.assign_if_falsy("relationColor", |stage_5| copy(stage_5, "lineColor"))?;
    stage.assign_if_falsy("relationLabelBackground", relation_background)?;
    stage.assign_if_falsy("relationLabelColor", |stage_6| copy(stage_6, "actorTextColor"))
}

fn update_tags(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("tagLabelColor", |stage_2| copy(stage_2, "primaryTextColor"))?;
    stage.assign_if_falsy("tagLabelBackground", |stage_3| copy(stage_3, "primaryColor"))?;
    stage.assign("tagLabelBorder", |stage_4| {
        if let Some(value) = stage_4.truthy_from("tagBorder") {
            Ok(value)
        } else {
            copy(stage_4, "primaryBorderColor")
        }
    })?;
    stage.assign_if_falsy("tagLabelFontSize", |_| Ok(ComputedValue::string("10px")))?;
    stage.assign_if_falsy("commitLabelColor", |stage_5| {
        copy(stage_5, "secondaryTextColor")
    })?;
    stage.assign_if_falsy("commitLabelBackground", |stage_6| {
        copy(stage_6, "secondaryColor")
    })?;
    stage.assign_if_falsy("commitLabelFontSize", |_| Ok(ComputedValue::string("10px")))
}

fn update_event_modeling_light(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, value) in [
        ("emUiFill", "white"),
        ("emUiStroke", "#dbdada"),
        ("emProcessorFill", "#edb3f6"),
        ("emProcessorStroke", "#b88cbf"),
        ("emReadModelFill", "#d3f1a2"),
        ("emReadModelStroke", "#a3b732"),
        ("emCommandFill", "#bcd6fe"),
        ("emCommandStroke", "#679ac3"),
        ("emEventFill", "#ffb778"),
        ("emEventStroke", "#c19a0f"),
        ("emSwimlaneBackgroundOdd", "rgb(250,250,250)"),
        ("emSwimlaneBackgroundStroke", "rgb(240,240,240)"),
    ] {
        stage.assign_if_falsy(target, |_| Ok(ComputedValue::string(value)))?;
    }

    stage.assign_if_falsy("emArrowhead", |stage_2| copy(stage_2, "lineColor"))?;
    stage.assign_if_falsy("emRelationStroke", |stage_3| copy(stage_3, "lineColor"))?;
    stage.assign_if_falsy("attributeBackgroundColorOdd", |_| {
        Ok(ComputedValue::string("#ffffff"))
    })?;
    stage.assign_if_falsy("attributeBackgroundColorEven", |_| {
        Ok(ComputedValue::string("#f2f2f2"))
    })
}

fn update_pie_text(
    stage: &mut ThemeState,
    title_source: &str,
    legend_source: &str,
) -> Result<(), ColorError> {
    stage.assign_if_falsy("pieTitleTextSize", |_| Ok(ComputedValue::string("25px")))?;
    stage.assign_if_falsy("pieTitleTextColor", |stage_2| copy(stage_2, title_source))?;
    stage.assign_if_falsy("pieSectionTextSize", |_| Ok(ComputedValue::string("17px")))?;
    stage.assign_if_falsy("pieSectionTextColor", |stage_3| copy(stage_3, "textColor"))?;
    stage.assign_if_falsy("pieLegendTextSize", |_| Ok(ComputedValue::string("17px")))?;
    stage.assign_if_falsy("pieLegendTextColor", |stage_4| copy(stage_4, legend_source))?;
    stage.assign_if_falsy("pieStrokeColor", |_| Ok(ComputedValue::string("black")))?;
    stage.assign_if_falsy("pieStrokeWidth", |_| Ok(ComputedValue::string("2px")))?;
    stage.assign_if_falsy("pieOuterStrokeWidth", |_| Ok(ComputedValue::string("2px")))?;
    stage.assign_if_falsy("pieOuterStrokeColor", |_| {
        Ok(ComputedValue::string("black"))
    })?;
    stage.assign_if_falsy("pieOpacity", |_| Ok(ComputedValue::string("0.7")))
}

fn update_venn_from_scales(
    stage: &mut ThemeState,
    lighten_amount: Option<f64>,
) -> Result<(), ColorError> {
    for index in 0..8 {
        let target = format!("venn{}", ::core::primitive::i32::wrapping_add(index, 1));
        let source = format!("cScale{index}");
        stage.assign_if_nullish(&target, |stage_2| {
            match lighten_amount {
                Some(amount) => {
                    lightened(stage_2, &source, amount)
                },
                ::core::option::Option::None => {
                    copy(stage_2, &source)
                },
            }
        })?;
    }

    stage.assign_if_nullish("vennTitleTextColor", |stage_3| copy(stage_3, "titleColor"))?;
    stage.assign_if_nullish("vennSetTextColor", |stage_4| copy(stage_4, "textColor"))
}

fn update_surfaces(
    stage: &mut ThemeState,
    hue: f64,
    saturation: f64,
    surface_lightness: impl Fn(usize) -> f64,
    peer_lightness: impl Fn(usize) -> f64,
) -> Result<(), ColorError> {
    for index in 0..5 {
        let surface = format!("surface{index}");
        stage.assign_if_falsy(&surface, |stage_2| {
            adjusted(
                stage_2,
                "mainBkg",
                ColorAdjustment::hsl(hue, saturation, surface_lightness(index)),
            )
        })?;
        let peer = format!("surfacePeer{index}");
        stage.assign_if_falsy(&peer, |stage_3| {
            adjusted(
                stage_3,
                "mainBkg",
                ColorAdjustment::hsl(hue, saturation, peer_lightness(index)),
            )
        })?;
    }

    Ok(())
}

fn update_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("secondBkg", |stage_2| lightened(stage_2, "mainBkg", 16.0))?;
    stage.assign("lineColor", |stage_3| copy(stage_3, "mainContrastColor"))?;
    stage.assign("arrowheadColor", |stage_4| copy(stage_4, "mainContrastColor"))?;

    for (target, source) in [
        ("nodeBkg", "mainBkg"),
        ("nodeBorder", "border1"),
        ("clusterBkg", "secondBkg"),
        ("clusterBorder", "border2"),
        ("defaultLinkColor", "lineColor"),
    ] {
        stage.assign(target, |stage_5| copy(stage_5, source))?;
    }

    stage.assign("edgeLabelBackground", |stage_6| {
        lightened(stage_6, "labelBackground", 25.0)
    })?;
    stage.assign_if_falsy("flowContainerStroke", |stage_7| {
        copy(stage_7, "secondaryBorderColor")
    })?;

    for (target, source) in [
        ("actorBorder", "border1"),
        ("actorBkg", "mainBkg"),
        ("actorTextColor", "mainContrastColor"),
        ("actorLineColor", "actorBorder"),
        ("signalColor", "mainContrastColor"),
        ("signalTextColor", "mainContrastColor"),
        ("labelBoxBkgColor", "actorBkg"),
        ("labelBoxBorderColor", "actorBorder"),
        ("labelTextColor", "mainContrastColor"),
        ("loopTextColor", "mainContrastColor"),
        ("noteBorderColor", "secondaryBorderColor"),
        ("noteBkgColor", "secondBkg"),
        ("noteTextColor", "secondaryTextColor"),
        ("activationBorderColor", "border1"),
        ("activationBkgColor", "secondBkg"),
    ] {
        stage.assign(target, |stage_8| copy(stage_8, source))?;
    }

    stage.assign_if_falsy("rectBkgColor", |stage_9| copy(stage_9, "tertiaryColor"))?;

    stage.assign("altSectionBkgColor", |stage_10| copy(stage_10, "background"))?;
    stage.assign("taskBkgColor", |stage_11| lightened(stage_11, "mainBkg", 23.0))?;

    for (target, source) in [
        ("taskTextColor", "darkTextColor"),
        ("taskTextLightColor", "mainContrastColor"),
        ("taskTextOutsideColor", "taskTextLightColor"),
        ("gridColor", "mainContrastColor"),
        ("doneTaskBkgColor", "mainContrastColor"),
    ] {
        stage.assign(target, |stage_12| copy(stage_12, source))?;
    }

    stage.assign("taskTextDarkColor", |stage_13| {
        inverted(stage_13, "doneTaskBkgColor")
    })?;
    stage.assign("archEdgeColor", |stage_14| copy(stage_14, "lineColor"))?;
    stage.assign("archEdgeArrowColor", |stage_15| copy(stage_15, "lineColor"))?;

    update_state_dark(stage)?;
    update_journey(stage)?;

    for (index, value) in [
        "#0b0000", "#4d1037", "#3f5258", "#4f2f1b", "#6e0a0a", "#3b0048", "#995a01", "#154706",
        "#161722", "#00296f", "#01629c", "#010029",
    ]
    .into_iter()
    .enumerate()
    {
        let target = format!("cScale{}", index + 1);
        stage.assign_if_falsy(&target, |_| Ok(ComputedValue::string(value)))?;
    }

    stage.assign_if_falsy("cScale0", |stage_16| copy(stage_16, "primaryColor"))?;
    for_theme_color_indices(stage, |stage_17, index| {
        let scale = format!("cScale{index}");
        let inverse = format!("cScaleInv{index}");
        stage_17.assign_if_falsy(&inverse, |stage_18| inverted(stage_18, &scale))
    })?;
    for_theme_color_indices(stage, |stage_19, index| {
        let scale = format!("cScale{index}");
        let peer = format!("cScalePeer{index}");
        stage_19.assign_if_falsy(&peer, |stage_20| lightened(stage_20, &scale, 10.0))
    })?;
    update_surfaces(
        stage,
        30.0,
        -30.0,
        |i| 10.0 - i as f64 * 4.0,
        |i| 7.0 - i as f64 * 4.0,
    )?;

    stage.assign_if_falsy("scaleLabelColor", |stage_21| {
        if stage_21.variables.get("darkMode").is_some_and(is_js_truthy) {
            Ok(stage_21.computed(Value::String("black".to_string()), &["darkMode"]))
        } else {
            copy(stage_21, "labelTextColor").map(|mut computed| {
                computed.inherit(stage_21, "darkMode");
                computed
            })
        }
    })?;
    for_theme_color_indices(stage, |stage_22, index| {
        let target = format!("cScaleLabel{index}");
        stage_22.assign_if_falsy(&target, |stage_23| copy(stage_23, "scaleLabelColor"))
    })?;
    for_theme_color_indices(stage, |stage_24, index| {
        let pie = format!("pie{index}");
        let scale = format!("cScale{index}");
        stage_24.assign(&pie, |stage_25| copy(stage_25, &scale))
    })?;
    update_pie_text(stage, "mainContrastColor", "mainContrastColor")?;
    update_venn_from_scales(stage, Some(30.0))?;
    update_cynefin(
        stage, "#FF6B6B", "#1B5E20", "#0D47A1", "#BF360C", "#F57F17", "#4A148C",
    )?;
    update_quadrant(stage)?;
    update_xy_chart(
        stage,
        "#3498db,#2ecc71,#e74c3c,#f1c40f,#bdc3c7,#ffffff,#34495e,#9b59b6,#1abc9c,#e67e22",
    )?;
    update_packet(stage, "background")?;
    update_radar(stage)?;
    update_wardley(stage, "#ff6b6b", "mainBkg")?;
    stage.assign("classText", |stage_26| copy(stage_26, "primaryTextColor"))?;
    update_requirement(stage, |stage_27| {
        let dark_mode = stage_27.variables.get("darkMode").is_some_and(is_js_truthy);
        let mut computed = if dark_mode {
            darkened(stage_27, "secondaryColor", 30.0)?
        } else {
            copy(stage_27, "secondaryColor")?
        };
        computed.inherit(stage_27, "darkMode");
        Ok(computed)
    })?;
    update_git_dark(stage)?;
    update_tags(stage)?;
    update_event_modeling_dark(stage)?;
    stage.assign_if_falsy("nodeBorder", |_| Ok(ComputedValue::string("#999")))?;
    Ok(())
}

fn update_state_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("transitionColor", |stage_2| copy(stage_2, "lineColor"))?;
    stage.assign_if_falsy("transitionLabelColor", |stage_3| copy(stage_3, "textColor"))?;
    stage.assign_if_falsy("stateLabelColor", |stage_4| {
        first_truthy(stage_4, &["stateBkg", "primaryTextColor"])
    })?;
    stage.assign_if_falsy("stateBkg", |stage_5| copy(stage_5, "mainBkg"))?;
    stage.assign_if_falsy("labelBackgroundColor", |stage_6| copy(stage_6, "stateBkg"))?;
    stage.assign_if_falsy("compositeBackground", |stage_7| {
        first_truthy(stage_7, &["background", "tertiaryColor"])
    })?;
    stage.assign_if_falsy("altBackground", |_| Ok(ComputedValue::string("#555")))?;
    stage.assign_if_falsy("compositeTitleBackground", |stage_8| copy(stage_8, "mainBkg"))?;
    stage.assign_if_falsy("compositeBorder", |stage_9| copy(stage_9, "nodeBorder"))?;
    stage.assign("innerEndBackground", |stage_10| {
        copy(stage_10, "primaryBorderColor")
    })?;
    stage.assign("specialStateColor", |_| {
        Ok(ComputedValue::string("#f4f4f4"))
    })?;
    stage.assign_if_falsy("errorBkgColor", |stage_11| copy(stage_11, "tertiaryColor"))?;
    stage.assign_if_falsy("errorTextColor", |stage_12| copy(stage_12, "tertiaryTextColor"))
}

fn update_git_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("git0", |stage_2| lightened(stage_2, "secondaryColor", 20.0))?;

    for (target, pie, fallback, hue, amount) in [
        ("git1", "pie2", "secondaryColor", 0.0, 20.0),
        ("git2", "pie3", "tertiaryColor", 0.0, 20.0),
        ("git3", "pie4", "primaryColor", -30.0, 20.0),
        ("git4", "pie5", "primaryColor", -60.0, 20.0),
        ("git5", "pie6", "primaryColor", -90.0, 10.0),
        ("git6", "pie7", "primaryColor", 60.0, 10.0),
        ("git7", "pie8", "primaryColor", 120.0, 20.0),
    ] {
        stage.assign(target, |stage_3| {
            let source = if let Some(value) = stage_3.truthy_from(pie) {
                value
            } else if ::core::matches!(::core::primitive::f64::classify(hue), ::core::num::FpCategory::Zero) {
                copy(stage_3, fallback)?
            } else {
                adjusted(stage_3, fallback, ColorAdjustment::hsl(hue, 0.0, 0.0))?
            };
            transform_color(source, |color| theme_color::lighten(color, amount))
        })?;
    }

    for index in 0..8 {
        let git = format!("git{index}");
        let inverse = format!("gitInv{index}");
        stage.assign_if_falsy(&inverse, |stage_4| inverted(stage_4, &git))?;
    }

    for index in 0..8 {
        let target = format!("gitBranchLabel{index}");
        stage.assign_if_falsy(&target, |stage_5| {
            if matches!(index, 0 | 3) {
                inverted(stage_5, "labelTextColor")
            } else {
                copy(stage_5, "labelTextColor")
            }
        })?;
    }

    Ok(())
}

fn update_event_modeling_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, value) in [
        ("emUiFill", "#2d2d2d"),
        ("emUiStroke", "#555"),
        ("emProcessorStroke", "#8a6d8c"),
        ("emReadModelStroke", "#6d8c5c"),
        ("emCommandStroke", "#5c6d8c"),
        ("emEventStroke", "#8c755c"),
    ] {
        stage.assign_if_falsy(target, |_| Ok(ComputedValue::string(value)))?;
    }

    for (target, color) in [
        ("emProcessorFill", "#5a3d5c"),
        ("emReadModelFill", "#3d5a2d"),
        ("emCommandFill", "#2d3d5a"),
        ("emEventFill", "#5a452d"),
    ] {
        stage.assign_if_falsy(target, |_| {
            Ok(ComputedValue::string({ let checked_value = theme_color::lighten(color, 10.0)?; checked_value }))
        })?;
    }

    stage.assign_if_falsy("emSwimlaneBackgroundOdd", |stage_2| {
        lightened(stage_2, "background", 5.0)
    })?;
    stage.assign_if_falsy("emSwimlaneBackgroundStroke", |stage_3| {
        lightened(stage_3, "background", 12.0)
    })?;
    stage.assign_if_falsy("emArrowhead", |stage_4| copy(stage_4, "lineColor"))?;
    stage.assign_if_falsy("emRelationStroke", |stage_5| copy(stage_5, "lineColor"))?;
    stage.assign_if_falsy("attributeBackgroundColorOdd", |stage_6| {
        lightened(stage_6, "background", 12.0)
    })?;
    stage.assign_if_falsy("attributeBackgroundColorEven", |stage_7| {
        lightened(stage_7, "background", 2.0)
    })
}

fn update_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("actorBorder", |stage_2| darkened(stage_2, "mainBkg", 20.0))?;
    stage.assign("actorBkg", |stage_3| copy(stage_3, "mainBkg"))?;
    stage.assign("labelBoxBkgColor", |stage_4| copy(stage_4, "actorBkg"))?;
    stage.assign("labelTextColor", |stage_5| copy(stage_5, "actorTextColor"))?;
    stage.assign("loopTextColor", |stage_6| copy(stage_6, "actorTextColor"))?;
    stage.assign("noteBorderColor", |stage_7| copy(stage_7, "border2"))?;
    stage.assign("noteTextColor", |stage_8| copy(stage_8, "actorTextColor"))?;
    stage.assign("actorLineColor", |stage_9| copy(stage_9, "actorBorder"))?;
    stage.assign_if_falsy("rectBkgColor", |stage_10| copy(stage_10, "tertiaryColor"))?;

    stage.assign_if_falsy("cScale0", |stage_11| copy(stage_11, "primaryColor"))?;
    stage.assign_if_falsy("cScale1", |stage_12| copy(stage_12, "secondaryColor"))?;
    stage.assign_if_falsy("cScale2", |stage_13| copy(stage_13, "tertiaryColor"))?;

    for (index, hue) in [30.0, 60.0, 90.0, 120.0, 150.0, 210.0, 270.0, 300.0, 330.0]
        .into_iter()
        .enumerate()
    {
        let target = format!("cScale{}", index + 3);
        stage.assign_if_falsy(&target, |stage_14| {
            adjusted(stage_14, "primaryColor", ColorAdjustment::hsl(hue, 0.0, 0.0))
        })?;
    }

    stage.assign_if_falsy("cScalePeer1", |stage_15| {
        darkened(stage_15, "secondaryColor", 45.0)
    })?;
    stage.assign_if_falsy("cScalePeer2", |stage_16| {
        darkened(stage_16, "tertiaryColor", 40.0)
    })?;
    for_theme_color_indices(stage, |stage_17, index| {
        let scale = format!("cScale{index}");
        stage_17.assign(&scale, |stage_18| darkened(stage_18, &scale, 10.0))?;
        let peer = format!("cScalePeer{index}");
        stage_17.assign_if_falsy(&peer, |stage_19| darkened(stage_19, &scale, 25.0))
    })?;
    for_theme_color_indices(stage, |stage_20, index| {
        let scale = format!("cScale{index}");
        let inverse = format!("cScaleInv{index}");
        stage_20.assign_if_falsy(&inverse, |stage_21| {
            adjusted(stage_21, &scale, ColorAdjustment::hsl(180.0, 0.0, 0.0))
        })
    })?;
    stage.assign("scaleLabelColor", |stage_22| {
        if stage_22
            .variables
            .get("scaleLabelColor")
            .is_some_and(|value| value.as_str() != Some("calculated") && is_js_truthy(value))
        {
            copy(stage_22, "scaleLabelColor")
        } else {
            copy(stage_22, "labelTextColor")
        }
    })?;
    for_theme_color_indices(stage, |stage_23, index| {
        let target = format!("cScaleLabel{index}");
        stage_23.assign_if_falsy(&target, |stage_24| copy(stage_24, "scaleLabelColor"))
    })?;
    update_surfaces(
        stage,
        30.0,
        -30.0,
        |i| -(5.0 + i as f64 * 5.0),
        |i| -(8.0 + i as f64 * 5.0),
    )?;

    for (target, source) in [
        ("nodeBkg", "mainBkg"),
        ("nodeBorder", "border1"),
        ("clusterBkg", "secondBkg"),
        ("clusterBorder", "border2"),
        ("defaultLinkColor", "lineColor"),
    ] {
        stage.assign(target, |stage_25| copy(stage_25, source))?;
    }

    stage.assign_if_falsy("flowContainerStroke", |stage_26| {
        copy(stage_26, "secondaryBorderColor")
    })?;

    for (target, source) in [
        ("taskBorderColor", "border1"),
        ("taskTextColor", "taskTextLightColor"),
        ("taskTextOutsideColor", "taskTextDarkColor"),
        ("activeTaskBorderColor", "taskBorderColor"),
        ("activeTaskBkgColor", "mainBkg"),
        ("archEdgeColor", "lineColor"),
        ("archEdgeArrowColor", "lineColor"),
    ] {
        stage.assign(target, |stage_27| copy(stage_27, source))?;
    }

    stage.assign_if_falsy("rowOdd", |stage_28| lightened(stage_28, "mainBkg", 75.0))?;
    stage.assign_if_falsy("rowEven", |stage_29| lightened(stage_29, "mainBkg", 20.0))?;
    update_state_forest(stage)?;
    stage.assign("classText", |stage_30| copy(stage_30, "primaryTextColor"))?;
    update_journey(stage)?;
    update_pie_forest(stage)?;
    update_venn_forest(stage)?;
    update_cynefin(
        stage, "#8B4513", "#C8E6C9", "#DCEDC8", "#FFE0B2", "#FFF9C4", "#D7CCC8",
    )?;
    update_quadrant(stage)?;
    update_packet(stage, "mainBkg")?;
    update_radar(stage)?;
    update_wardley(stage, "#dc3545", "background")?;
    update_xy_chart(
        stage,
        "#CDE498,#FF6B6B,#A0D2DB,#D7BDE2,#F0F0F0,#FFC3A0,#7FD8BE,#FF9A8B,#FAF3E0,#FFF176",
    )?;
    update_requirement(stage, |stage_31| copy(stage_31, "edgeLabelBackground"))?;
    update_git_forest(stage)?;
    update_tags(stage)?;
    update_event_modeling_light(stage)
}

fn update_state_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("transitionColor", |stage_2| copy(stage_2, "lineColor"))?;
    stage.assign_if_falsy("transitionLabelColor", |stage_3| copy(stage_3, "textColor"))?;
    stage.assign_if_falsy("stateLabelColor", |stage_4| {
        first_truthy(stage_4, &["stateBkg", "primaryTextColor"])
    })?;
    stage.assign_if_falsy("stateBkg", |stage_5| copy(stage_5, "mainBkg"))?;
    stage.assign_if_falsy("labelBackgroundColor", |stage_6| copy(stage_6, "stateBkg"))?;
    stage.assign_if_falsy("compositeBackground", |stage_7| {
        first_truthy(stage_7, &["background", "tertiaryColor"])
    })?;
    stage.assign_if_falsy("altBackground", |_| Ok(ComputedValue::string("#f0f0f0")))?;
    stage.assign_if_falsy("compositeTitleBackground", |stage_8| copy(stage_8, "mainBkg"))?;
    stage.assign_if_falsy("compositeBorder", |stage_9| copy(stage_9, "nodeBorder"))?;
    stage.assign("innerEndBackground", |stage_10| {
        copy(stage_10, "primaryBorderColor")
    })?;
    stage.assign("specialStateColor", |stage_11| copy(stage_11, "lineColor"))?;
    stage.assign_if_falsy("errorBkgColor", |stage_12| copy(stage_12, "tertiaryColor"))?;
    stage.assign_if_falsy("errorTextColor", |stage_13| copy(stage_13, "tertiaryTextColor"))?;
    Ok(())
}

fn update_pie_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, source, hue, lightness) in [
        ("pie1", "primaryColor", 0.0, 0.0),
        ("pie2", "secondaryColor", 0.0, 0.0),
        ("pie3", "tertiaryColor", 0.0, 0.0),
        ("pie4", "primaryColor", 0.0, -30.0),
        ("pie5", "secondaryColor", 0.0, -30.0),
        ("pie6", "tertiaryColor", 40.0, -40.0),
        ("pie7", "primaryColor", 60.0, -10.0),
        ("pie8", "primaryColor", -60.0, -10.0),
        ("pie9", "primaryColor", 120.0, 0.0),
        ("pie10", "primaryColor", 60.0, -50.0),
        ("pie11", "primaryColor", -60.0, -50.0),
        ("pie12", "primaryColor", 120.0, -50.0),
    ] {
        stage.assign_if_falsy(target, |stage_2| {
            if ::core::matches!(::core::primitive::f64::classify(hue), ::core::num::FpCategory::Zero) && ::core::matches!(::core::primitive::f64::classify(lightness), ::core::num::FpCategory::Zero) {
                copy(stage_2, source)
            } else {
                adjusted(stage_2, source, ColorAdjustment::hsl(hue, 0.0, lightness))
            }
        })?;
    }

    update_pie_text(stage, "taskTextDarkColor", "taskTextDarkColor")
}

fn update_venn_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, source, hue) in [
        ("venn1", "primaryColor", 0.0),
        ("venn2", "secondaryColor", 0.0),
        ("venn3", "tertiaryColor", 0.0),
        ("venn4", "primaryColor", 60.0),
        ("venn5", "primaryColor", -60.0),
        ("venn6", "secondaryColor", 60.0),
        ("venn7", "primaryColor", 120.0),
        ("venn8", "secondaryColor", 120.0),
    ] {
        stage.assign_if_nullish(target, |stage_2| {
            adjusted(stage_2, source, ColorAdjustment::hsl(hue, 0.0, -30.0))
        })?;
    }

    stage.assign_if_nullish("vennTitleTextColor", |stage_3| copy(stage_3, "titleColor"))?;
    stage.assign_if_nullish("vennSetTextColor", |stage_4| copy(stage_4, "textColor"))
}

fn update_git_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (index, source, hue) in [
        (0, "primaryColor", 0.0),
        (1, "secondaryColor", 0.0),
        (2, "tertiaryColor", 0.0),
        (3, "primaryColor", -30.0),
        (4, "primaryColor", -60.0),
        (5, "primaryColor", -90.0),
        (6, "primaryColor", 60.0),
        (7, "primaryColor", 120.0),
    ] {
        let target = format!("git{index}");
        stage.assign_if_falsy(&target, |stage_2| {
            if ::core::matches!(::core::primitive::f64::classify(hue), ::core::num::FpCategory::Zero) {
                copy(stage_2, source)
            } else {
                adjusted(stage_2, source, ColorAdjustment::hsl(hue, 0.0, 0.0))
            }
        })?;
    }

    let dark_mode = stage.variables.get("darkMode").is_some_and(is_js_truthy);

    for index in 0..8 {
        let target = format!("git{index}");
        stage.assign(&target, |stage_3| {
            let mut computed = if dark_mode {
                lightened(stage_3, &target, 25.0)?
            } else {
                darkened(stage_3, &target, 25.0)?
            };
            computed.inherit(stage_3, "darkMode");
            Ok(computed)
        })?;
    }

    for index in 0..8 {
        let target = format!("git{index}");
        let inverse = format!("gitInv{index}");
        stage.assign_if_falsy(&inverse, |stage_4| inverted(stage_4, &target))?;
    }

    for index in 0..8 {
        let target = format!("gitBranchLabel{index}");
        stage.assign_if_falsy(&target, |stage_5| {
            if matches!(index, 0 | 3) {
                inverted(stage_5, "labelTextColor")
            } else {
                copy(stage_5, "labelTextColor")
            }
        })?;
    }

    Ok(())
}

fn update_neutral(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("secondBkg", |stage_2| lightened(stage_2, "contrast", 55.0))?;
    stage.assign("border2", |stage_3| copy(stage_3, "contrast"))?;
    stage.assign("actorBorder", |stage_4| lightened(stage_4, "border1", 23.0))?;

    for (target, source) in [
        ("actorBkg", "mainBkg"),
        ("actorTextColor", "text"),
        ("actorLineColor", "actorBorder"),
        ("signalColor", "text"),
        ("signalTextColor", "text"),
        ("labelBoxBkgColor", "actorBkg"),
        ("labelBoxBorderColor", "actorBorder"),
        ("labelTextColor", "text"),
        ("loopTextColor", "text"),
    ] {
        stage.assign(target, |stage_5| copy(stage_5, source))?;
    }

    stage.assign_if_falsy("rectBkgColor", |stage_6| copy(stage_6, "tertiaryColor"))?;

    for (target, value) in [
        ("noteBorderColor", "#999"),
        ("noteBkgColor", "#666"),
        ("noteTextColor", "#fff"),
    ] {
        stage.assign(target, |_| Ok(ComputedValue::string(value)))?;
    }

    for (index, value) in [
        "#555", "#F4F4F4", "#555", "#BBB", "#777", "#999", "#DDD", "#FFF", "#DDD", "#BBB", "#999",
        "#777",
    ]
    .into_iter()
    .enumerate()
    {
        let target = format!("cScale{index}");
        stage.assign_if_falsy(&target, |_| Ok(ComputedValue::string(value)))?;
    }

    let dark_mode = stage.variables.get("darkMode").is_some_and(is_js_truthy);
    for_theme_color_indices(stage, |stage_7, index| {
        let scale = format!("cScale{index}");
        let inverse = format!("cScaleInv{index}");
        stage_7.assign_if_falsy(&inverse, |stage_8| inverted(stage_8, &scale))
    })?;
    for_theme_color_indices(stage, |stage_9, index| {
        let scale = format!("cScale{index}");
        let peer = format!("cScalePeer{index}");
        stage_9.assign_if_falsy(&peer, |stage_10| {
            let mut computed = if dark_mode {
                lightened(stage_10, &scale, 10.0)?
            } else {
                darkened(stage_10, &scale, 10.0)?
            };
            computed.inherit(stage_10, "darkMode");
            Ok(computed)
        })
    })?;
    stage.assign_if_falsy("scaleLabelColor", |stage_11| {
        if dark_mode {
            Ok(stage_11.computed(Value::String("black".to_string()), &["darkMode"]))
        } else {
            copy(stage_11, "labelTextColor").map(|mut computed| {
                computed.inherit(stage_11, "darkMode");
                computed
            })
        }
    })?;
    stage.assign_if_falsy("cScaleLabel0", |stage_12| copy(stage_12, "cScale1"))?;
    stage.assign_if_falsy("cScaleLabel2", |stage_13| copy(stage_13, "cScale1"))?;
    for_theme_color_indices(stage, |stage_14, index| {
        let target = format!("cScaleLabel{index}");
        stage_14.assign_if_falsy(&target, |stage_15| copy(stage_15, "scaleLabelColor"))
    })?;
    update_surfaces(
        stage,
        0.0,
        0.0,
        |i| -(5.0 + i as f64 * 5.0),
        |i| -(8.0 + i as f64 * 5.0),
    )?;

    for (target, source) in [
        ("nodeBkg", "mainBkg"),
        ("nodeBorder", "border1"),
        ("clusterBkg", "secondBkg"),
        ("clusterBorder", "border2"),
        ("defaultLinkColor", "lineColor"),
        ("titleColor", "text"),
    ] {
        stage.assign(target, |stage_16| copy(stage_16, source))?;
    }

    stage.assign_if_falsy("flowContainerStroke", |stage_17| {
        copy(stage_17, "secondaryBorderColor")
    })?;
    stage.assign("sectionBkgColor", |stage_18| {
        lightened(stage_18, "contrast", 30.0)
    })?;
    stage.assign("sectionBkgColor2", |stage_19| {
        lightened(stage_19, "contrast", 30.0)
    })?;
    stage.assign("taskBorderColor", |stage_20| darkened(stage_20, "contrast", 10.0))?;

    for (target, source) in [
        ("taskBkgColor", "contrast"),
        ("taskTextColor", "taskTextLightColor"),
        ("taskTextDarkColor", "text"),
        ("taskTextOutsideColor", "taskTextDarkColor"),
        ("activeTaskBorderColor", "taskBorderColor"),
        ("activeTaskBkgColor", "mainBkg"),
    ] {
        stage.assign(target, |stage_21| copy(stage_21, source))?;
    }

    stage.assign("gridColor", |stage_22| lightened(stage_22, "border1", 30.0))?;

    for (target, source) in [
        ("doneTaskBkgColor", "done"),
        ("doneTaskBorderColor", "lineColor"),
        ("critBkgColor", "critical"),
    ] {
        stage.assign(target, |stage_23| copy(stage_23, source))?;
    }

    stage.assign("critBorderColor", |stage_24| {
        darkened(stage_24, "critBkgColor", 10.0)
    })?;
    stage.assign("todayLineColor", |stage_25| copy(stage_25, "critBkgColor"))?;
    stage.assign("vertLineColor", |stage_26| copy(stage_26, "critBkgColor"))?;
    stage.assign("archEdgeColor", |stage_27| copy(stage_27, "lineColor"))?;
    stage.assign("archEdgeArrowColor", |stage_28| copy(stage_28, "lineColor"))?;

    update_state_neutral(stage)?;
    stage.assign("classText", |stage_29| copy(stage_29, "primaryTextColor"))?;
    update_journey(stage)?;
    for_theme_color_indices(stage, |stage_30, index| {
        let pie = format!("pie{index}");
        let scale = format!("cScale{index}");
        stage_30.assign(&pie, |stage_31| copy(stage_31, &scale))
    })?;
    stage.assign("pie12", |stage_32| copy(stage_32, "pie0"))?;
    update_pie_text(stage, "taskTextDarkColor", "taskTextDarkColor")?;
    update_venn_from_scales(stage, None)?;
    update_cynefin(
        stage, "#8B0000", "#E8F5E9", "#E3F2FD", "#FBE9E7", "#FFF8E1", "#F3E5F5",
    )?;
    update_quadrant(stage)?;
    update_xy_chart(
        stage,
        "#EEE,#6BB8E4,#8ACB88,#C7ACD6,#E8DCC2,#FFB2A8,#FFF380,#7E8D91,#FFD8B1,#FAF3E0",
    )?;
    update_radar(stage)?;
    update_wardley(stage, "#dc3545", "background")?;
    update_requirement(stage, |stage_33| copy(stage_33, "edgeLabelBackground"))?;
    update_git_neutral(stage)?;
    update_tags(stage)?;
    update_event_modeling_light(stage)
}

fn update_state_neutral(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("transitionColor", |_| Ok(ComputedValue::string("#000")))?;
    stage.assign_if_falsy("transitionLabelColor", |stage_2| copy(stage_2, "textColor"))?;
    stage.assign_if_falsy("stateLabelColor", |stage_3| {
        first_truthy(stage_3, &["stateBkg", "primaryTextColor"])
    })?;
    stage.assign_if_falsy("stateBkg", |stage_4| copy(stage_4, "mainBkg"))?;
    stage.assign_if_falsy("labelBackgroundColor", |stage_5| copy(stage_5, "stateBkg"))?;
    stage.assign_if_falsy("compositeBackground", |stage_6| {
        first_truthy(stage_6, &["background", "tertiaryColor"])
    })?;
    stage.assign_if_falsy("altBackground", |_| Ok(ComputedValue::string("#f4f4f4")))?;
    stage.assign_if_falsy("compositeTitleBackground", |stage_7| copy(stage_7, "mainBkg"))?;
    stage.assign_if_falsy("stateBorder", |_| Ok(ComputedValue::string("#000")))?;
    stage.assign("innerEndBackground", |stage_8| {
        copy(stage_8, "primaryBorderColor")
    })?;
    stage.assign("specialStateColor", |_| Ok(ComputedValue::string("#222")))?;
    stage.assign_if_falsy("errorBkgColor", |stage_9| copy(stage_9, "tertiaryColor"))?;
    stage.assign_if_falsy("errorTextColor", |stage_10| copy(stage_10, "tertiaryTextColor"))
}

fn update_git_neutral(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("git0", |stage_2| darkened(stage_2, "pie1", 25.0))?;

    for (target, pie, source, hue) in [
        ("git1", "pie2", "secondaryColor", 0.0),
        ("git2", "pie3", "tertiaryColor", 0.0),
        ("git3", "pie4", "primaryColor", -30.0),
        ("git4", "pie5", "primaryColor", -60.0),
        ("git5", "pie6", "primaryColor", -90.0),
        ("git6", "pie7", "primaryColor", 60.0),
        ("git7", "pie8", "primaryColor", 120.0),
    ] {
        stage.assign(target, |stage_3| {
            if let Some(value) = stage_3.truthy_from(pie) {
                Ok(value)
            } else if ::core::matches!(::core::primitive::f64::classify(hue), ::core::num::FpCategory::Zero) {
                copy(stage_3, source)
            } else {
                adjusted(stage_3, source, ColorAdjustment::hsl(hue, 0.0, 0.0))
            }
        })?;
    }

    for index in 0..8 {
        let git = format!("git{index}");
        let inverse = format!("gitInv{index}");
        stage.assign_if_falsy(&inverse, |stage_4| inverted(stage_4, &git))?;
    }

    stage.assign_if_falsy("branchLabelColor", |stage_5| copy(stage_5, "labelTextColor"))?;

    for index in 0..8 {
        let target = format!("gitBranchLabel{index}");

        if matches!(index, 1 | 3) {
            stage.assign(&target, |_| Ok(ComputedValue::string("white")))?;
        } else {
            stage.assign(&target, |stage_6| copy(stage_6, "branchLabelColor"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shallow_replay_preserves_sibling_dependencies_and_replaces_nested_children() {
        let empty_replay = json!({"chart": {}});
        let explicit = json!({"source": "#123456", "chart": {"nested": {"new": 2}}});
        let explicit = explicit.as_object().unwrap();
        let mut state =
            ThemeState::constructor(Map::new(), explicit, Some(OwnerSource::AllExplicit));
        state.overlay_explicit(explicit);
        state.commit_computed(
            "chart",
            ComputedValue::object(
                json!({"retained": "#123456", "nested": {"old": 1}})
                    .as_object()
                    .unwrap()
                    .clone(),
                BTreeMap::from([
                    ("retained".to_string(), state.owners("source")),
                    ("nested.old".to_string(), state.owners("source")),
                ]),
            ),
        );
        state.replay_explicit(explicit);
        assert_eq!(
            state.variables["chart"],
            json!({"retained": "#123456", "nested": {"new": 2}})
        );
        assert!(state.depends_on("chart.retained", "source"));
        assert!(!state.depends_on("chart.nested.old", "source"));
        assert!(state.depends_on("chart.nested.new", "chart.nested.new"));
        state.replay_explicit(empty_replay.as_object().unwrap());
        assert!(state.depends_on("chart.retained", "source"));
        assert!(!state.depends_on("chart", "chart"));
        let scalar_explicit = json!({"chart": false});
        let scalar_explicit = scalar_explicit.as_object().unwrap();
        let mut scalar_state = ThemeState::constructor(
            state.variables.clone(),
            scalar_explicit,
            Some(OwnerSource::AllExplicit),
        );
        scalar_state.overlay_explicit(scalar_explicit);
        scalar_state.replay_explicit(scalar_explicit);
        assert_eq!(scalar_state.variables["chart"], json!(false));
        assert!(!scalar_state.depends_on("chart.retained", "source"));
        assert!(scalar_state.depends_on("chart", "chart"));
    }

    #[test]
    fn javascript_or_chain_does_not_evaluate_an_unselected_last_operand() {
        let mut variables = Map::new();
        variables.insert("selected".to_string(), json!("value"));
        let explicit = variables.clone();
        let mut state =
            ThemeState::constructor(variables, &explicit, Some(OwnerSource::AllExplicit));
        state.overlay_explicit(&explicit);

        let computed = first_truthy(&state, &["selected", "missing"]).unwrap();

        assert_eq!(computed.value, json!("value"));
        assert_eq!(computed.owners, state.owners("selected"));
        assert!(
            !state
                .authority
                .as_ref()
                .unwrap()
                .sources
                .borrow()
                .seeds
                .contains_key("missing")
        );
    }

    /// Reference only the old authority bookkeeping, not a second theme evaluator. The value
    /// assignments and their read operands are shared by both sides of this differential test.
    struct EagerAuthority {
        dependencies: BTreeMap<String, BTreeSet<String>>,
    }

    impl EagerAuthority {
        fn new(explicit: &Map<String, Value>) -> Self {
            let mut oracle = Self {
                dependencies: BTreeMap::new(),
            };
            for (key, value) in explicit {
                oracle.authored(key, value);
            }
            oracle
        }

        fn remove_subtree(&mut self, root: &str) {
            let prefix = format!("{root}.");
            self.dependencies
                .retain(|path, _| path != root && !path.starts_with(&prefix));
        }

        fn authored(&mut self, root: &str, value: &Value) {
            self.remove_subtree(root);
            for path in explicit_dependency_leaf_paths(root, value) {
                self.dependencies
                    .insert(path.clone(), BTreeSet::from([path]));
            }
        }

        fn replay(&mut self, explicit: &Map<String, Value>, before: &Map<String, Value>) {
            for (key, value) in explicit {
                if before.get(key).is_some_and(Value::is_object)
                    && let Value::Object(children) = value
                {
                    for (child, value) in children {
                        self.authored(&format!("{key}.{child}"), value);
                    }
                } else {
                    self.authored(key, value);
                }
            }
        }

        fn commit(&mut self, root: &str, nested: &[(&str, &[&str])]) {
            let sources = nested
                .iter()
                .map(|(path, reads)| {
                    let sources = reads
                        .iter()
                        .filter_map(|read| self.dependencies.get(*read))
                        .flat_map(|sources| sources.iter().cloned())
                        .collect::<BTreeSet<_>>();
                    ((*path).to_string(), sources)
                })
                .collect::<Vec<_>>();
            self.remove_subtree(root);
            for (path, sources) in sources {
                if !sources.is_empty() {
                    let path = if path.is_empty() {
                        root.to_string()
                    } else {
                        format!("{root}.{path}")
                    };
                    self.dependencies.insert(path, sources);
                }
            }
        }
    }

    #[test]
    fn lazy_authority_matches_eager_bookkeeping_across_replacement_and_replay() {
        for authored in [
            json!(false),
            json!(0),
            Value::Null,
            json!({}),
            json!("#123456"),
        ] {
            let explicit = json!({
                "source": "#123456", "other": "#123456", "authored": authored,
                "chart": {"nested": {"new": 2}, "empty": {}, "same": "same"},
                "chart.literal": 1, "chart.nested": {"shadow": 3},
                "chart.same": {"shadow": 4}
            });
            let explicit = explicit.as_object().unwrap();
            let mut state =
                ThemeState::constructor(Map::new(), explicit, Some(OwnerSource::AllExplicit));
            state.overlay_explicit(explicit);
            let mut oracle = EagerAuthority::new(explicit);
            assert_eq!(state.snapshot().dependencies, oracle.dependencies);

            let computed = state.computed(json!("#123456"), &["source"]);
            state.commit_computed("authored", computed);
            oracle.commit("authored", &[("", &["source"])]);
            assert_eq!(state.snapshot().dependencies, oracle.dependencies);

            let nested = [
                ("retained", &["source"][..]),
                ("nested.old", &["authored"][..]),
            ];
            let nested_owners = nested
                .iter()
                .map(|(path, reads)| {
                    (
                        (*path).to_string(),
                        state.computed(Value::Null, reads).owners,
                    )
                })
                .collect();
            state.commit_computed(
                "chart",
                ComputedValue::object(
                    json!({"retained": "#123456", "nested": {"old": 1}})
                        .as_object()
                        .unwrap()
                        .clone(),
                    nested_owners,
                ),
            );
            oracle.commit("chart", &nested);
            assert_eq!(state.snapshot().dependencies, oracle.dependencies);


            state.commit_computed("source", ComputedValue::string("#123456"));
            oracle.commit("source", &[]);
            let computed = state.computed(json!("#123456"), &["source", "other"]);
            state.commit_computed("combined", computed);
            oracle.commit("combined", &[("", &["source", "other"])]);
            assert_eq!(state.snapshot().dependencies, oracle.dependencies);

            let before_replay = state.variables.clone();
            state.replay_explicit(explicit);
            oracle.replay(explicit, &before_replay);
            assert_eq!(state.snapshot().dependencies, oracle.dependencies);
            assert!(state.depends_on("chart.retained", "source"));
            assert!(!state.depends_on("chart.nested.old", "source"));
        }
    }

    #[test]
    fn replay_barriers_match_eager_authority_without_computed_writes() {
        for ancestor in [
            json!(false),
            json!(0),
            Value::Null,
            json!([]),
            json!({}),
            json!({"b": false}),
        ] {
            for descendant in [
                json!(false),
                Value::Null,
                json!([]),
                json!({}),
                json!({"c": {}}),
            ] {
                let explicit = json!({
                    "a": ancestor, "a-ignored": 1, "a.b": descendant, "a.b.c": {}
                });
                let explicit = explicit.as_object().unwrap();
                let mut state =
                    ThemeState::constructor(Map::new(), explicit, Some(OwnerSource::AllExplicit));
                state.overlay_explicit(explicit);
                let mut oracle = EagerAuthority::new(explicit);
                assert_eq!(state.snapshot().dependencies, oracle.dependencies);
                let before = state.variables.clone();
                state.replay_explicit(explicit);
                oracle.replay(explicit, &before);
                assert_eq!(
                    state.snapshot().dependencies,
                    oracle.dependencies,
                    "{explicit:?}"
                );
            }
        }

        let explicit = json!({"a": false, "a.b": {}});
        let explicit = explicit.as_object().unwrap();
        let mut state =
            ThemeState::constructor(Map::new(), explicit, Some(OwnerSource::AllExplicit));
        state.overlay_explicit(explicit);
        assert!(state.depends_on("a.b", "a.b"));
        state.replay_explicit(explicit);
        assert!(state.depends_on("a", "a"));
        assert!(!state.depends_on("a.b", "a.b"));
    }

    #[test]
    fn literal_root_insertion_order_matches_eager_overlay_and_replay() {
        let orders = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for values in [
            [json!(false), json!(true), json!(false)],
            [json!(false), json!({}), json!({})],
            [json!({}), json!(true), json!({"d": 1})],
            [json!({"c": 1}), json!(true), json!({})],
            [json!({"b": true}), json!({}), json!(false)],
            [
                json!({"b": {"c": {"d": 1}}, "other": 1}),
                json!({"c": false}),
                json!({"d": true}),
            ],
            [json!({"b.c": true}), json!({"c": {}}), json!({"d": 1})],
        ] {
            let keys = ["a", "a.b", "a.b.c"];
            for order in orders {


                let explicit = order
                    .map(|index| (keys[index].to_string(), values[index].clone()))
                    .into_iter()
                    .collect::<Map<_, _>>();
                assert_eq!(
                    explicit.keys().map(String::as_str).collect::<Vec<_>>(),
                    order.map(|index| keys[index])
                );
                let mut state =
                    ThemeState::constructor(Map::new(), &explicit, Some(OwnerSource::AllExplicit));
                state.overlay_explicit(&explicit);
                let mut oracle = EagerAuthority::new(&explicit);
                assert_eq!(
                    state.snapshot().dependencies,
                    oracle.dependencies,
                    "overlay: {explicit:?}"
                );


                let reads = ["a", "a.b", "a.b.c", "a.b.c.d", "a.other"];
                let computed = state.computed(Value::Null, &reads);
                state.commit_computed("derived", computed);
                oracle.commit("derived", &[("", &reads)]);
                assert_eq!(
                    state.snapshot().dependencies,
                    oracle.dependencies,
                    "first read: {explicit:?}"
                );

                let before = state.variables.clone();
                state.replay_explicit(&explicit);
                oracle.replay(&explicit, &before);
                assert_eq!(
                    state.snapshot().dependencies,
                    oracle.dependencies,
                    "replay: {explicit:?}"
                );
                let computed = state.computed(Value::Null, &reads);
                state.commit_computed("after_replay", computed);
                oracle.commit("after_replay", &[("", &reads)]);
                assert_eq!(
                    state.snapshot().dependencies,
                    oracle.dependencies,
                    "read after replay: {explicit:?}"
                );
            }
        }
    }

    #[test]
    fn overlay_order_indexes_only_overlapping_literal_roots() {
        for unrelated in [0, 16, 256] {
            let mut explicit = (0..unrelated)
                .map(|index| (format!("unrelated{index}.leaf"), json!(true)))
                .collect::<Map<_, _>>();
            explicit.insert("a.b".to_string(), json!(true));
            explicit.insert("a".to_string(), json!(false));
            let authority = AuthorityState::new(&explicit, OwnerSource::AllExplicit);
            assert_eq!(
                authority.overlay_order,
                BTreeMap::from([("a.b", unrelated), ("a", unrelated + 1)])
            );
            assert!(!authority.explicit_has_leaf_path("a.b"));
            assert!(authority.sources.borrow().seeds.is_empty());
            assert!(authority.replacements.is_empty());
        }
    }

    fn staged_programs() -> [(StagedProgram, super::super::MermaidThemeId); 4] {
        use super::super::MermaidThemeId;
        [
            (StagedProgram::Base, MermaidThemeId::Base),
            (StagedProgram::Dark, MermaidThemeId::Dark),
            (StagedProgram::Forest, MermaidThemeId::Forest),
            (StagedProgram::Neutral, MermaidThemeId::Neutral),
        ]
    }

    #[test]
    fn prepared_empty_results_preserve_executor_order_and_are_isolated_from_materialization() {
        use super::super::ThemeProgram;

        for (program, theme) in staged_programs() {
            let expected = Resolution::execute_impl(
                program,
                ThemeProgram::resolve(theme).prepared_constructor(),
                &Map::new(),
                None,
            )
            .unwrap();
            let prepared = program.resolved_without_overrides().unwrap();
            let expected_bytes = serde_json::to_vec(&expected.variables).unwrap();
            assert_eq!(
                serde_json::to_vec(prepared).unwrap(),
                expected_bytes,
                "{theme}: serialization includes nested object key order"
            );

            let mut materialized = prepared.clone();
            materialized
                .get_mut("radar")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("axisColor".to_string(), json!("#123456"));
            materialized.insert("hostOnly".to_string(), json!(true));
            let reused = program.resolved_without_overrides().unwrap();
            assert!(std::ptr::eq(prepared, reused));
            assert_eq!(serde_json::to_vec(reused).unwrap(), expected_bytes);
        }
    }

    #[test]
    fn empty_staged_materialization_preserves_config_bytes_and_compatibility() {
        use super::super::{ThemeProgram, apply_theme_defaults};
        use merman_model::presentation::ThemeCompatibilityRecipe;

        for (program, theme) in staged_programs() {
            let theme_program = ThemeProgram::resolve(theme);
            let mut inputs = Vec::new();
            inputs.push(SourcePresentationConfig::from_value(
                json!({"theme": theme.as_str()}),
            ));
            for raw in [
                json!({}),
                json!({"nodeBkg": null}),
                Value::Null,
                json!(false),
            ] {
                inputs.push(SourcePresentationConfig::from_value(json!({
                    "theme": theme.as_str(), "themeVariables": raw,
                })));
            }
            let binding = ThemeCompatibilityRecipe::try_new(
                [0x79; 32],
                MermaidConfig::from_value(json!({"theme": theme.as_str()})),
            )
            .unwrap();
            inputs.push(SourcePresentationConfig::from_theme_compatibility_recipe(
                binding,
            ));
            let binding = ThemeCompatibilityRecipe::try_new(
                [0x7a; 32],
                MermaidConfig::from_value(json!({
                    "theme": theme.as_str(), "themeVariables": {"nodeBkg": "#123456"},
                })),
            )
            .unwrap();
            let mut normalized_owner =
                SourcePresentationConfig::from_theme_compatibility_recipe(binding);
            normalized_owner
                .set_value_preserving_theme_compatibility("themeVariables.nodeBkg", Value::Null);
            assert!(normalized_owner.has_theme_variable_calculation_owners());
            inputs.push(normalized_owner);
            let mut explicit_owner =
                SourcePresentationConfig::from_value(json!({"theme": theme.as_str()}));
            explicit_owner.set_value_explicit("themeVariables", json!({}));
            assert!(explicit_owner.has_theme_variable_calculation_owners());
            inputs.push(explicit_owner);

            for mut actual in inputs {
                let mut expected = actual.clone();
                let explicit = theme_program
                    .normalize_overrides(super::super::theme_variables_map(expected.as_config()));
                assert!(explicit.is_empty());
                let resolution = Resolution::execute_with_owners(
                    program,
                    theme_program.prepared_constructor(),
                    &explicit,
                    &expected,
                )
                .unwrap();
                expected.retain_normalized_theme_compatibility_variables(&explicit);
                resolution.materialize_into(&mut expected);

                apply_theme_defaults(&mut actual).unwrap();
                assert_eq!(
                    serde_json::to_vec(actual.as_value()).unwrap(),
                    serde_json::to_vec(expected.as_value()).unwrap(),
                    "{theme}"
                );
                actual.freeze_theme_compatibility();
                expected.freeze_theme_compatibility();
                assert_eq!(
                    actual.mermaid_compatibility_fields(),
                    expected.mermaid_compatibility_fields(),
                    "{theme}"
                );
                assert_eq!(
                    actual.explicit_config_owns_path("themeVariables"),
                    expected.explicit_config_owns_path("themeVariables"),
                    "{theme}"
                );
            }
        }
    }

    #[test]
    fn custom_detector_observes_staged_executor_key_order() {
        use super::super::{MermaidThemeId, ThemeProgram};

        fn detector(_text: &str, config: &mut SourcePresentationConfig) -> bool {
            let theme = MermaidThemeId::parse(config.get_str("theme").unwrap()).unwrap();
            let theme_program = ThemeProgram::resolve(theme);
            let expected = Resolution::execute_impl(
                theme_program.staged_program().unwrap(),
                theme_program.prepared_constructor(),
                &Map::new(),
                None,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_vec(&config.as_value()["themeVariables"]).unwrap(),
                serde_json::to_vec(&expected.variables).unwrap(),
                "{theme}: a detector can observe recursive insertion order"
            );
            true
        }

        for (program, theme) in staged_programs() {
            let mut engine = crate::MermaidEngine::new().with_site_config(
                SourcePresentationConfig::from_value(json!({
                    "theme": theme.as_str(),
                })),
            );
            *engine.registry_mut() = crate::DetectorRegistry::new();
            engine.registry_mut().add_fn("flowchart-v2", detector);
            let metadata = engine.parse_metadata_sync("staged-order-probe").unwrap();
            assert_eq!(metadata.diagram_type, "flowchart-v2");
            assert_eq!(
                serde_json::to_vec(&metadata.effective_config.as_value()["themeVariables"])
                    .unwrap(),
                serde_json::to_vec(program.resolved_without_overrides().unwrap()).unwrap(),
                "{theme}: metadata retains the observed order"
            );
        }
    }

    #[test]
    fn unread_authored_scalars_do_not_seed_authority_at_any_scale() {
        use super::super::ThemeProgram;
        for (program, theme) in staged_programs() {
            let constructor = ThemeProgram::resolve(theme).prepared_constructor();
            let mut previous: Option<(usize, AuthorityWork)> = None;
            for count in [0, 64, 256, 1024, 4096] {
                let explicit = (0..count)
                    .map(|index| (format!("z{index:08}"), json!(1)))
                    .collect::<Map<_, _>>();
                let mut config = SourcePresentationConfig::from_value(json!({}));
                config.deep_merge_explicit(&json!({"themeVariables": explicit}));
                assert!(
                    AuthorityState::new(&explicit, OwnerSource::Config(&config))
                        .overlay_order
                        .is_empty()
                );
                let resolved =
                    Resolution::execute_with_owners(program, constructor, &explicit, &config)
                        .unwrap();
                assert_eq!(
                    resolved.variables.get("z00000000"),
                    (count > 0).then_some(&json!(1))
                );
                if count > 0 {
                    let authority = resolved.authority.as_ref().unwrap();
                    assert!(
                        authority.paths.is_empty(),
                        "{program:?}/{count}: unread keys are not source identities"
                    );
                    assert!(
                        authority.dependencies.is_empty(),
                        "{program:?}/{count}: no derived owners exist"
                    );
                    assert_eq!(resolved.work.source_queries, 0);
                    if let Some((previous_count, previous_work)) = previous {
                        assert_eq!(resolved.work.reads, previous_work.reads);
                        assert_eq!(
                            resolved.work.removed_replacements,
                            previous_work.removed_replacements
                        );
                        assert_eq!(
                            resolved.work.replacement_probes - previous_work.replacement_probes,
                            count - previous_count
                        );
                    }
                    previous = Some((count, resolved.work));
                } else {
                    assert!(
                        resolved.authority.is_none(),
                        "an owner-free execution stores no authority state"
                    );
                    assert_eq!(resolved.work, AuthorityWork::default());
                }
            }
        }
    }

    #[test]
    fn equal_values_with_different_owners_keep_distinct_authority() {
        use super::super::{MermaidThemeId, ThemeProgram};
        let constructor = ThemeProgram::resolve(MermaidThemeId::Base).prepared_constructor();
        let explicit = json!({"primaryColor": "#123456", "secondaryColor": "#123456"});
        let explicit = explicit.as_object().unwrap();
        let mut resolved_values = Vec::new();
        for (owned, should_own_main, should_own_activation) in [
            ("primaryColor", true, false),
            ("secondaryColor", false, true),
        ] {
            let mut config =
                SourcePresentationConfig::from_value(json!({"themeVariables": explicit}));
            config.deep_merge_explicit(&json!({"themeVariables": {(owned): "#123456"}}));
            let resolved = Resolution::execute_with_owners(
                StagedProgram::Base,
                constructor,
                explicit,
                &config,
            )
            .unwrap();
            resolved_values.push(resolved.variables.clone());
            resolved.materialize_into(&mut config);
            assert_eq!(
                config.config_path_overrides_typed_default("themeVariables.mainBkg"),
                should_own_main
            );
            assert_eq!(
                config.config_path_overrides_typed_default("themeVariables.activationBkgColor"),
                should_own_activation
            );
        }
        assert_eq!(resolved_values[0], resolved_values[1]);
    }

    #[test]
    fn ownership_mode_preserves_values_errors_and_color_limit() {
        use super::super::ThemeProgram;
        for (program, theme) in staged_programs() {
            let constructor = ThemeProgram::resolve(theme).prepared_constructor();
            for overrides in [
                json!({}),
                json!({"primaryColor": "#123456", "radar": {"axisColor": null}, "venn1": 0, "darkMode": false}),
                json!({"primaryColor": "not-a-color", "background": "also-invalid"}),
                json!({"THEME_COLOR_LIMIT": 64}),
                json!({"THEME_COLOR_LIMIT": 65}),
            ] {
                let explicit = overrides.as_object().unwrap();
                let owner_free = Resolution::execute_impl(program, constructor, explicit, None);
                let owner_bearing = Resolution::execute(program, constructor, explicit);
                match (owner_free, owner_bearing) {
                    (Ok(left), Ok(right)) => {
                        assert_eq!(left.variables, right.variables, "{program:?}/{overrides}");
                        assert!(left.authority.is_none());
                        assert_eq!(left.work, AuthorityWork::default());
                    }
                    (Err(left), Err(right)) => {
                        assert_eq!(format!("{left:?}"), format!("{right:?}"))
                    }
                    _ => panic!("ownership changed execution outcome: {program:?}/{overrides}"),
                }
            }
        }
    }
}
