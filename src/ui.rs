use rustyline::config::Config;
use rustyline::completion::Completer;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::history::MemHistory;
use rustyline::validate::Validator;
use rustyline::{
    Cmd, ConditionalEventHandler, Editor, Event, EventContext, EventHandler, Helper, KeyCode,
    KeyEvent, Modifiers, RepeatCount,
};
use std::borrow::Cow;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
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

pub const IMG: &str = "📷";

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

static ORIG_TERMIOS: std::sync::OnceLock<libc::termios> = std::sync::OnceLock::new();

fn restore_terminal() {
    const SEQ: &[u8] = b"\x1b[?25h\x1b[0m\r\n";
    unsafe {
        libc::write(1, SEQ.as_ptr() as *const libc::c_void, SEQ.len());
        if let Some(t) = ORIG_TERMIOS.get() {
            libc::tcsetattr(0, libc::TCSANOW, t);
        } else {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) == 0 {
                t.c_lflag |= libc::ICANON | libc::ECHO | libc::ISIG;
                t.c_iflag |= libc::ICRNL | libc::IXON;
                libc::tcsetattr(0, libc::TCSANOW, &t);
            }
        }
    }
}

extern "C" fn on_signal(_: libc::c_int) {
    restore_terminal();
    unsafe { libc::_exit(130) };
}

pub fn guard() {
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut t) == 0 {
            let _ = ORIG_TERMIOS.set(t);
        }
        libc::signal(libc::SIGINT, on_signal as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as libc::sighandler_t);
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));
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
    fn highlight<'l>(&self, line: &'l str, _: usize) -> Cow<'l, str> {
        if styled() && line.contains(IMG) {
            Cow::Owned(line.replace(IMG, &format!("{ACCENT}{IMG}{RESET}")))
        } else {
            Cow::Borrowed(line)
        }
    }
    fn highlight_char(&self, line: &str, _: usize, _: CmdKind) -> bool {
        line.contains(IMG)
    }
}

fn editor() -> Option<Editor<Prompt, MemHistory>> {
    let mut ed = Editor::with_history(Config::builder().build(), MemHistory::new()).ok()?;
    ed.set_helper(Some(Prompt));
    Some(ed)
}

fn query_row() -> Option<u16> {
    unsafe {
        let mut orig: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut orig) != 0 {
            return None;
        }
        let mut raw = orig;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 2;
        if libc::tcsetattr(0, libc::TCSAFLUSH, &raw) != 0 {
            return None;
        }
        print!("\x1b[6n");
        let _ = io::stdout().flush();
        let mut row: u16 = 0;
        let mut esc = false;
        let mut in_csi = false;
        let mut after_semi = false;
        loop {
            let mut b = [0u8; 1];
            if libc::read(0, b.as_mut_ptr() as *mut libc::c_void, 1) <= 0 {
                break;
            }
            match b[0] {
                0x1b => {
                    esc = true;
                    in_csi = false;
                    after_semi = false;
                    row = 0;
                }
                b'[' if esc => in_csi = true,
                b';' => after_semi = true,
                b'R' => break,
                c if in_csi && !after_semi && c.is_ascii_digit() => {
                    row = row.saturating_mul(10).saturating_add((c - b'0') as u16);
                }
                _ => {}
            }
        }
        libc::tcsetattr(0, libc::TCSANOW, &orig);
        (row > 0).then_some(row)
    }
}

pub struct Cat {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    flash: Arc<AtomicUsize>,
}

impl Cat {
    pub fn start() -> Cat {
        let flash = Arc::new(AtomicUsize::new(0));
        let mut cat = Cat {
            stop: Arc::new(AtomicBool::new(true)),
            handle: None,
            flash: flash.clone(),
        };
        if !tty() {
            return cat;
        }
        let row = query_row();
        println!("{ACCENT}^.^{RESET}");
        let _ = io::stdout().flush();
        let Some(row) = row else {
            return cat;
        };
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = thread::spawn(move || {
            let mut i = 0usize;
            while !flag.load(Ordering::Relaxed) {
                let face = if i % 12 == 11 { "-.-" } else { "^.^" };
                if flash.load(Ordering::Relaxed) > 0 {
                    flash.fetch_sub(1, Ordering::Relaxed);
                    print!("\x1b7\x1b[{row};1H\x1b[2K{ACCENT}{face}{RESET}  {DIM}no image{RESET}\x1b8");
                } else {
                    print!("\x1b7\x1b[{row};1H\x1b[2K{ACCENT}{face}{RESET}\x1b8");
                }
                let _ = io::stdout().flush();
                i += 1;
                thread::sleep(Duration::from_millis(120));
            }
            print!("\x1b7\x1b[{row};1H\x1b[2K{ACCENT}^.^{RESET}\x1b8");
            let _ = io::stdout().flush();
        });
        cat.stop = stop;
        cat.handle = Some(handle);
        cat
    }

    pub fn flash(&self) -> Arc<AtomicUsize> {
        self.flash.clone()
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle {
            let _ = h.join();
        }
    }
}

pub fn ask_title() -> Option<String> {
    if !tty() {
        return required("title › ");
    }
    match editor() {
        Some(mut ed) => loop {
            match ed.readline("title › ") {
                Ok(s) if !s.trim().is_empty() => break Some(s.trim().to_string()),
                Ok(_) => {}
                Err(_) => break None,
            }
        },
        None => None,
    }
}

fn capture(args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new(args[0]).args(&args[1..]).output().ok()?;
    out.status.success().then_some(out.stdout)
}

fn clipboard_image() -> Option<(Vec<u8>, String)> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let list = if wayland {
        capture(&["wl-paste", "--list-types"])?
    } else {
        capture(&["xclip", "-selection", "clipboard", "-t", "TARGETS", "-o"])?
    };
    let list = String::from_utf8_lossy(&list);
    let avail: Vec<&str> = list.lines().map(str::trim).collect();
    let mime = ["image/png", "image/jpeg", "image/gif", "image/webp"]
        .into_iter()
        .find(|m| avail.contains(m))?;
    let bytes = if wayland {
        capture(&["wl-paste", "--type", mime])?
    } else {
        capture(&["xclip", "-selection", "clipboard", "-t", mime, "-o"])?
    };
    (!bytes.is_empty()).then(|| (bytes, mime.to_string()))
}

type Images = Arc<Mutex<Vec<(Vec<u8>, String)>>>;

struct PasteImage {
    images: Images,
    flash: Arc<AtomicUsize>,
}

impl ConditionalEventHandler for PasteImage {
    fn handle(&self, _: &Event, _: RepeatCount, _: bool, ctx: &EventContext<'_>) -> Option<Cmd> {
        match clipboard_image() {
            Some(img) => {
                let idx = ctx.line()[..ctx.pos()].matches(IMG).count();
                let mut imgs = self.images.lock().unwrap();
                let at = idx.min(imgs.len());
                imgs.insert(at, img);
                Some(Cmd::Insert(1, IMG.to_string()))
            }
            None => {
                self.flash.store(6, Ordering::Relaxed);
                Some(Cmd::Noop)
            }
        }
    }
}

struct DelImage {
    images: Images,
}

impl ConditionalEventHandler for DelImage {
    fn handle(&self, _: &Event, _: RepeatCount, _: bool, ctx: &EventContext<'_>) -> Option<Cmd> {
        let head = &ctx.line()[..ctx.pos()];
        if head.ends_with(IMG) {
            let idx = head.matches(IMG).count().saturating_sub(1);
            let mut imgs = self.images.lock().unwrap();
            if idx < imgs.len() {
                imgs.remove(idx);
            }
        }
        None
    }
}

pub fn body(flash: Arc<AtomicUsize>) -> Option<(String, Vec<(Vec<u8>, String)>)> {
    println!("{}", paint("body · Ctrl+D to post", DIM));
    if !tty() {
        let mut buf = String::new();
        io::stdin().lock().read_to_string(&mut buf).ok()?;
        return Some((buf.trim_matches('\n').to_string(), Vec::new()));
    }
    let images: Images = Arc::new(Mutex::new(Vec::new()));
    let mut ed = editor()?;
    ed.bind_sequence(
        KeyEvent(KeyCode::Enter, Modifiers::NONE),
        EventHandler::Simple(Cmd::Newline),
    );
    ed.bind_sequence(KeyEvent::ctrl('d'), EventHandler::Simple(Cmd::AcceptLine));
    ed.bind_sequence(
        KeyEvent::ctrl('v'),
        EventHandler::Conditional(Box::new(PasteImage {
            images: images.clone(),
            flash,
        })),
    );
    ed.bind_sequence(
        KeyEvent(KeyCode::Backspace, Modifiers::NONE),
        EventHandler::Conditional(Box::new(DelImage {
            images: images.clone(),
        })),
    );
    let text = ed.readline("").ok().map(|s| s.trim_matches('\n').to_string())?;
    let imgs = images.lock().unwrap().clone();
    Some((text, imgs))
}

pub fn banner(sub: &str) {
    println!();
    if styled() {
        println!("{BOLD}{ACCENT}lzt{RESET}{DIM} · {sub}{RESET}");
    } else {
        println!("lzt · {sub}");
    }
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

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
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
            let mut tick = tokio::time::interval(Duration::from_millis(120));
            let mut i = 0usize;
            loop {
                tick.tick().await;
                let dots = ".".repeat(1 + i % 3);
                print!(
                    "\r{ACCENT}{}{RESET} {DIM}{msg}{dots}{RESET}\x1b[K",
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
