/* going to explain the syntax that i use a lot that you normally wouldnt see in other languages
 ?: rust functions can return an Ok() or Err() type in a Result enum (or Some() and None in an Option enum), so
    if its an Ok() enum it goes through, if its an Err enum it returns immediately, as seen through the return sig
 match: self-explanatory, utilized for enums, where it can have many types and based on the type performs an action
 format! and execute!: this is a macro, expands into multiple lines of code, so the parameters aren't set, used as
                       an alternative tol method overloading, because that concept does not exist in Rust
 traits: most types used here implement some sort of trait, look at CrosstermBackend::new(), it takes in not the Stdout
         type itself, but any type/struct that implement Write (shown in pub const fn new(writer: Write) -> Self)

 other stuff you should know:
    &str vs String: String is an owned, heap-allocated String that is growable. &str is a collection of characters on the stack which you do not own.
                    Since &str is a reference, you would think that str would give you the owned type, not quite. str is a dynamically sized type, meaning
                    at compile-time, you do not know the size, which is bad. That's why you hide it behind a pointer, so it references the data instead of
                    owning something that has an arbitrary length.
    arrays vs slices: Arrays are owned types, defined by [T; N], where T is type and N is size. It is stored on the stack. A slice is represented as &[T]
                      A slice is a view into an array, so it does not own something, but only borrows the data. A slice contains two things, the length and its
                      size, which means from this we can figure out the size at runtime. It can also be said that a &str is similar to a slice as well.
*/



// look at this for info about game: https://github.com/kevinlin1/absurdle/blob/main/README.md
// also would recommend to look at the main function first, since that's the entry point of the program
mod words;

use std::collections::{HashMap, HashSet};
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use words::Words;

// green means correct placement, yellow means letter is in word, gray means the letter is not present
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Feedback {
    Green,
    Yellow,
    Gray,
}

// the message, success means you got it, error could mean the word isn't valid, info could be how many words remaning
#[derive(Debug, Clone, Copy)]
enum StatusLevel {
    Info,
    Error,
    Success,
}

impl StatusLevel {
    fn style(self) -> Style {
        // based on type, it would be outputted differently, .fg() changes the colour
        match self {
            StatusLevel::Info => Style::default().fg(Color::Cyan),
            StatusLevel::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            StatusLevel::Success => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        }
    }
}

// KeyState is used to colour the icons on the keyboard located on the right-hand side
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum KeyState {
    White,
    Gray,
    Yellow,
    Green,
}

impl KeyState {
    fn style(self) -> Style {
        match self {
            KeyState::White => Style::default()
                .fg(Color::Black) // change foreground to black
                .bg(Color::White) // change background to white
                .add_modifier(Modifier::BOLD), // bold
            KeyState::Gray => Style::default()
                .fg(Color::White)// change foreground to white
                .bg(Color::DarkGray) // change background to dark gray
                .add_modifier(Modifier::BOLD), // bold
            KeyState::Yellow => Style::default()
                .fg(Color::Black)// change foreground to black
                .bg(Color::Yellow)// change background to yellow
                .add_modifier(Modifier::BOLD), // bold
            KeyState::Green => Style::default()
                .fg(Color::Black) // change foreground to black
                .bg(Color::Green) // change background to green
                .add_modifier(Modifier::BOLD), // bold
        }
    }
}

// word is 5 letters, so a letter is represented by the Feedback struct
type Pattern = [Feedback; 5];

#[derive(Debug, Clone)]
struct GuessEntry {
    guess: String, // word inputted
    pattern: Pattern, // the colour of each letter of the word
    remaining_after: usize, // largest pool of words remaining after this word
}

struct App {
    dictionary: Vec<&'static str>, // dictionary
    dictionary_set: HashSet<&'static str>, // dictionary as a hashset
    remaining: Vec<&'static str>, // remaining words
    input: String, // input
    history: Vec<GuessEntry>,
    restart_feedback: Option<String>, // possible answers from the previous run, shown after Ctrl-R restart
    status: String, // self-explanatory, prints out in the text bar under the game
    status_level: StatusLevel, // self-explanatory
    won: bool, // self-explanatory
    should_quit: bool, // self-explanatory
}

impl App {
    // instantiates the App
    fn new() -> Self {
        let dictionary: Vec<&'static str> = Words::new().guesses;
        let dictionary_set = dictionary.iter().copied().collect();

        Self {
            remaining: dictionary.clone(),
            dictionary,
            dictionary_set,
            input: String::new(),
            history: Vec::new(),
            restart_feedback: None,
            status: "Type a five-letter word and press Enter.".to_string(), // starting message
            status_level: StatusLevel::Info,
            won: false,
            should_quit: false,
        }
    }

    // ran after user puts Ctrl-R (check on_key function)
    fn restart(&mut self) {
        // if the player gives up mid-game, save the possible answers so we can show it after the restart
        let previous_feedback = if !self.won && !self.history.is_empty() {
            Some(format!(
                "Possible answers before restart ({}): {}",
                self.remaining.len(),
                self.remaining.join(", ")
            ))
        } else {
            None
        };

        // takes the dictionary and clones it
        self.remaining = self.dictionary.clone();
        // clears input
        self.input.clear();
        // clears history of input
        self.history.clear();
        // keep a copy of previous pool so we can render it as feedback in the fresh session
        self.restart_feedback = previous_feedback;
        // set won bool to false
        self.won = false;

        // changes status and adds a new one as an Info
        if self.restart_feedback.is_some() {
            self.set_status(
                "New game started. Previous possible answers are shown on the board.",
                StatusLevel::Info,
            );
        } else {
            self.set_status(
                format!("New game started. {} possible answers.", self.remaining.len()),
                StatusLevel::Info,
            );
        }
    }

    // Ctrl-C and Ctrl-R are implemented keybinds, as said we have to implement our own since we use raw mode
    fn on_key(&mut self, key: KeyEvent) {
        // if the key pressed is the Control key
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                // if its c that's pressed
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    // quit bool is true
                    self.should_quit = true;
                    return;
                }
                // if its r that's pressed
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    // instantly restarts the program
                    self.restart();
                    return;
                }
                _ => {}
            }
        }

        // if control AND alt are both pressed, return
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }

        // now looks for key codes that aren't associated with control
        match key.code {
            // esc is an alias to control
            KeyCode::Esc => self.should_quit = true,
            // user hasn't won yet, so we have to make our own backspace
            // said this a lot but raw mode doesn't let us use these, so likewise you also wouldn't see us using the write macro
            KeyCode::Backspace if !self.won => {
                // removes the last character
                self.input.pop();
            }
            KeyCode::Enter => {
                // sends input, checks if the user won
                if self.won {
                    // creates a new status that player won as a Success type
                    self.set_status(
                        "Solved. Press Ctrl+R to play again, or Esc to quit.",
                        StatusLevel::Success,
                    );
                } else {
                    // if not won, submit the guess
                    self.submit_guess();
                }
            }
            /* writing into the input box, only can write if it's actually a character, you haven't won, and it's an
               alphabetical character
            */
            KeyCode::Char(c) if !self.won && c.is_ascii_alphabetic() && self.input.len() < 5 => {
                // push it as a lowercase, wouldn't be shown as one though, we do that later
                self.input.push(c.to_ascii_lowercase());
            }
            // anything else, like [ would be ignored case
            _ => {}
        }
    }

    fn submit_guess(&mut self) {
        // checks if it's a 5-letter word
        if self.input.len() != 5 {
            self.set_status("Guess must be exactly 5 letters.", StatusLevel::Error);
            return;
        }

        // if it's not a valid word defined in words.rs, true
        if !self.dictionary_set.contains(self.input.as_str()) {
            self.set_status("Not a valid 5-letter word.", StatusLevel::Error);
            return;
        }

        // if it is in the already entered words, as checking in the closure, true
        if self.history.iter().any(|guess| guess.guess == self.input) {
            self.set_status("You already put this in!", StatusLevel::Error);
            return;
        }

        // guess is cloned string
        let guess = self.input.clone();

        // two variables, pattern is the colour of each letter, next_remaining is a Vec
        let (pattern, next_remaining) = choose_feedback(&guess, &self.remaining);
        // takes the length of the remaining words
        let remaining_after = next_remaining.len();

        // adds the input into the history
        self.history.push(GuessEntry {
            guess: guess.clone(),
            pattern,
            remaining_after,
        });

        // update remaining words
        self.remaining = next_remaining;

        // clears the input after guess is processed
        self.input.clear();

        // user guesses correct
        if pattern == [Feedback::Green; 5] {
            // won becomes true
            self.won = true;
            // sets status to the Success type
            self.set_status(
                format!(
                    "You trapped Absurdle in {} guesses with {}.",
                    self.history.len(),
                    guess.to_uppercase()
                ),
                StatusLevel::Success,
            );
            // case where the user inputs a valid guess, still incorrect
        } else {
            self.set_status(
                format!("Feedback leaves {} possible words.", remaining_after),
                StatusLevel::Info,
            );
        }
    }

    // NOTE: impl Trait is the same thing as what other functions have, takes in types that can be converted into a string
    // could use dyn Trait as well, not much benefit in that
    fn set_status(&mut self, status: impl Into<String>, level: StatusLevel) {
        self.status = status.into(); // anything that implements Into<String> has the .into() type, in this case, .into() turning into a String is inferred
        self.status_level = level; // changes the status level
    }
}

fn get_feedback(guess: &str, target: &str) -> Pattern {
    // turn both strings into Vec<char> so we can index by position (rust strings can't be indexed directly)
    let guess_chars: Vec<char> = guess.chars().collect(); // .chars.collect() means its going to go over the string and then turn it into an Iterator (NOT an array), the .collect() part turns anything that implements the FromIterator trait into a collection
    let target_chars = target.chars().collect::<Vec<char>>(); // also collect() can make a bunch of iterators, it's only the type we gave the variable that it would turn it into, as an example, let target_chars = target.chars().collect() would be invalid because it cannot infer a type, but let target_chars = target.chars().collect::<Vec<char>>(); is absolutely valid
    let mut result = [Feedback::Gray; 5]; // default result, to be updated
    let mut target_used = [false; 5];

    // resolve greens first so those slots are consumed immediately
    for i in 0..5 {
        if guess_chars[i] == target_chars[i] {
            result[i] = Feedback::Green;
            target_used[i] = true;
        }
    }

    // for non-green letters, find unmatched target letters and mark as yellow
    for i in 0..5 {
        if result[i] == Feedback::Green {
            continue;
        }
        for j in 0..5 {
            if !target_used[j] && guess_chars[i] == target_chars[j] {
                result[i] = Feedback::Yellow;
                target_used[j] = true;
                break;
            }
        }
    }

    result
}

fn pattern_score(pattern: &Pattern) -> usize {
    // this is the tie-break, game would ideally want gray over yellow and yellow over green, so gray gets highest weight
    pattern.iter().fold(0, |acc, f| { // .fold() traverses over whole iterator, has starting value, and performs a function in hopes of returning a type that is the same to the starting value
        acc + match f {
            Feedback::Gray => 3,
            Feedback::Yellow => 1,
            Feedback::Green => 0,
        }
    })
}

fn choose_feedback(guess: &str, remaining: &[&'static str]) -> (Pattern, Vec<&'static str>) {
    // all words that would generate that pattern for this guess
    let mut feedback_groups: HashMap<Pattern, Vec<&'static str>> = HashMap::new();

    // partition remaining words by the pattern they'd show to the player
    for &word in remaining {
        let pattern = get_feedback(guess, word);
        feedback_groups.entry(pattern).or_default().push(word);
    }

    // pick the largest partition so the game stays as hard as possible
    // if partitions tie in size, use pattern_score to prefer less informative feedback
    feedback_groups
        .into_iter()
        .max_by_key(|(pattern, words)| (words.len(), pattern_score(pattern)))
        .expect("remaining words must not be empty")
}

fn tile_style(feedback: Feedback) -> Style {
    // translates board feedback into a coloured tile
    match feedback {
        Feedback::Green => Style::default()
            .fg(Color::Black)// foreground becomes black
            .bg(Color::Green) // background becomes green
            .add_modifier(Modifier::BOLD), // bold
        Feedback::Yellow => Style::default()
            .fg(Color::Black)// foreground becomes black
            .bg(Color::Yellow)// background becomes yellow
            .add_modifier(Modifier::BOLD), // bold
        Feedback::Gray => Style::default()
            .fg(Color::White)// foreground becomes white
            .bg(Color::DarkGray) // background becomes dark gray
            .add_modifier(Modifier::BOLD), // bold
    }
}

fn empty_tile_style() -> Style {
    // style used for blank boxes on rows that haven't been guessed yet
    Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD)
}

fn key_state_from_feedback(feedback: Feedback) -> KeyState {
    // keyboard colors are derived from feedback colors directly
    match feedback {
        Feedback::Green => KeyState::Green,
        Feedback::Yellow => KeyState::Yellow,
        Feedback::Gray => KeyState::Gray,
    }
}

fn derive_keyboard_states(history: &[GuessEntry]) -> HashMap<char, KeyState> {
    // final keyboard state per character (q/a/z etc)
    let mut key_states = HashMap::new();

    // each key can only upgrade in certainty: white -> gray -> yellow -> green
    for entry in history {
        for (ch, feedback) in entry.guess.chars().zip(entry.pattern.iter()) {
            // .chars() turns it into Chars struct, .zip() makes an iterator that iterates over two iterators
            let next_state = key_state_from_feedback(*feedback);
            key_states
                .entry(ch) // .entry() looks if the key is valid, and returns an Entry enum with Occupied and Vacant as variants
                // if Occupied variant is returned, modifies the value
                .and_modify(|state| {
                    if next_state > *state {
                        *state = next_state;
                    }
                })
                // .or_insert() applies if the key does not exist, then does it (this is sort of like a conditional, but its methods)
                .or_insert(next_state);
        }
    }

    key_states
}

fn render_guess_line(attempt: usize, entry: &GuessEntry) -> Line<'static> {
    // one completed row on the board, includes attempt number and remaining-word count
    let mut spans = Vec::with_capacity(18);
    spans.push(Span::styled(
        format!("{attempt:>2}. "),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    for (ch, feedback) in entry.guess.chars().zip(entry.pattern.iter()) {
        spans.push(Span::styled(
            format!(" {} ", ch.to_ascii_uppercase()),
            tile_style(*feedback),
        ));
        spans.push(Span::raw(" "));
    }

    spans.push(Span::styled(
        format!("{:>5} left", entry.remaining_after),
        Style::default().fg(Color::Gray),
    ));

    Line::from(spans)
}

fn render_empty_guess_line(attempt: usize) -> Line<'static> {
    // placeholder row for future guesses
    let mut spans = Vec::with_capacity(14);
    spans.push(Span::styled(
        format!("{attempt:>2}. "),
        Style::default().fg(Color::DarkGray),
    ));

    for _ in 0..5 {
        spans.push(Span::styled("   ", empty_tile_style()));
        spans.push(Span::raw(" "));
    }

    Line::from(spans)
}

fn render_keyboard_row(
    row: &str,
    indent: usize,
    key_states: &HashMap<char, KeyState>,
) -> Line<'static> {
    // render one keyboard row (qwerty/asdfg/zxcvb), with manual indent for shape
    let mut spans = Vec::with_capacity(row.len() + 2);

    spans.push(Span::raw(" ".repeat(indent))); // .push() appends element to back of Vec

    for ch in row.chars() {
        // .get() returns the Option enum, so it won't panic if the key doesn't exist, .copied() changes the type, and if the key isn't there, make it a White keystate
        let key_state = key_states.get(&ch).copied().unwrap_or(KeyState::White);
        spans.push(Span::styled(
            format!(" {} ", ch.to_ascii_uppercase()),
            key_state.style(),
        ));
    }

    Line::from(spans)
}

fn draw_ui(frame: &mut Frame, app: &App) {
    // top-level vertical layout: header, main body, current input, status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "ABSURDLE",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Adversarial Wordle in your terminal"),
        ]),
        Line::from("Enter: submit  Backspace: delete  Ctrl+R: restart  Esc/Ctrl+C: quit"),
    ])
    .block(Block::default().title("Game").borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    // split the body into board (left) and stats/keyboard (right)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);

    // board row count adapts to terminal height, with a floor so it doesn't collapse too much
    let board_rows = (body_chunks[0].height.saturating_sub(2) as usize).max(8);
    // if history is longer than visible rows, only render the newest rows
    let history_start = app.history.len().saturating_sub(board_rows);

    let mut board_lines: Vec<Line> = app
        .history
        .iter() // turns collection to iterator
        .skip(history_start)// skips first n amount of elements
        .enumerate()// gives the k and v pair
        .map(|(offset, entry)| render_guess_line(history_start + offset + 1, entry))// for each iteration, runs a closure
        .collect(); // turns it back into a collection

    // fill the rest of the board with blank placeholders
    while board_lines.len() < board_rows {
        let attempt = board_lines.len() + 1;
        board_lines.push(render_empty_guess_line(attempt));
    }

    // first-launch helper text in row 1 until the first guess is submitted
    if app.history.is_empty() {
        // if game was restarted mid-run, show the old remaining pool as feedback before first new guess
        if let Some(restart_feedback) = app.restart_feedback.as_deref() {
            board_lines[0] = Line::from(Span::styled(
                restart_feedback,
                Style::default().fg(Color::Gray),
            ));
        } else {
            board_lines[0] = Line::from(Span::styled(
                "Start typing below and press Enter to guess.",
                Style::default().fg(Color::Gray),
            ));
        }
    };

    let history = Paragraph::new(board_lines)
        .block(Block::default().title("Board").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(history, body_chunks[0]);

    // right side is split into stats (top) and keyboard legend (bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(7)])
        .split(body_chunks[1]);

    let mut sidebar_lines = vec![
        Line::from(format!("Dictionary: {}", app.dictionary.len())),
        Line::from(format!("Remaining: {}", app.remaining.len())),
        Line::from(format!("Guesses: {}", app.history.len())),
        Line::from(format!("State: {}", if app.won { "Solved" } else { "In play" })),
        Line::from(""),
    ];

    if let Some(last) = app.history.last() {
        sidebar_lines.push(Line::from(format!("Last cut: {}", last.remaining_after)));
    }

    let sidebar = Paragraph::new(sidebar_lines)
        .block(Block::default().title("Stats").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(sidebar, right_chunks[0]);

    // derive per-key colors from the full guess history
    let key_states = derive_keyboard_states(&app.history);
    let keyboard_lines = vec![
        render_keyboard_row("qwertyuiop", 0, &key_states),
        Line::from(""),
        render_keyboard_row("asdfghjkl", 1, &key_states),
        Line::from(""),
        render_keyboard_row("zxcvbnm", 2, &key_states),
        Line::from(""),
        Line::from(vec![
            Span::styled(" A ", KeyState::Green.style()),
            Span::raw(" green "),
            Span::styled(" A ", KeyState::Yellow.style()),
            Span::raw(" yellow "),
            Span::styled(" A ", KeyState::Gray.style()),
            Span::raw(" gray "),
            Span::styled(" A ", KeyState::White.style()),
            Span::raw(" white"),
        ]),
    ];

    let keyboard = Paragraph::new(keyboard_lines)
        .block(Block::default().title("Keyboard").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(keyboard, right_chunks[1]);

    // current guess input field, always rendered in uppercase for readability
    let input = Paragraph::new(app.input.to_uppercase())
        .block(
            Block::default()
                .title("Current Guess (5 letters)")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(input, chunks[2]);

    // manually position cursor inside the input widget while the game is still active
    if !app.won && chunks[2].width > 2 && chunks[2].height > 2 {
        let max_cursor_x = chunks[2].x + chunks[2].width - 2;
        let cursor_x = (chunks[2].x + 1 + app.input.len() as u16).min(max_cursor_x);
        let cursor_y = chunks[2].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    // bottom status bar shows validation errors, progress, or win message
    let status = Paragraph::new(app.status.as_str())
        .style(app.status_level.style())
        .block(Block::default().title("Status").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(status, chunks[3]);
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        // redraw every loop so UI reflects current input/history/state
        terminal.draw(|frame| draw_ui(frame, &app))?;

        if app.should_quit {
            break;
        }

        // poll avoids blocking forever, read then forwards key presses into app state
        if event::poll(Duration::from_millis(120))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    /*
      raw mode basically turns off the default features that the terminal would have,
      such as new line characters and special keys, so i would have to define them in
      this script
    */
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // instead of playing on the current terminal process, create a new screen
    execute!(stdout, EnterAlternateScreen)?;

    // create a wrapper that sends commands to the terminal, we pass in the stdout because it implements the Write trait
    let backend = CrosstermBackend::new(stdout);
    // we do not generally use CrosstermBackend, instead Terminal is more recommended, as per the docs
    let mut terminal = Terminal::new(backend)?;

    // run the application
    let run_result = run_app(&mut terminal);

    // when the user quits, make sure to: disable raw mode, leave the screen we made, and reveal the cursor to them
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // return the result from running the app
    run_result
}
