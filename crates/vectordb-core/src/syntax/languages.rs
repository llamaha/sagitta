use std::collections::HashSet;

lazy_static::lazy_static! {
    pub static ref SUPPORTED_LANGUAGES: HashSet<&'static str> = {
        let mut s = HashSet::new();
        s.insert("rs");
        s.insert("md");
        s.insert("go");
        s.insert("js");
        s.insert("jsx");
        s.insert("ts");
        s.insert("tsx");
        s.insert("yaml");
        s.insert("yml");
        s.insert("rb");
        s.insert("py");
        s
    };
} 