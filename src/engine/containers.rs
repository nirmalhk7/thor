pub struct StoragePartition {
    pub partition_id: String,
    pub base_path: String,
}

impl StoragePartition {
    pub fn new(partition_id: &str, base_path: &str) -> Self {
        Self {
            partition_id: partition_id.to_string(),
            base_path: format!("{}/{}", base_path, partition_id),
        }
    }

    pub fn isolate(&self) {
        println!("Isolating storage for partition: {}", self.partition_id);
    }
}

pub struct NetworkContext {
    pub context_id: String,
    pub proxy_rules: Vec<String>,
}

impl NetworkContext {
    pub fn new(context_id: &str) -> Self {
        Self {
            context_id: context_id.to_string(),
            proxy_rules: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: &str) {
        self.proxy_rules.push(rule.to_string());
    }

    pub fn isolate(&self) {
        println!("Isolating network context: {}", self.context_id);
    }
}
