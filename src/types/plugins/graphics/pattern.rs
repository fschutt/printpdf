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
}

impl Into<lopdf::Dictionary> for PatternList {
    fn into(self)
    -> lopdf::Dictionary
    {
        // todo
        lopdf::Dictionary::new()
    }
}