use rustyline::completion::Completer;
use rustyline::config::Config;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::MemHistory;
use rustyline::validate::Validator;
use rustyline::{Cmd, Editor, EventHandler, Helper, KeyCode, KeyEvent, Modifiers};
use std::borrow::Cow;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ULINE: &str = "\x1b[4m";
const ACCENT: &str = "\x1b[38;5;177m";
const OK: &str = "\x1b[38;5;114m";
const ERR: &str = "\x1b[38;5;203m";
const GHOST: &str = "\x1b[38;5;240m";

fn styled() -> bool {
    static S: OnceLock<bool> = OnceLock::new();
    *S.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal())
}

fn paint(s: &str, code: &str) -> String {
    if styled() {
        format!("{code}{s}{RESET}")
    } else {
        s.to_string()
    }
}

fn tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

struct Prompt;

impl Completer for Prompt {
    type Candidate = String;
}
impl Hinter for Prompt {
    type Hint = String;
}
impl Validator for Prompt {}
impl Helper for Prompt {}
impl Highlighter for Prompt {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(&'s self, prompt: &'p str, _: bool) -> Cow<'b, str> {
        match prompt.find('›') {
            Some(i) if styled() => {
                let rest = &prompt[i + '›'.len_utf8()..];
                Cow::Owned(format!("{DIM}{}{RESET}{ACCENT}›{RESET}{rest}", &prompt[..i]))
            }
            _ => Cow::Borrowed(prompt),
        }
    }
}

fn editor() -> Option<Editor<Prompt, MemHistory>> {
    let mut ed = Editor::with_history(Config::builder().build(), MemHistory::new()).ok()?;
    ed.set_helper(Some(Prompt));
    Some(ed)
}

pub fn banner(sub: &str) {
    println!();
    if styled() {
        println!("{BOLD}{ACCENT}lzt{RESET}{DIM} · {sub}{RESET}");
    } else {
        println!("lzt · {sub}");
    }
}

pub fn ask_title() -> Option<String> {
    if !tty() {
        return required("title › ");
    }
    println!("{ACCENT}(^.^){RESET}");
    let _ = io::stdout().flush();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let anim = thread::spawn(move || {
        let frames = ["^.^", "^.^", "-.-", "^.^", "o.o", "^.^"];
        let mut i = 0usize;
        while !flag.load(Ordering::Relaxed) {
            print!("\x1b7\x1b[1A\r\x1b[2K{ACCENT}({}){RESET}\x1b8", frames[i % frames.len()]);
            let _ = io::stdout().flush();
            i += 1;
            for _ in 0..6 {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    });
    let out = match editor() {
        Some(mut ed) => loop {
            match ed.readline("title › ") {
                Ok(s) if !s.trim().is_empty() => break Some(s.trim().to_string()),
                Ok(_) => {}
                Err(_) => break None,
            }
        },
        None => None,
    };
    stop.store(true, Ordering::Relaxed);
    let _ = anim.join();
    out
}

pub fn line(prompt: &str) -> Option<String> {
    if tty() {
        editor()?.readline(prompt).ok().map(|s| s.trim().to_string())
    } else {
        print!("{prompt}");
        let _ = io::stdout().flush();
        let mut s = String::new();
        match io::stdin().lock().read_line(&mut s) {
            Ok(n) if n > 0 => Some(s.trim().to_string()),
            _ => None,
        }
    }
}

pub fn required(prompt: &str) -> Option<String> {
    loop {
        match line(prompt) {
            Some(s) if !s.is_empty() => return Some(s),
            Some(_) => println!("{}", paint("empty — try again", DIM)),
            None => return None,
        }
    }
}

pub fn body() -> Option<String> {
    println!("{}", paint("body · Ctrl+D to post", DIM));
    if tty() {
        let mut ed = editor()?;
        ed.bind_sequence(
            KeyEvent(KeyCode::Enter, Modifiers::NONE),
            EventHandler::Simple(Cmd::Newline),
        );
        ed.bind_sequence(KeyEvent::ctrl('d'), EventHandler::Simple(Cmd::AcceptLine));
        ed.readline("").ok().map(|s| s.trim_matches('\n').to_string())
    } else {
        let mut buf = String::new();
        io::stdin().lock().read_to_string(&mut buf).ok()?;
        Some(buf.trim_matches('\n').to_string())
    }
}

pub fn success(msg: &str, url: &str) {
    let (msg, url) = (clean(msg), clean(url));
    if styled() {
        for code in [GHOST, OK] {
            print!("\r{code}✓{RESET} {DIM}{msg}{RESET}");
            let _ = io::stdout().flush();
            thread::sleep(Duration::from_millis(55));
        }
        println!("\r\x1b[2K{BOLD}{OK}✓{RESET} {OK}{msg}{RESET}");
        println!("  {ULINE}{ACCENT}{url}{RESET}");
    } else {
        println!("✓ {msg}");
        println!("{url}");
    }
}

pub fn fail(msg: &str) {
    eprintln!("{} {}", paint("✗", ERR), paint(&clean(msg), ERR));
}

pub fn hint(msg: &str) {
    println!("{}", paint(msg, DIM));
}

pub struct Spin {
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Spin {
    pub fn start(msg: &str) -> Spin {
        if !styled() {
            println!("{msg}");
            return Spin { handle: None };
        }
        let msg = msg.to_string();
        print!("\x1b[?25l");
        let _ = io::stdout().flush();
        let handle = tokio::spawn(async move {
            const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut tick = tokio::time::interval(Duration::from_millis(80));
            let mut i = 0usize;
            loop {
                tick.tick().await;
                let cat = if i % 14 == 13 { "-.-" } else { "^.^" };
                print!(
                    "\r{ACCENT}{}{RESET} {DIM}{msg}{RESET}  {ACCENT}{cat}{RESET}",
                    FRAMES[i % FRAMES.len()]
                );
                let _ = io::stdout().flush();
                i += 1;
            }
        });
        Spin { handle: Some(handle) }
    }

    pub fn stop(self) {
        if let Some(h) = self.handle {
            h.abort();
            print!("\r\x1b[2K\x1b[?25h");
            let _ = io::stdout().flush();
        }
    }
}
