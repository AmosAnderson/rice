use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::Label;
use crate::error::RuntimeError;
use crate::value::Value;

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
        if let Some(parent) = &self.parent {
            return parent.borrow().get(name);
        }
        None
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

    /// True if the variable is shared in this scope or any ancestor scope
    /// (e.g. via SHARED or COMMON), so it resolves to a module-level variable.
    pub fn is_shared(&self, name: &str) -> bool {
        self.shared_vars.contains(name) || self.is_shared_in_ancestors(name)
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

    fn get_from_root(env: &EnvRef, key: &str) -> Option<Value> {
        let e = env.borrow();
        if let Some(parent) = &e.parent {
            Self::get_from_root(parent, key)
        } else {
            e.vars.get(key).cloned()
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
