use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::Label;
use crate::error::RuntimeError;
use crate::value::{BasicType, Value};

pub type EnvRef = Rc<RefCell<Environment>>;

#[derive(Debug)]
pub struct Environment {
    vars: HashMap<String, Value>,
    constants: HashMap<String, Value>,
    parent: Option<EnvRef>,
    labels: HashMap<String, usize>,
    pub option_base: i32,
    pub shared_vars: HashSet<String>,
    // Names explicitly declared in this scope (DIM, SHARED, STATIC, CONST, etc.).
    // Used when OPTION EXPLICIT is enabled.
    declared_vars: HashSet<String>,
    // Declared types constrain assignment and restore defaults after CLEAR.
    declared_types: HashMap<String, BasicType>,
    // Implicit arrays have no scalar slot. Track first use separately so a later
    // DEFtype does not retroactively type them, or mistake a scalar prefix for one.
    initialized_arrays: HashSet<String>,
}

impl Environment {
    pub fn new_global() -> EnvRef {
        Rc::new(RefCell::new(Self {
            vars: HashMap::new(),
            constants: HashMap::new(),
            parent: None,
            labels: HashMap::new(),
            option_base: 1,
            shared_vars: HashSet::new(),
            declared_vars: HashSet::new(),
            declared_types: HashMap::new(),
            initialized_arrays: HashSet::new(),
        }))
    }

    pub fn new_child(parent: EnvRef) -> EnvRef {
        let option_base = parent.borrow().option_base;
        Rc::new(RefCell::new(Self {
            vars: HashMap::new(),
            constants: HashMap::new(),
            parent: Some(parent),
            labels: HashMap::new(),
            option_base,
            shared_vars: HashSet::new(),
            declared_vars: HashSet::new(),
            declared_types: HashMap::new(),
            initialized_arrays: HashSet::new(),
        }))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let key = name;
        if let Some(v) = self.constants.get(key) {
            return Some(v.clone());
        }
        // If variable is shared, read from root
        if self.shared_vars.contains(key)
            && let Some(parent) = &self.parent
        {
            return Self::get_from_root(parent, key);
        }
        if let Some(v) = self.vars.get(key) {
            return Some(v.clone());
        }
        // An uninitialized typed local (including after CLEAR) owns its default.
        if self.declared_types.contains_key(key) {
            return None;
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().get(name);
        }
        None
    }

    /// Read an array element without creating local type or ownership metadata.
    /// Typed local arrays shadow inherited values even before an element exists.
    pub fn get_array_element(&self, base: &str, key: &str) -> Option<Value> {
        if let Some(value) = self.constants.get(key) {
            return Some(value.clone());
        }
        if self.is_shared_binding(base)
            && let Some(parent) = &self.parent
        {
            return Self::get_from_root(parent, key);
        }
        if let Some(value) = self.vars.get(key) {
            return Some(value.clone());
        }
        if self.declared_types.contains_key(base) || self.declared_types.contains_key(key) {
            return None;
        }
        self.parent
            .as_ref()
            .and_then(|parent| parent.borrow().get_array_element(base, key))
    }

    /// Whether the write destination already owns a scalar or array binding.
    /// An unshared parent binding does not prevent a new implicit local binding.
    pub fn is_initialized(&self, name: &str) -> bool {
        if self.is_shared_binding(name)
            && let Some(parent) = &self.parent
        {
            return parent.borrow().shared_is_initialized(name);
        }
        self.declared_types.contains_key(name)
            || self.vars.contains_key(name)
            || self.initialized_arrays.contains(name)
            || self.is_const(name)
    }

    /// Whether an array read can see an existing scalar or array base binding.
    /// Read lookup may inherit a dynamic array even when writes would be local.
    pub fn array_is_initialized_for_read(&self, base: &str) -> bool {
        if self.is_shared_binding(base)
            && let Some(parent) = &self.parent
        {
            return parent.borrow().shared_is_initialized(base);
        }
        self.declared_types.contains_key(base)
            || self.vars.contains_key(base)
            || self.constants.contains_key(base)
            || self.initialized_arrays.contains(base)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.borrow().array_is_initialized_for_read(base))
    }

    /// Whether a module-level scalar or array binding already exists.
    pub fn shared_is_initialized(&self, name: &str) -> bool {
        match &self.parent {
            Some(parent) => parent.borrow().shared_is_initialized(name),
            None => {
                self.declared_types.contains_key(name)
                    || self.vars.contains_key(name)
                    || self.constants.contains_key(name)
                    || self.initialized_arrays.contains(name)
            }
        }
    }

    /// Record implicit array initialization without declaring it for OPTION EXPLICIT.
    pub fn mark_array_initialized(&mut self, base: &str) {
        if self.is_shared_binding(base)
            && let Some(parent) = &self.parent
        {
            Self::mark_array_initialized_in_root(parent, base);
            return;
        }
        self.initialized_arrays.insert(base.to_string());
    }

    /// Inspect initialization metadata owned by this scope for STATIC persistence.
    pub fn local_initialized_arrays(&self) -> &HashSet<String> {
        &self.initialized_arrays
    }

    /// Restore saved local metadata without redirecting it to an ancestor binding.
    pub fn restore_local_array_initialization(&mut self, names: &HashSet<String>) {
        self.initialized_arrays.extend(names.iter().cloned());
    }

    pub fn set(&mut self, name: &str, value: Value) {
        let key = name;
        // Don't overwrite constants
        if self.constants.contains_key(key) || self.is_const_in_parents(key) {
            return; // Constant cannot be reassigned
        }
        // If variable is shared in this scope or any ancestor, write to root
        if (self.shared_vars.contains(key) || self.is_shared_in_ancestors(key))
            && let Some(parent) = &self.parent
        {
            Self::set_in_root(parent, key, value);
            return;
        }
        self.set_local(name, value);
    }

    /// Assign with the constraints of a declaration or suffix at the write destination.
    /// An ordinary local write never inherits a parent's type constraint.
    pub fn set_checked(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if self.constants.contains_key(name) || self.is_const_in_parents(name) {
            return Err(RuntimeError::General {
                msg: format!("cannot assign to constant: {name}"),
            });
        }
        if self.is_shared_binding(name)
            && let Some(parent) = &self.parent
        {
            return Self::set_checked_in_root(parent, name, value);
        }
        let value = self.coerce_assignment(name, &value)?;
        self.set_local(name, value);
        Ok(())
    }

    /// Validate and convert a prospective assignment without changing either value.
    pub fn coerce_assignment(&self, name: &str, value: &Value) -> Result<Value, RuntimeError> {
        if self.is_const(name) {
            return Err(RuntimeError::General {
                msg: format!("cannot assign to constant: {name}"),
            });
        }
        let ty = if self.is_shared_binding(name) {
            match &self.parent {
                Some(parent) => Self::declared_type_from_root(parent, name),
                None => self.declared_types.get(name).cloned(),
            }
        } else {
            self.declared_types.get(name).cloned()
        };
        match ty.or_else(|| BasicType::from_suffix(name)) {
            Some(ty) => value.coerce_to(ty),
            None => Ok(value.clone()),
        }
    }

    /// Register a local type unless this scope explicitly shares the name.
    /// A declaration shadows an inherited shared binding.
    pub fn declare_type(&mut self, name: &str, ty: BasicType) {
        self.declared_vars.insert(name.to_string());
        if self.shared_vars.contains(name)
            && let Some(parent) = &self.parent
        {
            self.declared_types.remove(name);
            Self::declare_type_in_root(parent, name, ty);
            return;
        }
        self.declared_types.insert(name.to_string(), ty);
    }

    /// Remove explicit type metadata; suffix constraints still apply.
    pub fn clear_declared_type(&mut self, name: &str) {
        self.declared_vars.insert(name.to_string());
        self.declared_types.remove(name);
        if self.shared_vars.contains(name)
            && let Some(parent) = &self.parent
        {
            Self::clear_declared_type_in_root(parent, name);
        }
    }

    /// Resolve a declaration for a read, respecting local shadowing.
    pub fn declared_type(&self, name: &str) -> Option<BasicType> {
        if self.is_shared_binding(name)
            && let Some(parent) = &self.parent
        {
            return Self::declared_type_from_root(parent, name);
        }
        if let Some(ty) = self.declared_types.get(name) {
            return Some(ty.clone());
        }
        if self.declared_vars.contains(name)
            || self.vars.contains_key(name)
            || self.constants.contains_key(name)
        {
            return None;
        }
        self.parent
            .as_ref()
            .and_then(|parent| parent.borrow().declared_type(name))
    }

    /// A local declaration, excluding parent and shared declarations.
    pub fn local_declared_type(&self, name: &str) -> Option<BasicType> {
        self.declared_types.get(name).cloned()
    }

    /// Inspect local type metadata independently of whether values survive CLEAR.
    pub fn local_type_entries(&self) -> Vec<(String, BasicType)> {
        self.declared_types
            .iter()
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect()
    }

    /// Inspect the module-level value without changing the current binding.
    pub fn shared_value(&self, name: &str) -> Option<Value> {
        match &self.parent {
            Some(parent) => Self::get_from_root(parent, name),
            None => self
                .constants
                .get(name)
                .or_else(|| self.vars.get(name))
                .cloned(),
        }
    }

    /// Inspect the module-level declaration without changing the current binding.
    pub fn shared_declared_type(&self, name: &str) -> Option<BasicType> {
        match &self.parent {
            Some(parent) => Self::declared_type_from_root(parent, name),
            None => self.declared_types.get(name).cloned(),
        }
    }

    /// Inspect module-level variable entries before validating a shared array.
    pub fn shared_entries(&self) -> Vec<(String, Value)> {
        match &self.parent {
            Some(parent) => parent.borrow().shared_entries(),
            None => self
                .vars
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        }
    }

    /// Inspect flattened elements owned by the array's write destination.
    pub fn array_entries(&self, base: &str) -> Vec<(String, Value)> {
        if self.is_shared_binding(base)
            && let Some(parent) = &self.parent
        {
            return Self::array_entries_from_root(parent, base);
        }
        let prefix = format!("{base}_");
        self.vars
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    /// Clear flattened element values at the array's write destination.
    pub fn clear_array_elements(&mut self, base: &str) {
        if self.is_shared_binding(base)
            && let Some(parent) = &self.parent
        {
            Self::clear_array_elements_in_root(parent, base);
            return;
        }
        let prefix = format!("{base}_");
        self.vars.retain(|key, _| !key.starts_with(&prefix));
    }

    /// Validate an element write without installing local binding metadata.
    /// This uses the same base declaration as binding the destination for a write.
    pub fn coerce_element_assignment(
        &self,
        base: &str,
        key: &str,
        value: &Value,
    ) -> Result<Value, RuntimeError> {
        if self.is_const(key) {
            return Err(RuntimeError::General {
                msg: format!("cannot assign to constant: {key}"),
            });
        }
        match self
            .declared_type(base)
            .or_else(|| BasicType::from_suffix(base))
        {
            Some(ty) => value.coerce_to(ty),
            None => Ok(value.clone()),
        }
    }

    /// Give a flattened array element the declared type and ownership of its base.
    pub fn bind_element_type(&mut self, base: &str, key: &str) {
        self.mark_array_initialized(base);
        let ty = self
            .declared_type(base)
            .or_else(|| BasicType::from_suffix(base));
        if self.is_shared_binding(base) && self.parent.is_some() {
            self.shared_vars.insert(key.to_string());
        } else {
            self.shared_vars.remove(key);
            self.declared_vars.insert(key.to_string());
        }
        if let Some(ty) = ty {
            self.declare_type(key, ty);
        } else {
            self.clear_declared_type(key);
        }
    }

    fn set_local(&mut self, name: &str, value: Value) {
        if let Some(existing) = self.vars.get_mut(name) {
            *existing = value;
        } else {
            self.vars.insert(name.to_string(), value);
        }
    }

    pub fn define_const(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if self.constants.contains_key(name) {
            return Err(RuntimeError::DuplicateDefinition { name: name.into() });
        }
        self.constants.insert(name.to_string(), value);
        self.declared_vars.insert(name.to_string());
        Ok(())
    }

    pub fn declare_var(&mut self, name: &str) {
        self.declared_vars.insert(name.to_string());
    }

    pub fn is_declared(&self, name: &str) -> bool {
        self.declared_vars.contains(name)
    }

    /// True if the visible binding resolves to a module-level shared variable.
    pub fn is_shared(&self, name: &str) -> bool {
        self.is_shared_binding(name)
    }

    /// True if the name is a constant defined in this scope or any ancestor scope.
    pub fn is_const(&self, name: &str) -> bool {
        self.constants.contains_key(name) || self.is_const_in_parents(name)
    }

    pub fn register_label(&mut self, label: &Label, index: usize) {
        self.labels.insert(label.to_string(), index);
    }

    pub fn resolve_label(&self, label: &Label) -> Option<usize> {
        self.labels.get(&label.to_string()).copied()
    }

    pub fn clear_vars(&mut self) {
        self.vars.clear();
        self.initialized_arrays.clear();
    }

    pub fn var_keys(&self) -> Vec<String> {
        self.vars.keys().cloned().collect()
    }

    pub fn vars_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.vars
    }

    pub fn vars_ref(&self) -> &HashMap<String, Value> {
        &self.vars
    }

    pub fn var_entries(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.vars.iter()
    }

    fn is_const_in_parents(&self, key: &str) -> bool {
        if let Some(parent) = &self.parent {
            let p = parent.borrow();
            if p.constants.contains_key(key) {
                return true;
            }
            p.is_const_in_parents(key)
        } else {
            false
        }
    }

    fn is_shared_in_ancestors(&self, key: &str) -> bool {
        if let Some(parent) = &self.parent {
            let p = parent.borrow();
            if p.shared_vars.contains(key) {
                return true;
            }
            p.is_shared_in_ancestors(key)
        } else {
            false
        }
    }

    fn is_shared_binding(&self, key: &str) -> bool {
        if self.shared_vars.contains(key) {
            return true;
        }
        if self.declared_vars.contains(key)
            || self.vars.contains_key(key)
            || self.constants.contains_key(key)
            || self.declared_types.contains_key(key)
        {
            return false;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.borrow().is_shared_binding(key))
    }

    fn set_checked_in_root(env: &EnvRef, key: &str, value: Value) -> Result<(), RuntimeError> {
        let parent = env.borrow().parent.clone();
        match parent {
            Some(parent) => Self::set_checked_in_root(&parent, key, value),
            None => env.borrow_mut().set_checked(key, value),
        }
    }

    fn declare_type_in_root(env: &EnvRef, key: &str, ty: BasicType) {
        let parent = env.borrow().parent.clone();
        match parent {
            Some(parent) => Self::declare_type_in_root(&parent, key, ty),
            None => env.borrow_mut().declare_type(key, ty),
        }
    }

    fn clear_declared_type_in_root(env: &EnvRef, key: &str) {
        let parent = env.borrow().parent.clone();
        match parent {
            Some(parent) => Self::clear_declared_type_in_root(&parent, key),
            None => env.borrow_mut().clear_declared_type(key),
        }
    }

    fn array_entries_from_root(env: &EnvRef, base: &str) -> Vec<(String, Value)> {
        let env = env.borrow();
        match &env.parent {
            Some(parent) => Self::array_entries_from_root(parent, base),
            None => env.array_entries(base),
        }
    }

    fn mark_array_initialized_in_root(env: &EnvRef, base: &str) {
        let parent = env.borrow().parent.clone();
        match parent {
            Some(parent) => Self::mark_array_initialized_in_root(&parent, base),
            None => {
                env.borrow_mut().initialized_arrays.insert(base.to_string());
            }
        }
    }

    fn clear_array_elements_in_root(env: &EnvRef, base: &str) {
        let parent = env.borrow().parent.clone();
        match parent {
            Some(parent) => Self::clear_array_elements_in_root(&parent, base),
            None => env.borrow_mut().clear_array_elements(base),
        }
    }

    fn declared_type_from_root(env: &EnvRef, key: &str) -> Option<BasicType> {
        let env = env.borrow();
        match &env.parent {
            Some(parent) => Self::declared_type_from_root(parent, key),
            None => env.declared_types.get(key).cloned(),
        }
    }

    fn get_from_root(env: &EnvRef, key: &str) -> Option<Value> {
        let e = env.borrow();
        if let Some(parent) = &e.parent {
            Self::get_from_root(parent, key)
        } else {
            e.constants.get(key).or_else(|| e.vars.get(key)).cloned()
        }
    }

    fn set_in_root(env: &EnvRef, key: &str, value: Value) {
        let mut e = env.borrow_mut();
        if e.parent.is_none() {
            // This is root
            e.set_local(key, value);
        } else {
            let parent = e.parent.clone().unwrap();
            drop(e);
            Self::set_in_root(&parent, key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_reads_inherit_values_without_installing_local_bindings() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("VALUES", BasicType::Integer);
        root.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        root.borrow_mut()
            .set_checked("VALUES_1", Value::Integer(42))
            .unwrap();
        let child = Environment::new_child(root.clone());
        let grandchild = Environment::new_child(child.clone());
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_1"),
            Some(Value::Integer(42))
        );
        assert!(child.borrow().local_type_entries().is_empty());
        assert!(grandchild.borrow().local_type_entries().is_empty());
        assert!(!grandchild.borrow().is_initialized("VALUES"));

        child.borrow_mut().declare_type("VALUES", BasicType::Long);
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_1"),
            None
        );
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        child
            .borrow_mut()
            .set_checked("VALUES_1", Value::Long(7))
            .unwrap();
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_1"),
            Some(Value::Long(7))
        );
        child.borrow_mut().clear_vars();
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_1"),
            None
        );
        assert_eq!(root.borrow().get("VALUES_1"), Some(Value::Integer(42)));
    }

    #[test]
    fn local_flattened_constants_take_precedence_over_shared_array_values() {
        let root = Environment::new_global();
        root.borrow_mut().shared_vars.insert("VALUES".into());
        root.borrow_mut().declare_type("VALUES", BasicType::Integer);
        root.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        root.borrow_mut()
            .set_checked("VALUES_1", Value::Integer(42))
            .unwrap();
        let child = Environment::new_child(root.clone());
        child
            .borrow_mut()
            .define_const("VALUES_1", Value::Integer(7))
            .unwrap();

        assert!(child.borrow().is_shared("VALUES"));
        assert_eq!(child.borrow().get("VALUES_1"), Some(Value::Integer(7)));
        assert_eq!(
            child.borrow().get_array_element("VALUES", "VALUES_1"),
            Some(Value::Integer(7))
        );
        assert!(
            child
                .borrow()
                .coerce_element_assignment("VALUES", "VALUES_1", &Value::Integer(9))
                .is_err()
        );
        assert_eq!(root.borrow().get("VALUES_1"), Some(Value::Integer(42)));
    }

    #[test]
    fn array_initialization_tracks_bases_without_changing_declaration_status() {
        let env = Environment::new_global();
        let mut env = env.borrow_mut();
        env.set_checked("VALUES_1", Value::Numeric(7.0)).unwrap();
        assert!(!env.is_initialized("VALUES"));
        env.mark_array_initialized("VALUES");
        assert!(env.is_initialized("VALUES"));
        assert!(env.shared_is_initialized("VALUES"));
        assert!(!env.is_declared("VALUES"));
        assert_eq!(env.declared_type("VALUES"), None);

        env.clear_array_elements("VALUES");
        assert!(env.is_initialized("VALUES"));
        env.declare_type("TYPED", BasicType::Integer);
        env.bind_element_type("TYPED", "TYPED_1");
        env.clear_vars();
        assert!(!env.is_initialized("VALUES"));
        assert!(env.is_initialized("TYPED"));
    }

    #[test]
    fn initialized_array_reads_inherit_dynamic_bases_without_creating_locals() {
        let root = Environment::new_global();
        root.borrow_mut().mark_array_initialized("VALUES");
        root.borrow_mut()
            .set_checked("UNRELATED_1", Value::Numeric(7.0))
            .unwrap();
        let child = Environment::new_child(root.clone());
        let grandchild = Environment::new_child(child.clone());
        assert!(grandchild.borrow().array_is_initialized_for_read("VALUES"));
        assert!(!grandchild.borrow().is_initialized("VALUES"));
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_2"),
            None
        );
        assert!(
            !grandchild
                .borrow()
                .array_is_initialized_for_read("UNRELATED")
        );
        assert!(grandchild.borrow().initialized_arrays.is_empty());

        root.borrow_mut().clear_vars();
        assert!(!grandchild.borrow().array_is_initialized_for_read("VALUES"));
        child
            .borrow_mut()
            .declare_type("VALUES", BasicType::Integer);
        child.borrow_mut().clear_vars();
        assert!(grandchild.borrow().array_is_initialized_for_read("VALUES"));
        assert!(!grandchild.borrow().is_initialized("VALUES"));

        root.borrow_mut().shared_vars.insert("SHARED".into());
        root.borrow_mut().mark_array_initialized("SHARED");
        assert!(grandchild.borrow().array_is_initialized_for_read("SHARED"));
        root.borrow_mut().clear_vars();
        assert!(!grandchild.borrow().array_is_initialized_for_read("SHARED"));
    }

    #[test]
    fn local_array_initialization_can_be_saved_and_restored_independently() {
        let root = Environment::new_global();
        root.borrow_mut().shared_vars.insert("SHARED".into());
        let child = Environment::new_child(root.clone());
        child.borrow_mut().mark_array_initialized("LOCAL");
        child.borrow_mut().mark_array_initialized("SHARED");
        let saved = child.borrow().local_initialized_arrays().clone();
        assert_eq!(saved, HashSet::from(["LOCAL".into()]));

        let next_call = Environment::new_child(root);
        next_call
            .borrow_mut()
            .restore_local_array_initialization(&saved);
        assert!(next_call.borrow().is_initialized("LOCAL"));
        assert!(!next_call.borrow().is_declared("LOCAL"));
        assert!(next_call.borrow().vars_ref().is_empty());
        next_call.borrow_mut().clear_vars();
        assert!(next_call.borrow().local_initialized_arrays().is_empty());
        assert!(!next_call.borrow().is_initialized("LOCAL"));
    }

    #[test]
    fn shared_array_reads_and_initialization_follow_root_ownership() {
        let root = Environment::new_global();
        root.borrow_mut().shared_vars.insert("VALUES".into());
        root.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        root.borrow_mut()
            .set_checked("VALUES_1", Value::Str("root".into()))
            .unwrap();
        let child = Environment::new_child(root.clone());
        let grandchild = Environment::new_child(child.clone());
        assert!(grandchild.borrow().is_initialized("VALUES"));
        assert!(grandchild.borrow().shared_is_initialized("VALUES"));
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_1"),
            Some(Value::Str("root".into()))
        );
        assert!(grandchild.borrow().initialized_arrays.is_empty());

        root.borrow_mut().shared_vars.insert("MISSING".into());
        grandchild.borrow_mut().mark_array_initialized("MISSING");
        assert!(root.borrow().is_initialized("MISSING"));
        assert!(grandchild.borrow().initialized_arrays.is_empty());

        child.borrow_mut().declare_type("VALUES", BasicType::String);
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        child
            .borrow_mut()
            .set_checked("VALUES_1", Value::Str("local".into()))
            .unwrap();
        assert!(!grandchild.borrow().is_shared("VALUES"));
        assert_eq!(
            grandchild.borrow().get_array_element("VALUES", "VALUES_1"),
            Some(Value::Str("local".into()))
        );
        assert_eq!(
            root.borrow().get("VALUES_1"),
            Some(Value::Str("root".into()))
        );
    }

    #[test]
    fn prospective_element_writes_validate_without_shadowing_parent_values() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("VALUES", BasicType::Integer);
        root.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        root.borrow_mut()
            .set_checked("VALUES_1", Value::Integer(42))
            .unwrap();
        root.borrow_mut()
            .define_const("VALUES_2", Value::Integer(3))
            .unwrap();
        let child = Environment::new_child(root);
        assert_eq!(
            child
                .borrow()
                .coerce_element_assignment("VALUES", "VALUES_1", &Value::Numeric(7.0))
                .unwrap(),
            Value::Integer(7)
        );
        assert!(
            child
                .borrow()
                .coerce_element_assignment("VALUES", "VALUES_1", &Value::Numeric(32768.0))
                .is_err()
        );
        assert!(
            child
                .borrow()
                .coerce_element_assignment("VALUES", "VALUES_2", &Value::Integer(4))
                .is_err()
        );
        assert_eq!(
            child.borrow().get_array_element("VALUES", "VALUES_1"),
            Some(Value::Integer(42))
        );
        assert!(child.borrow().local_type_entries().is_empty());
        assert!(!child.borrow().is_initialized("VALUES"));
        assert_eq!(
            child
                .borrow()
                .coerce_element_assignment("SUFFIX%", "SUFFIX%_1", &Value::Numeric(7.0))
                .unwrap(),
            Value::Integer(7)
        );
        child.borrow_mut().declare_type("VALUES", BasicType::String);
        assert_eq!(
            child
                .borrow()
                .coerce_element_assignment("VALUES", "VALUES_1", &Value::Str("local".into()))
                .unwrap(),
            Value::Str("local".into())
        );
    }

    #[test]
    fn every_declared_primitive_type_checks_assignment_and_survives_clear() {
        let env = Environment::new_global();
        let mut env = env.borrow_mut();
        for (ty, input, expected) in [
            (BasicType::Numeric, Value::Int64(7), Value::Numeric(7.0)),
            (BasicType::Integer, Value::Numeric(7.0), Value::Integer(7)),
            (BasicType::Long, Value::Numeric(7.0), Value::Long(7)),
            (BasicType::Single, Value::Numeric(7.0), Value::Single(7.0)),
            (BasicType::Double, Value::Int64(7), Value::Numeric(7.0)),
            (BasicType::Int64, Value::Numeric(7.0), Value::Int64(7)),
            (BasicType::Bool, Value::Numeric(7.0), Value::Bool(true)),
            (BasicType::Byte, Value::Numeric(7.0), Value::Byte(7)),
            (
                BasicType::Bytes,
                Value::Bytes(vec![7]),
                Value::Bytes(vec![7]),
            ),
            (
                BasicType::String,
                Value::Str("a".into()),
                Value::Str("a".into()),
            ),
            (
                BasicType::FixedLengthString(3),
                Value::Str("a".into()),
                Value::Str("a  ".into()),
            ),
        ] {
            env.declare_type("ITEM", ty.clone());
            env.set_checked("ITEM", input).unwrap();
            assert_eq!(env.get("ITEM").unwrap(), expected, "{ty:?}");
            assert_eq!(
                env.get("ITEM").unwrap().get_type(),
                expected.get_type(),
                "{ty:?}"
            );
            let invalid = Value::Record {
                type_name: "OTHER".into(),
                fields: HashMap::new(),
            };
            assert!(env.set_checked("ITEM", invalid).is_err(), "{ty:?}");
            assert_eq!(env.get("ITEM").unwrap(), expected, "{ty:?}");
            env.clear_vars();
            assert_eq!(env.declared_type("ITEM"), Some(ty));
            assert!(env.get("ITEM").is_none());
        }
    }

    #[test]
    fn suffixes_constrain_values_unless_an_explicit_type_overrides_them() {
        let env = Environment::new_global();
        let mut env = env.borrow_mut();
        for (name, value, expected) in [
            ("ITEM%", Value::Numeric(7.0), Value::Integer(7)),
            ("ITEM&", Value::Numeric(7.0), Value::Long(7)),
            ("ITEM!", Value::Numeric(7.0), Value::Single(7.0)),
            ("ITEM#", Value::Int64(7), Value::Numeric(7.0)),
            ("ITEM$", Value::Str("a".into()), Value::Str("a".into())),
        ] {
            env.set_checked(name, value).unwrap();
            assert_eq!(env.get(name).unwrap(), expected);
            assert_eq!(env.get(name).unwrap().get_type(), expected.get_type());
            assert_eq!(env.declared_type(name), None);
            assert!(env.set_checked(name, Value::Bytes(vec![])).is_err());
            env.declare_type(name, BasicType::Bytes);
            env.set_checked(name, Value::Bytes(vec![1])).unwrap();
            assert_eq!(env.get(name).unwrap(), Value::Bytes(vec![1]));
            env.clear_declared_type(name);
            assert!(env.set_checked(name, Value::Bytes(vec![])).is_err());
        }
    }

    #[test]
    fn untyped_array_elements_remain_dynamic_and_suffixed_elements_are_typed() {
        let env = Environment::new_global();
        let mut env = env.borrow_mut();
        env.bind_element_type("VALUES%", "VALUES%_1");
        env.set_checked("VALUES%_1", Value::Numeric(4.0)).unwrap();
        assert_eq!(env.get("VALUES%_1").unwrap(), Value::Integer(4));
        assert!(
            env.set_checked("VALUES%_1", Value::Str("wrong".into()))
                .is_err()
        );

        env.declare_type("VALUES", BasicType::Byte);
        env.bind_element_type("VALUES", "VALUES_1");
        assert!(env.set_checked("VALUES_1", Value::Numeric(256.0)).is_err());
        env.clear_declared_type("VALUES");
        env.bind_element_type("VALUES", "VALUES_1");
        assert_eq!(env.declared_type("VALUES_1"), None);
        env.set_checked("VALUES_1", Value::Str("dynamic".into()))
            .unwrap();
        env.set_checked("VALUES_1", Value::Numeric(2.5)).unwrap();
        assert_eq!(env.get("VALUES_1").unwrap(), Value::Numeric(2.5));
    }

    #[test]
    fn shared_inspection_reads_root_without_changing_local_bindings() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("ITEM", BasicType::Integer);
        root.borrow_mut()
            .set_checked("ITEM", Value::Numeric(7.0))
            .unwrap();
        root.borrow_mut()
            .define_const("FIXED", Value::Integer(9))
            .unwrap();
        let child = Environment::new_child(root);
        child.borrow_mut().declare_type("ITEM", BasicType::String);
        child
            .borrow_mut()
            .set_checked("ITEM", Value::Str("local".into()))
            .unwrap();
        assert_eq!(child.borrow().shared_value("ITEM"), Some(Value::Integer(7)));
        assert_eq!(
            child.borrow().shared_value("FIXED"),
            Some(Value::Integer(9))
        );
        assert_eq!(
            child.borrow().shared_declared_type("ITEM"),
            Some(BasicType::Integer)
        );
        assert_eq!(
            child.borrow().shared_entries(),
            vec![("ITEM".into(), Value::Integer(7))]
        );
        assert_eq!(child.borrow().get("ITEM"), Some(Value::Str("local".into())));
        assert_eq!(
            child.borrow().local_declared_type("ITEM"),
            Some(BasicType::String)
        );
        assert!(!child.borrow().is_shared("ITEM"));
    }

    #[test]
    fn local_type_entries_survive_clear_and_exclude_shared_root_metadata() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("GLOBAL", BasicType::Integer);
        let child = Environment::new_child(root);
        child.borrow_mut().shared_vars.insert("GLOBAL".into());
        child
            .borrow_mut()
            .declare_type("GLOBAL", BasicType::Integer);
        child.borrow_mut().declare_type("LOCAL", BasicType::String);
        child
            .borrow_mut()
            .set_checked("LOCAL", Value::Str("value".into()))
            .unwrap();
        child.borrow_mut().clear_vars();
        assert_eq!(
            child.borrow().local_type_entries(),
            vec![("LOCAL".into(), BasicType::String)]
        );
        assert!(child.borrow().vars_ref().is_empty());
    }

    #[test]
    fn clearing_shared_element_metadata_keeps_untyped_elements_dynamic() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("VALUES", BasicType::Byte);
        let child = Environment::new_child(root.clone());
        child.borrow_mut().shared_vars.insert("VALUES".into());
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        child
            .borrow_mut()
            .set_checked("VALUES_1", Value::Byte(7))
            .unwrap();
        child.borrow_mut().clear_declared_type("VALUES");
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        assert_eq!(root.borrow().declared_type("VALUES"), None);
        assert_eq!(root.borrow().declared_type("VALUES_1"), None);
        child
            .borrow_mut()
            .set_checked("VALUES_1", Value::Str("dynamic".into()))
            .unwrap();
        assert_eq!(
            root.borrow().get("VALUES_1"),
            Some(Value::Str("dynamic".into()))
        );
        assert!(child.borrow().vars_ref().is_empty());
    }

    #[test]
    fn array_entries_and_clear_follow_shared_ownership_and_local_shadowing() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("VALUES", BasicType::Integer);
        root.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        root.borrow_mut()
            .set_checked("VALUES_1", Value::Integer(7))
            .unwrap();
        root.borrow_mut()
            .set_checked("UNRELATED", Value::Numeric(9.0))
            .unwrap();
        root.borrow_mut()
            .define_const("VALUES_FIXED", Value::Integer(11))
            .unwrap();
        let child = Environment::new_child(root.clone());
        child.borrow_mut().shared_vars.insert("VALUES".into());
        let shared_grandchild = Environment::new_child(child.clone());
        assert_eq!(
            shared_grandchild.borrow().array_entries("VALUES"),
            vec![("VALUES_1".into(), Value::Integer(7))]
        );

        let local_grandchild = Environment::new_child(child);
        local_grandchild
            .borrow_mut()
            .declare_type("VALUES", BasicType::String);
        local_grandchild
            .borrow_mut()
            .bind_element_type("VALUES", "VALUES_1");
        local_grandchild
            .borrow_mut()
            .set_checked("VALUES_1", Value::Str("local".into()))
            .unwrap();
        assert_eq!(
            local_grandchild.borrow().array_entries("VALUES"),
            vec![("VALUES_1".into(), Value::Str("local".into()))]
        );
        local_grandchild.borrow_mut().clear_array_elements("VALUES");
        assert!(local_grandchild.borrow().array_entries("VALUES").is_empty());
        assert_eq!(root.borrow().get("VALUES_1"), Some(Value::Integer(7)));

        shared_grandchild
            .borrow_mut()
            .clear_array_elements("VALUES");
        assert!(root.borrow().array_entries("VALUES").is_empty());
        assert_eq!(root.borrow().get("UNRELATED"), Some(Value::Numeric(9.0)));
        assert_eq!(root.borrow().get("VALUES_FIXED"), Some(Value::Integer(11)));
        assert_eq!(
            root.borrow().declared_type("VALUES"),
            Some(BasicType::Integer)
        );
    }

    #[test]
    fn checked_assignment_converts_strict_values_and_preserves_them_on_error() {
        let env = Environment::new_global();
        let mut env = env.borrow_mut();
        env.declare_type("COUNT", BasicType::Int64);
        env.set_checked("COUNT", Value::Numeric(42.0)).unwrap();
        assert!(matches!(env.get("COUNT"), Some(Value::Int64(42))));
        for invalid in [Value::Numeric(2.5), Value::Str("42".into())] {
            assert!(env.set_checked("COUNT", invalid).is_err());
            assert!(matches!(env.get("COUNT"), Some(Value::Int64(42))));
        }

        env.declare_type("OCTET", BasicType::Byte);
        env.set_checked("OCTET", Value::Numeric(255.0)).unwrap();
        assert!(env.set_checked("OCTET", Value::Numeric(256.0)).is_err());
        assert!(matches!(env.get("OCTET"), Some(Value::Byte(255))));
        env.declare_type("ENABLED", BasicType::Bool);
        env.set_checked("ENABLED", Value::Bool(true)).unwrap();
        assert!(
            env.set_checked("ENABLED", Value::Str("true".into()))
                .is_err()
        );
        env.declare_type("PAYLOAD", BasicType::Bytes);
        env.set_checked("PAYLOAD", Value::Bytes(vec![0, 255]))
            .unwrap();
        assert!(
            env.set_checked("PAYLOAD", Value::Str("data".into()))
                .is_err()
        );
        assert!(matches!(env.get("PAYLOAD"), Some(Value::Bytes(bytes)) if bytes == [0, 255]));
    }

    #[test]
    fn declarations_survive_clear_and_explicitly_removing_a_type_restores_dynamic_assignment() {
        let env = Environment::new_global();
        let mut env = env.borrow_mut();
        env.declare_type("ITEM", BasicType::Int64);
        env.set_checked("ITEM", Value::Int64(10)).unwrap();
        env.clear_vars();
        assert!(env.get("ITEM").is_none());
        assert_eq!(env.declared_type("ITEM"), Some(BasicType::Int64));
        assert!(env.set_checked("ITEM", Value::Str("wrong".into())).is_err());
        env.declare_type("ITEM", BasicType::Numeric);
        assert_eq!(env.declared_type("ITEM"), Some(BasicType::Numeric));
        assert!(env.set_checked("ITEM", Value::Str("wrong".into())).is_err());
        env.clear_declared_type("ITEM");
        assert_eq!(env.declared_type("ITEM"), None);
        env.set_checked("ITEM", Value::Str("allowed".into()))
            .unwrap();
        assert!(matches!(env.get("ITEM"), Some(Value::Str(text)) if text == "allowed"));
    }

    #[test]
    fn cleared_strict_locals_and_elements_do_not_read_parent_values() {
        let root = Environment::new_global();
        root.borrow_mut().set("ITEM", Value::Numeric(200.0));
        root.borrow_mut().set("VALUES_1", Value::Numeric(300.0));
        let child = Environment::new_child(root);
        child.borrow_mut().declare_type("ITEM", BasicType::Byte);
        child.borrow_mut().declare_type("VALUES", BasicType::Byte);
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        child
            .borrow_mut()
            .set_checked("ITEM", Value::Byte(5))
            .unwrap();
        child
            .borrow_mut()
            .set_checked("VALUES_1", Value::Byte(6))
            .unwrap();
        child.borrow_mut().clear_vars();
        assert!(child.borrow().get("ITEM").is_none());
        assert!(child.borrow().get("VALUES_1").is_none());
        assert_eq!(child.borrow().declared_type("ITEM"), Some(BasicType::Byte));
        assert_eq!(
            child.borrow().declared_type("VALUES_1"),
            Some(BasicType::Byte)
        );
    }

    #[test]
    fn record_declarations_check_identity_and_survive_clear() {
        let record_type = BasicType::UserDefined("PACKET".into());
        let root = Environment::new_global();
        root.borrow_mut().set("ITEM", Value::Numeric(42.0));
        root.borrow_mut().set("VALUES_1", Value::Numeric(43.0));
        let child = Environment::new_child(root);
        child.borrow_mut().declare_type("ITEM", record_type.clone());
        child
            .borrow_mut()
            .declare_type("VALUES", record_type.clone());
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        for invalid in [
            Value::Str("wrong".into()),
            Value::Numeric(7.0),
            Value::Record {
                type_name: "OTHER".into(),
                fields: HashMap::new(),
            },
        ] {
            assert!(
                child
                    .borrow_mut()
                    .set_checked("ITEM", invalid.clone())
                    .is_err()
            );
            assert!(child.borrow_mut().set_checked("VALUES_1", invalid).is_err());
        }
        let record = Value::Record {
            type_name: "PACKET".into(),
            fields: HashMap::new(),
        };
        child
            .borrow_mut()
            .set_checked("ITEM", record.clone())
            .unwrap();
        child.borrow_mut().set_checked("VALUES_1", record).unwrap();
        child.borrow_mut().clear_vars();
        assert!(child.borrow().get("ITEM").is_none());
        assert!(child.borrow().get("VALUES_1").is_none());
        assert_eq!(
            child.borrow().declared_type("ITEM"),
            Some(record_type.clone())
        );
        assert_eq!(child.borrow().declared_type("VALUES_1"), Some(record_type));
    }

    #[test]
    fn local_assignment_and_declaration_shadow_parent_constraints() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("ITEM", BasicType::Int64);
        root.borrow_mut()
            .set_checked("ITEM", Value::Int64(5))
            .unwrap();
        let child = Environment::new_child(root.clone());
        assert_eq!(child.borrow().declared_type("ITEM"), Some(BasicType::Int64));
        assert_eq!(child.borrow().local_declared_type("ITEM"), None);
        child
            .borrow_mut()
            .set_checked("ITEM", Value::Str("local".into()))
            .unwrap();
        assert_eq!(child.borrow().declared_type("ITEM"), None);
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Int64(5))));

        let child = Environment::new_child(root.clone());
        child.borrow_mut().declare_type("ITEM", BasicType::String);
        assert_eq!(
            child.borrow().declared_type("ITEM"),
            Some(BasicType::String)
        );
        child
            .borrow_mut()
            .set_checked("ITEM", Value::Str("local".into()))
            .unwrap();
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Int64(5))));
    }

    #[test]
    fn explicit_local_parameter_shadows_ancestor_shared_type() {
        let root = Environment::new_global();
        root.borrow_mut().shared_vars.insert("ITEM".into());
        root.borrow_mut().declare_type("ITEM", BasicType::Int64);
        root.borrow_mut()
            .set_checked("ITEM", Value::Int64(5))
            .unwrap();
        let child = Environment::new_child(root.clone());
        child.borrow_mut().declare_type("ITEM", BasicType::Bool);
        assert!(!child.borrow().is_shared("ITEM"));
        child
            .borrow_mut()
            .set_checked("ITEM", Value::Bool(true))
            .unwrap();
        assert_eq!(child.borrow().declared_type("ITEM"), Some(BasicType::Bool));
        assert!(matches!(
            child.borrow().get("ITEM"),
            Some(Value::Bool(true))
        ));
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Int64(5))));
    }

    #[test]
    fn checked_shared_assignments_validate_the_root_declaration() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("ITEM", BasicType::Byte);
        let child = Environment::new_child(root.clone());
        child.borrow_mut().shared_vars.insert("ITEM".into());
        let grandchild = Environment::new_child(child.clone());
        grandchild
            .borrow_mut()
            .set_checked("ITEM", Value::Numeric(7.0))
            .unwrap();
        assert!(
            grandchild
                .borrow_mut()
                .set_checked("ITEM", Value::Numeric(256.0))
                .is_err()
        );
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Byte(7))));
        assert_eq!(
            grandchild.borrow().declared_type("ITEM"),
            Some(BasicType::Byte)
        );
        assert!(grandchild.borrow().vars_ref().is_empty());

        grandchild
            .borrow_mut()
            .declare_type("ITEM", BasicType::Bool);
        grandchild
            .borrow_mut()
            .set_checked("ITEM", Value::Bool(true))
            .unwrap();
        assert_eq!(
            grandchild.borrow().local_declared_type("ITEM"),
            Some(BasicType::Bool)
        );
        assert_eq!(root.borrow().declared_type("ITEM"), Some(BasicType::Byte));
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Byte(7))));

        let explicit_shared = Environment::new_child(child);
        explicit_shared
            .borrow_mut()
            .shared_vars
            .insert("ITEM".into());
        explicit_shared
            .borrow_mut()
            .declare_type("ITEM", BasicType::Bool);
        explicit_shared
            .borrow_mut()
            .set_checked("ITEM", Value::Bool(true))
            .unwrap();
        assert_eq!(root.borrow().declared_type("ITEM"), Some(BasicType::Bool));
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Bool(true))));
        assert_eq!(explicit_shared.borrow().local_declared_type("ITEM"), None);
    }

    #[test]
    fn flattened_elements_inherit_type_and_shared_destination() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("VALUES", BasicType::Int64);
        let child = Environment::new_child(root.clone());
        child.borrow_mut().shared_vars.insert("VALUES".into());
        child.borrow_mut().bind_element_type("VALUES", "VALUES_1");
        child
            .borrow_mut()
            .set_checked("VALUES_1", Value::Numeric(17.0))
            .unwrap();
        assert!(
            child
                .borrow_mut()
                .set_checked("VALUES_1", Value::Numeric(1.5))
                .is_err()
        );
        assert!(matches!(
            root.borrow().get("VALUES_1"),
            Some(Value::Int64(17))
        ));
        assert!(matches!(
            child.borrow().get("VALUES_1"),
            Some(Value::Int64(17))
        ));
        assert!(child.borrow().vars_ref().is_empty());

        let grandchild = Environment::new_child(child.clone());
        grandchild.borrow_mut().declare_var("VALUES");
        grandchild
            .borrow_mut()
            .declare_type("VALUES", BasicType::String);
        grandchild
            .borrow_mut()
            .bind_element_type("VALUES", "VALUES_1");
        grandchild
            .borrow_mut()
            .set_checked("VALUES_1", Value::Str("local".into()))
            .unwrap();
        assert!(
            matches!(grandchild.borrow().get("VALUES_1"), Some(Value::Str(text)) if text == "local")
        );
        assert!(matches!(
            root.borrow().get("VALUES_1"),
            Some(Value::Int64(17))
        ));
    }

    #[test]
    fn checked_assignment_preserves_constants_and_legacy_set_is_unchecked() {
        let root = Environment::new_global();
        root.borrow_mut().declare_type("FIXED", BasicType::Int64);
        root.borrow_mut()
            .define_const("FIXED", Value::Int64(9))
            .unwrap();
        let child = Environment::new_child(root.clone());
        assert!(
            child
                .borrow_mut()
                .set_checked("FIXED", Value::Str("ignored".into()))
                .is_err()
        );
        assert!(
            child
                .borrow()
                .coerce_assignment("FIXED", &Value::Int64(10))
                .is_err()
        );
        assert!(
            root.borrow_mut()
                .set_checked("FIXED", Value::Int64(10))
                .is_err()
        );
        assert!(matches!(root.borrow().get("FIXED"), Some(Value::Int64(9))));
        root.borrow_mut().declare_type("ITEM", BasicType::Int64);
        root.borrow_mut().set("ITEM", Value::Str("legacy".into()));
        assert!(matches!(root.borrow().get("ITEM"), Some(Value::Str(text)) if text == "legacy"));
    }

    #[test]
    fn repeated_local_updates_preserve_parent_binding() {
        let root = Environment::new_global();
        root.borrow_mut().set("COUNT", Value::Numeric(10.0));
        let child = Environment::new_child(root.clone());

        for value in [1.0, 2.0, 3.0] {
            child.borrow_mut().set("COUNT", Value::Numeric(value));
            assert!(matches!(child.borrow().get("COUNT"), Some(Value::Numeric(n)) if n == value));
        }

        assert!(matches!(
            root.borrow().get("COUNT"),
            Some(Value::Numeric(10.0))
        ));
    }

    #[test]
    fn repeated_shared_updates_target_root_through_ancestors() {
        let root = Environment::new_global();
        let child = Environment::new_child(root.clone());
        child.borrow_mut().shared_vars.insert("COUNT".into());
        let grandchild = Environment::new_child(child.clone());

        for value in [1.0, 2.0, 3.0] {
            grandchild.borrow_mut().set("COUNT", Value::Numeric(value));
            assert!(matches!(root.borrow().get("COUNT"), Some(Value::Numeric(n)) if n == value));
            assert!(
                matches!(grandchild.borrow().get("COUNT"), Some(Value::Numeric(n)) if n == value)
            );
        }

        assert!(child.borrow().vars_ref().is_empty());
        assert!(grandchild.borrow().vars_ref().is_empty());
    }

    #[test]
    fn updates_preserve_local_and_ancestor_constants() {
        let root = Environment::new_global();
        root.borrow_mut()
            .define_const("FIXED", Value::Numeric(10.0))
            .unwrap();
        let child = Environment::new_child(root.clone());
        child
            .borrow_mut()
            .define_const("LOCAL", Value::Numeric(20.0))
            .unwrap();

        for value in [1.0, 2.0] {
            child.borrow_mut().set("FIXED", Value::Numeric(value));
            child.borrow_mut().set("LOCAL", Value::Numeric(value));
        }

        assert!(matches!(
            root.borrow().get("FIXED"),
            Some(Value::Numeric(10.0))
        ));
        assert!(matches!(
            child.borrow().get("FIXED"),
            Some(Value::Numeric(10.0))
        ));
        assert!(matches!(
            child.borrow().get("LOCAL"),
            Some(Value::Numeric(20.0))
        ));
        assert!(child.borrow().vars_ref().is_empty());
    }
}
