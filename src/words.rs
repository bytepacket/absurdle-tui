pub struct Words<'a> {
    pub guesses: Vec<&'a str>,
}

impl<'a> Words<'a> {
    pub fn new() -> Self {
        // include_str! opens a file and parses file (aahed\naalii.....) and then splits at \n into iterator and collects as a collection of &'static str
        let guesses: Vec<&'a str> = include_str!("words.txt").lines().collect();

        Words { guesses }
    }
}
