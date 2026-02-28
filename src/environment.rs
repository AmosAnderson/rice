use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Label;
use crate::error::RuntimeError;
use crate::token::TypeSuffix;
use crate::value::Value;

pub type EnvRef = Rc<RefCell<Environment>>;

#[derive(Debug)]
pub struct Environment {
    vars: HashMap<String, Value>,
    constants: HashMap<String, Value>,
    parent: Option<EnvRef>,
    labels: HashMap<String, usize>,
    pub gosub_stack: Vec<usize>,
    pub option_base: i32,
}

impl Environment {
    pub fn new_global() -> EnvRef {
        Rc::new(RefCell::new(Self {
            vars: HashMap::new(),
            constants: HashMap::new(),
            parent: None,
            labels: HashMap::new(),
            gosub_stack: Vec::new(),
            option_base: 0,
        }))
    }

    pub fn new_child(parent: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self {
            vars: HashMap::new(),
            constants: HashMap::new(),
            parent: Some(parent),
            labels: HashMap::new(),
            gosub_stack: Vec::new(),
            option_base: 0,
        }))
    }

    pub fn get(&self, name: &str, suffix: Option<TypeSuffix>) -> Option<Value> {
        let key = Self::var_key(name, suffix);
        if let Some(v) = self.constants.get(&key) {
            return Some(v.clone());
        }
        if let Some(v) = self.vars.get(&key) {
            return Some(v.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().get(name, suffix);
        }
        None
    }

    pub fn set(&mut self, name: &str, suffix: Option<TypeSuffix>, value: Value) {
        let key = Self::var_key(name, suffix);
        // Don't overwrite constants
        if self.constants.contains_key(&key) {
            return; // Silently ignore; caller should check
        }
        self.vars.insert(key, value);
    }

    pub fn define_const(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        let key = Self::var_key(name, None);
        if self.constants.contains_key(&key) {
            return Err(RuntimeError::DuplicateDefinition { name: name.into() });
        }
        self.constants.insert(key, value);
        Ok(())
    }

    pub fn register_label(&mut self, label: &Label, index: usize) {
        self.labels.insert(label.to_string(), index);
    }

    pub fn resolve_label(&self, label: &Label) -> Option<usize> {
        self.labels.get(&label.to_string()).copied()
    }

    fn var_key(name: &str, suffix: Option<TypeSuffix>) -> String {
        match suffix {
            Some(s) => format!("{}{}", name, s.to_char()),
            None => name.to_string(),
        }
    }
}
