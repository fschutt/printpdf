use *;
use std::collections::HashMap;

/// __STUB__
#[derive(Debug)]
pub struct Pattern {

}

impl Pattern {
    /// Creates a new Pattern
    pub fn new()
    -> Self
    {
        Self 
        { 

        }
    }
}

#[derive(Debug)]
pub struct PatternRef {
    name: String,
}

impl PatternRef {
    pub fn new(index: usize)
    -> Self
    {
        Self {
            name: format!("PT{}", index),
        }
    }
}

#[derive(Debug)]
pub struct PatternList {
    patterns: HashMap<String, Pattern>,
}

impl PatternList {
    /// Creates a new pattern list
    pub fn new()
    -> Self
    {
        Self {
            patterns: HashMap::new(),
        }
    }

    /// Adds a new pattern to the pattern list
    pub fn add_pattern(&mut self, pattern: Pattern)
    -> PatternRef
    {
        let len = self.patterns.len();
        let pattern_ref = PatternRef::new(len);
        self.patterns.insert(pattern_ref.name.clone(), pattern);
        pattern_ref
    }
}

impl Into<lopdf::Dictionary> for PatternList {
    fn into(self)
    -> lopdf::Dictionary
    {
        // todo
        lopdf::Dictionary::new()
    }
}