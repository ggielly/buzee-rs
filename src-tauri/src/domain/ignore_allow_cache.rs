use std::collections::{HashMap, HashSet};

#[derive(Default, Clone, Debug)]
pub struct PrefixSet {
    children: HashMap<u8, Box<PrefixSet>>,
    is_terminal: bool,
}

impl PrefixSet {
    pub fn insert(&mut self, s: &str) {
        let mut node = self;
        for b in s.as_bytes() {
            node = node.children.entry(*b).or_default();
        }
        node.is_terminal = true;
    }

    pub fn contains_prefix_of(&self, path: &str) -> bool {
        let mut node = self;
        for b in path.as_bytes() {
            if node.is_terminal {
                return true;
            }
            match node.children.get(b) {
                Some(next) => node = next,
                None => return false,
            }
        }
        node.is_terminal
    }
}

#[derive(Default, Clone, Debug)]
pub struct IgnoreAllowCacheState {
    pub ignored_file_paths: HashSet<String>,
    pub ignored_folder_prefixes: PrefixSet,
    pub ignored_indexonly_file_paths: HashSet<String>,
    pub ignored_indexonly_folder_prefixes: PrefixSet,
    pub allowed_file_paths: HashSet<String>,
    pub allowed_folder_prefixes: PrefixSet,
}

impl IgnoreAllowCacheState {
    pub fn from_db(conn: &mut diesel::SqliteConnection) -> Self {
        use crate::infrastructure::indexing::{get_all_allowed_paths, get_all_ignored_paths};

        let ignored_items = get_all_ignored_paths(conn);
        let allowed_items = get_all_allowed_paths(conn);

        let mut ignored_file_paths = HashSet::new();
        let mut ignored_folder_prefixes = PrefixSet::default();
        let mut ignored_indexonly_file_paths = HashSet::new();
        let mut ignored_indexonly_folder_prefixes = PrefixSet::default();
        let mut allowed_file_paths = HashSet::new();
        let mut allowed_folder_prefixes = PrefixSet::default();

        for item in &ignored_items {
            if item.is_folder {
                if item.ignore_indexing {
                    ignored_folder_prefixes.insert(&item.path);
                } else {
                    ignored_indexonly_folder_prefixes.insert(&item.path);
                }
            } else if item.ignore_indexing {
                ignored_file_paths.insert(item.path.clone());
            } else {
                ignored_indexonly_file_paths.insert(item.path.clone());
            }
        }

        for item in &allowed_items {
            if item.is_folder {
                allowed_folder_prefixes.insert(&item.path);
            } else {
                allowed_file_paths.insert(item.path.clone());
            }
        }

        Self {
            ignored_file_paths,
            ignored_folder_prefixes,
            ignored_indexonly_file_paths,
            ignored_indexonly_folder_prefixes,
            allowed_file_paths,
            allowed_folder_prefixes,
        }
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        self.allowed_file_paths.contains(path)
            || self.allowed_folder_prefixes.contains_prefix_of(path)
    }

    pub fn is_ignored(&self, path: &str) -> bool {
        self.ignored_file_paths.contains(path)
            || self.ignored_folder_prefixes.contains_prefix_of(path)
    }

    pub fn is_ignored_index_only(&self, path: &str) -> bool {
        self.ignored_indexonly_file_paths.contains(path)
            || self
                .ignored_indexonly_folder_prefixes
                .contains_prefix_of(path)
    }

    pub fn should_skip(&self, path: &str) -> bool {
        if self.is_allowed(path) {
            return false;
        }
        self.is_ignored(path)
    }
}