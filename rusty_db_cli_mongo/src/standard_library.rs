use std::collections::HashMap;

pub struct StandardLibrary {
    pub types: HashMap<String, TypeInfo>,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub methods: Vec<MethodInfo>,
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub signature: String,
    pub documentation: String,
}

pub trait Typed {
    fn get_type_info(&self) -> TypeInfo;
}

impl StandardLibrary {
    pub fn new() -> Self {
        Self {
            types: HashMap::from([(
                "db".into(),
                TypeInfo {
                    name: "Database handler".to_string(),
                    methods: vec![MethodInfo {
                        name: "Test collection".to_string(),
                        signature: "collection".to_string(),
                        documentation: "".to_string(),
                    }],
                },
            )]),
        }
    }

    pub fn get_type_info(&self, name: &str) -> Option<TypeInfo> {
        self.types.get(name).cloned()
    }
}
