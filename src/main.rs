mod api;
mod config;
mod secret;
mod ui;

const VERSION: &str = match option_env!("LZT_VERSION") {
    Some(v) => v,
    None => concat!("v", env!("CARGO_PKG_VERSION"), "-dev"),
};

const HELP: &str = "\
lzt — post threads to lolz via API

  lzt -t [-f <id>]   create a thread (forum_id from settings or -f)
  lzt -s             settings (token + forum_id)
  lzt -h             help
  lzt -V             version";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut thread = false;
    let mut settings = false;
    let mut forum: Option<u64> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" | "--thread" => thread = true,
            "-s" | "--settings" => settings = true,
            "-f" | "--forum" => {
                i += 1;
                match args.get(i).and_then(|x| x.parse().ok()) {
                    Some(id) => forum = Some(id),
                    None => {
                        ui::fail("-f expects a numeric forum_id");
                        std::process::exit(2);
                    }
                }
            }
            "-h" | "--help" => {
                println!("{HELP}");
                return;
            }
            "-v" | "-V" | "--version" => {
                println!("lzt {VERSION}");
                return;
            }
            other => {
                ui::fail(&format!("unknown argument: {other}"));
                println!("{HELP}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let code = if settings {
        run_settings()
    } else if thread {
        run_thread(forum).await
    } else {
        println!("{HELP}");
        0
    };
    std::process::exit(code);
}

async fn run_thread(forum: Option<u64>) -> i32 {
    let cfg = config::load();
    let token = match cfg.token() {
        Some(t) if !t.is_empty() => t,
        _ => {
            ui::fail("no token set");
            ui::hint("run: lzt -s");
            return 1;
        }
    };
    let forum_id = match forum.or(cfg.forum_id) {
        Some(f) => f,
        None => {
            ui::fail("no forum_id set");
            ui::hint("set it via lzt -s or pass -f <id>");
            return 1;
        }
    };

    ui::banner("new thread");
    ui::intro();
    let title = match ui::required("title › ") {
        Some(t) => t,
        None => return 1,
    };
    let body = match ui::body() {
        Some(b) if !b.trim().is_empty() => b,
        Some(_) => {
            ui::fail("empty post — cancelled");
            return 1;
        }
        None => return 1,
    };

    let spin = ui::Spin::start("posting…");
    let result = api::create_thread(&token, forum_id, &title, &body).await;
    spin.stop();

    match result {
        Ok(url) => {
            ui::success("Thread created successfully ^.^", &url);
            0
        }
        Err(e) => {
            ui::fail(&e);
            1
        }
    }
}

fn run_settings() -> i32 {
    let mut cfg = config::load();
    ui::banner("settings");

    let tip = match cfg.token() {
        Some(t) if t.len() >= 6 => format!("…{}", &t[t.len() - 6..]),
        Some(_) => "set".into(),
        None => "empty".into(),
    };
    if let Some(v) = ui::line(&format!("token ({tip}) › ")) {
        if !v.is_empty() {
            cfg.set_token(&v);
        }
    }

    let tip = cfg.forum_id.map_or("empty".into(), |f| f.to_string());
    if let Some(v) = ui::line(&format!("forum_id ({tip}) › ")) {
        if !v.is_empty() {
            match v.parse::<u64>() {
                Ok(f) => cfg.forum_id = Some(f),
                Err(_) => {
                    ui::fail("forum_id must be a number");
                    return 1;
                }
            }
        }
    }

    match config::save(&cfg) {
        Ok(path) => {
            ui::success("saved", &path);
            0
        }
        Err(e) => {
            ui::fail(&format!("couldn't save: {e}"));
            1
        }
    }
}
